use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use mantra_dex_sdk::config::MantraNetworkConfig;
use serde::{Deserialize, Serialize};

use crate::error::CliError;
use crate::utils::encryption::{decrypt, encrypt, EncryptedData};

/// CLI Configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CliConfig {
    /// Active network configuration
    pub network: MantraNetworkConfig,

    /// Wallet configurations - stored in encrypted format
    pub wallets: HashMap<String, EncryptedData>,

    /// Active wallet name
    pub active_wallet: Option<String>,

    /// Known tokens and their metadata
    pub tokens: HashMap<String, TokenInfo>,

    /// Session password - not stored in the config file, only kept in memory
    #[serde(skip)]
    pub session_password: Option<String>,
}

/// Token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    /// Token name
    pub name: String,

    /// Token symbol
    pub symbol: String,

    /// Token decimals
    pub decimals: u8,

    /// Token logo URL
    pub logo: Option<String>,
}

impl CliConfig {
    /// Load configuration from a file
    pub fn load(path: &PathBuf) -> Result<Self, CliError> {
        let content = fs::read_to_string(path).map_err(|e| CliError::Io(e))?;

        let config: CliConfig = toml::from_str(&content).map_err(|e| CliError::TomlDe(e))?;

        Ok(config)
    }

    /// Save configuration to a file
    pub fn save(&self, path: &PathBuf) -> Result<(), CliError> {
        let content = toml::to_string_pretty(self).map_err(|e| CliError::Toml(e))?;

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, content)?;
        Ok(())
    }

    /// Get the default configuration file path
    pub fn default_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("mantra-dex-cli");
        path.push("config.toml");
        path
    }

    /// Add a wallet with a name and mnemonic, encrypted with a password
    pub fn add_wallet(
        &mut self,
        name: &str,
        mnemonic: &str,
        password: &str,
    ) -> Result<(), CliError> {
        if self.wallets.contains_key(name) {
            return Err(CliError::Wallet(format!(
                "Wallet '{}' already exists",
                name
            )));
        }

        // Encrypt the mnemonic
        let encrypted = encrypt(mnemonic, password)?;

        self.wallets.insert(name.to_string(), encrypted);

        // If this is the first wallet, set it as active
        if self.active_wallet.is_none() {
            self.active_wallet = Some(name.to_string());
        }

        Ok(())
    }

    /// Remove a wallet by name
    pub fn remove_wallet(&mut self, name: &str) -> Result<(), CliError> {
        if !self.wallets.contains_key(name) {
            return Err(CliError::Wallet(format!("Wallet '{}' not found", name)));
        }

        self.wallets.remove(name);

        // If the active wallet was removed, clear it
        if let Some(active) = &self.active_wallet {
            if active == name {
                self.active_wallet = None;
            }
        }

        Ok(())
    }

    /// Set the active wallet
    pub fn set_active_wallet(&mut self, name: &str) -> Result<(), CliError> {
        if !self.wallets.contains_key(name) {
            return Err(CliError::Wallet(format!("Wallet '{}' not found", name)));
        }

        self.active_wallet = Some(name.to_string());
        Ok(())
    }

    /// Get a wallet mnemonic by name (requires password to decrypt)
    pub fn get_wallet_mnemonic(&self, name: &str, password: &str) -> Result<String, CliError> {
        let encrypted = self
            .wallets
            .get(name)
            .ok_or_else(|| CliError::Wallet(format!("Wallet '{}' not found", name)))?;

        decrypt(encrypted, password)
    }

    /// Check if a wallet password is correct
    pub fn verify_wallet_password(&self, name: &str, password: &str) -> Result<bool, CliError> {
        match self.get_wallet_mnemonic(name, password) {
            Ok(_) => Ok(true),
            Err(CliError::DecryptionFailed) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Store the password in the session cache
    pub fn store_session_password(&mut self, password: &str) {
        self.session_password = Some(password.to_string());
    }

    /// Clear the session password
    pub fn clear_session_password(&mut self) {
        self.session_password = None;
    }

    /// Get the session password, if available
    pub fn get_session_password(&self) -> Option<&str> {
        self.session_password.as_deref()
    }
}
