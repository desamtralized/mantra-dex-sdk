use std::fs;
use std::path::PathBuf;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::{Aead, OsRng, generic_array::GenericArray};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::{rand_core::RngCore, SaltString}};
use serde::{Serialize, Deserialize};

use crate::error::Error;

/// Encrypted wallet data stored on disk
#[derive(Serialize, Deserialize)]
struct EncryptedWalletData {
    /// Argon2 hash parameters and salt
    password_hash: String,
    /// Encrypted mnemonic (AES-256-GCM)
    encrypted_mnemonic: Vec<u8>,
    /// Nonce used for encryption
    nonce: Vec<u8>,
    /// Wallet metadata
    metadata: WalletMetadata,
}

/// Wallet metadata for display purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletMetadata {
    pub name: String,
    pub address: String,
    pub created_at: String,
    pub last_accessed: Option<String>,
}

/// Main wallet storage manager
pub struct WalletStorage {
    /// Directory where wallets are stored
    storage_dir: PathBuf,
}

impl WalletStorage {
    /// Create a new WalletStorage instance
    pub fn new() -> Result<Self, Error> {
        let storage_dir = Self::get_storage_directory()?;
        
        // Create directory if it doesn't exist
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)
                .map_err(|e| Error::Wallet(format!("Failed to create storage directory: {}", e)))?;
        }

        Ok(Self { storage_dir })
    }

    /// Get the default storage directory (~/.mantra_dex/wallets/)
    pub fn get_storage_directory() -> Result<PathBuf, Error> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| Error::Wallet("Could not determine home directory".to_string()))?;
        
        Ok(home_dir.join(".mantra_dex").join("wallets"))
    }

    /// Check if any wallets are saved
    pub fn has_saved_wallets(&self) -> Result<bool, Error> {
        if !self.storage_dir.exists() {
            return Ok(false);
        }

        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| Error::Wallet(format!("Failed to read storage directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| Error::Wallet(format!("Failed to read directory entry: {}", e)))?;
            
            if let Some(extension) = entry.path().extension() {
                if extension == "wallet" {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// List all saved wallet metadata
    pub fn list_wallets(&self) -> Result<Vec<WalletMetadata>, Error> {
        let mut wallets = Vec::new();

        if !self.storage_dir.exists() {
            return Ok(wallets);
        }

        let entries = fs::read_dir(&self.storage_dir)
            .map_err(|e| Error::Wallet(format!("Failed to read storage directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| Error::Wallet(format!("Failed to read directory entry: {}", e)))?;
            
            if let Some(extension) = entry.path().extension() {
                if extension == "wallet" {
                    // Try to load and decrypt wallet metadata
                    if let Ok(wallet_data) = self.load_wallet_file(&entry.path()) {
                        wallets.push(wallet_data.metadata);
                    }
                }
            }
        }

        Ok(wallets)
    }

    /// Save a wallet with encryption
    pub fn save_wallet(
        &self,
        name: &str,
        mnemonic: &str,
        password: &str,
        address: &str,
    ) -> Result<(), Error> {
        // Validate password strength
        self.validate_password(password)?;

        // Generate salt for Argon2
        let salt = SaltString::generate(&mut OsRng);
        
        // Hash password with Argon2
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::Wallet(format!("Failed to hash password: {}", e)))?
            .to_string();

        // Derive encryption key from password hash
        let key = self.derive_key_from_hash(&password_hash)?;

        // Generate random nonce for AES-GCM
        let cipher = Aes256Gcm::new(&key);
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the mnemonic
        let encrypted_mnemonic = cipher
            .encrypt(nonce, mnemonic.as_bytes())
            .map_err(|e| Error::Wallet(format!("Failed to encrypt mnemonic: {}", e)))?;

        // Create wallet metadata
        let metadata = WalletMetadata {
            name: name.to_string(),
            address: address.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_accessed: None,
        };

        // Create encrypted wallet data
        let wallet_data = EncryptedWalletData {
            password_hash,
            encrypted_mnemonic,
            nonce: nonce_bytes.to_vec(),
            metadata,
        };

        // Serialize and save to file
        let wallet_json = serde_json::to_string_pretty(&wallet_data)
            .map_err(|e| Error::Wallet(format!("Failed to serialize wallet data: {}", e)))?;

        let wallet_path = self.storage_dir.join(format!("{}.wallet", name));
        fs::write(&wallet_path, wallet_json)
            .map_err(|e| Error::Wallet(format!("Failed to write wallet file: {}", e)))?;

        Ok(())
    }

    /// Load and decrypt a wallet
    pub fn load_wallet(&self, name: &str, password: &str) -> Result<String, Error> {
        let wallet_path = self.storage_dir.join(format!("{}.wallet", name));
        
        if !wallet_path.exists() {
            return Err(Error::Wallet(format!("Wallet '{}' not found", name)));
        }

        let wallet_data = self.load_wallet_file(&wallet_path)?;

        // Verify password
        let parsed_hash = PasswordHash::new(&wallet_data.password_hash)
            .map_err(|e| Error::Wallet(format!("Failed to parse password hash: {}", e)))?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| Error::Wallet("Invalid password".to_string()))?;

        // Derive decryption key
        let key = self.derive_key_from_hash(&wallet_data.password_hash)?;

        // Decrypt mnemonic
        let cipher = Aes256Gcm::new(&key);
        let nonce = Nonce::from_slice(&wallet_data.nonce);

        let decrypted_bytes = cipher
            .decrypt(nonce, wallet_data.encrypted_mnemonic.as_ref())
            .map_err(|e| Error::Wallet(format!("Failed to decrypt mnemonic: {}", e)))?;

        let mnemonic = String::from_utf8(decrypted_bytes)
            .map_err(|e| Error::Wallet(format!("Invalid mnemonic data: {}", e)))?;

        // Update last accessed time
        self.update_last_accessed(name)?;

        Ok(mnemonic)
    }

    /// Delete a saved wallet
    pub fn delete_wallet(&self, name: &str) -> Result<(), Error> {
        let wallet_path = self.storage_dir.join(format!("{}.wallet", name));
        
        if !wallet_path.exists() {
            return Err(Error::Wallet(format!("Wallet '{}' not found", name)));
        }

        fs::remove_file(&wallet_path)
            .map_err(|e| Error::Wallet(format!("Failed to delete wallet file: {}", e)))?;

        Ok(())
    }

    /// Load wallet file and deserialize
    fn load_wallet_file(&self, path: &std::path::Path) -> Result<EncryptedWalletData, Error> {
        let wallet_content = fs::read_to_string(path)
            .map_err(|e| Error::Wallet(format!("Failed to read wallet file: {}", e)))?;

        let wallet_data: EncryptedWalletData = serde_json::from_str(&wallet_content)
            .map_err(|e| Error::Wallet(format!("Failed to parse wallet file: {}", e)))?;

        Ok(wallet_data)
    }

    /// Derive encryption key from password hash
    fn derive_key_from_hash(&self, password_hash: &str) -> Result<GenericArray<u8, aes_gcm::aes::cipher::typenum::U32>, Error> {
        // Use the first 32 bytes of the password hash as the key
        let hash_bytes = password_hash.as_bytes();
        let mut key_bytes = [0u8; 32];
        
        if hash_bytes.len() >= 32 {
            key_bytes.copy_from_slice(&hash_bytes[..32]);
        } else {
            key_bytes[..hash_bytes.len()].copy_from_slice(hash_bytes);
        }

        Ok(*GenericArray::from_slice(&key_bytes))
    }

    /// Update last accessed time for a wallet
    fn update_last_accessed(&self, name: &str) -> Result<(), Error> {
        let wallet_path = self.storage_dir.join(format!("{}.wallet", name));
        let mut wallet_data = self.load_wallet_file(&wallet_path)?;
        
        wallet_data.metadata.last_accessed = Some(chrono::Utc::now().to_rfc3339());
        
        let wallet_json = serde_json::to_string_pretty(&wallet_data)
            .map_err(|e| Error::Wallet(format!("Failed to serialize wallet data: {}", e)))?;

        fs::write(&wallet_path, wallet_json)
            .map_err(|e| Error::Wallet(format!("Failed to update wallet file: {}", e)))?;

        Ok(())
    }

    /// Validate password strength
    pub fn validate_password(&self, password: &str) -> Result<(), Error> {
        if password.len() < 12 {
            return Err(Error::Wallet("Password must be at least 12 characters long".to_string()));
        }

        let has_upper = password.chars().any(|c| c.is_uppercase());
        let has_lower = password.chars().any(|c| c.is_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_symbol = password.chars().any(|c| !c.is_alphanumeric());

        if !has_upper {
            return Err(Error::Wallet("Password must contain at least one uppercase letter".to_string()));
        }
        if !has_lower {
            return Err(Error::Wallet("Password must contain at least one lowercase letter".to_string()));
        }
        if !has_digit {
            return Err(Error::Wallet("Password must contain at least one number".to_string()));
        }
        if !has_symbol {
            return Err(Error::Wallet("Password must contain at least one symbol".to_string()));
        }

        Ok(())
    }
}

impl Default for WalletStorage {
    fn default() -> Self {
        Self::new().expect("Failed to initialize WalletStorage")
    }
} 