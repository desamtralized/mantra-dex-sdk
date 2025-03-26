use colored::Colorize;
use mantra_dex_sdk::{MantraDexClient, MantraWallet};
use prettytable::{Cell, Row, Table};
use std::str::FromStr;

use crate::commands::wallet::load_network_constants;
use crate::config::CliConfig;
use crate::error::CliError;

/// Print a success message
pub fn print_success(message: &str) {
    println!("{} {}", "✓".green(), message);
}

/// Print a table with data
pub fn print_table(headers: Vec<&str>, rows: Vec<Vec<String>>) {
    let mut table = Table::new();

    // Add headers
    table.set_titles(Row::new(
        headers.iter().map(|h| Cell::new(h.as_ref())).collect(),
    ));

    // Add data rows
    for row_data in rows {
        table.add_row(Row::new(row_data.iter().map(|c| Cell::new(&c)).collect()));
    }

    table.printstd();
}

/// Parse a coin string in the format "amount:denom" (e.g., "100:uom")
pub fn parse_coin(coin_str: &str) -> Result<mantra_dex_sdk::Coin, CliError> {
    let parts: Vec<&str> = coin_str.split(':').collect();
    if parts.len() != 2 {
        return Err(CliError::Parse(format!(
            "Invalid coin format, expected 'amount:denom' but got '{}'",
            coin_str
        )));
    }

    let amount_str = parts[0].trim();
    let denom = parts[1].trim();

    // Parse amount
    let amount = mantra_dex_sdk::Uint128::from_str(amount_str)
        .map_err(|_| CliError::Parse(format!("Invalid amount: {}", amount_str)))?;

    Ok(mantra_dex_sdk::Coin {
        denom: denom.to_string(),
        amount,
    })
}

/// Parse a decimal string (e.g., "0.01" for 1%)
pub fn parse_decimal(decimal_str: &str) -> Result<mantra_dex_sdk::Decimal, CliError> {
    mantra_dex_sdk::Decimal::from_str(decimal_str)
        .map_err(|e| CliError::Parse(format!("Invalid decimal: {}", e)))
}

/// Create a client instance from the config
pub async fn create_client(config: &CliConfig) -> Result<MantraDexClient, CliError> {
    let client = MantraDexClient::new(config.network.clone())
        .await
        .map_err(CliError::Sdk)?;

    // Skip wallet loading for read-only operations
    if config.session_password.is_none() {
        // If we don't have a session password, this is likely a read-only operation
        return Ok(client);
    }

    // If an active wallet is set, load it
    if let Some(active_wallet) = &config.active_wallet {
        if config.wallets.contains_key(active_wallet) {
            // Try to use session password first, or prompt if not available
            let password = if let Some(session_pwd) = config.get_session_password() {
                // Verify the session password works for this wallet
                if config.verify_wallet_password(active_wallet, session_pwd)? {
                    session_pwd.to_string()
                } else {
                    // Session password doesn't work for this wallet, prompt for a new one
                    let new_pwd = dialoguer::Password::new()
                        .with_prompt(&format!(
                            "Enter password to unlock wallet '{}'",
                            active_wallet
                        ))
                        .interact()
                        .map_err(|e| CliError::Command(format!("Password input error: {}", e)))?;

                    new_pwd
                }
            } else {
                // Prompt for password
                let new_pwd = dialoguer::Password::new()
                    .with_prompt(&format!(
                        "Enter password to unlock wallet '{}'",
                        active_wallet
                    ))
                    .interact()
                    .map_err(|e| CliError::Command(format!("Password input error: {}", e)))?;

                new_pwd
            };

            // Get the mnemonic
            let mnemonic = config.get_wallet_mnemonic(active_wallet, &password)?;
            let network_constants = load_network_constants(&config)?;
            let wallet = MantraWallet::from_mnemonic(&mnemonic, 0, &network_constants)
                .map_err(CliError::Sdk)?;

            return Ok(client.with_wallet(wallet));
        }
    }

    Ok(client)
}

/// Format an amount with token decimals for display
pub fn format_amount(amount: mantra_dex_sdk::Uint128, denom: &str, config: &CliConfig) -> String {
    let decimals = config.tokens.get(denom).map(|t| t.decimals).unwrap_or(6); // Default to 6 decimals if not found

    let amount_f64 = amount.u128() as f64 / 10_f64.powi(decimals as i32);
    format!("{:.6} {}", amount_f64, denom)
}

/// Prompt for and store the password for the active wallet if needed
pub fn ensure_session_password(config: &mut CliConfig) -> Result<(), CliError> {
    if let Some(active_wallet) = &config.active_wallet {
        if config.wallets.contains_key(active_wallet) {
            // Check if we already have a session password that works
            if let Some(session_pwd) = config.get_session_password() {
                if config.verify_wallet_password(active_wallet, session_pwd)? {
                    // We already have a working password
                    return Ok(());
                }
            }

            // We need to get a new password - loop until correct or user cancels
            println!("Unlock wallet '{}' (press Ctrl+C to cancel)", active_wallet);

            loop {
                let password = match dialoguer::Password::new()
                    .with_prompt("Enter password")
                    .interact()
                {
                    Ok(pwd) => pwd,
                    Err(e) => {
                        // Handle user interruption (Ctrl+C)
                        match &e {
                            dialoguer::Error::IO(io_err)
                                if io_err.kind() == std::io::ErrorKind::Interrupted =>
                            {
                                println!("Operation cancelled");
                                return Err(CliError::Command(
                                    "Password entry cancelled".to_string(),
                                ));
                            }
                            _ => {
                                return Err(CliError::Command(format!(
                                    "Password input error: {}",
                                    e
                                )))
                            }
                        }
                    }
                };

                // Verify it works
                if config.verify_wallet_password(active_wallet, &password)? {
                    // Store it in the session
                    config.store_session_password(&password);
                    return Ok(());
                } else {
                    // Password is incorrect, prompt again
                    println!("{}", "Invalid password, please try again".red());
                }
            }
        }
    }

    Ok(())
}

pub mod encryption {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
    use base64::{engine::general_purpose, Engine as _};
    use chacha20poly1305::{
        aead::{Aead, KeyInit, OsRng},
        ChaCha20Poly1305, Nonce,
    };
    use rand_core::RngCore;
    use std::str;

    use crate::error::CliError;

    // Length of nonce in bytes
    const NONCE_LEN: usize = 12;
    // Length of salt in bytes
    const SALT_LEN: usize = 16;

    /// Structure to hold encrypted data with all parameters needed for decryption
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct EncryptedData {
        /// Base64 encoded encrypted data
        pub ciphertext: String,
        /// Base64 encoded nonce
        pub nonce: String,
        /// Base64 encoded salt used for key derivation
        pub salt: String,
    }

    /// Encrypts data with a password
    pub fn encrypt(data: &str, password: &str) -> Result<EncryptedData, CliError> {
        // Generate a random salt for Argon2
        let mut salt_bytes = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt_bytes);
        let salt = SaltString::encode_b64(&salt_bytes)
            .map_err(|e| CliError::Command(format!("Failed to encode salt: {}", e)))?;

        // Generate a random nonce for ChaCha20Poly1305
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Derive key from password using Argon2
        let argon2 = Argon2::default();
        let key = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| CliError::Command(format!("Password hashing failed: {}", e)))?
            .hash
            .ok_or_else(|| CliError::Command("Failed to generate key".to_string()))?
            .as_bytes()
            .to_vec();

        // Encrypt the data
        let cipher = ChaCha20Poly1305::new_from_slice(&key[0..32])
            .map_err(|e| CliError::Command(format!("Failed to create cipher: {}", e)))?;

        let ciphertext = cipher
            .encrypt(nonce, data.as_bytes())
            .map_err(|e| CliError::Command(format!("Encryption failed: {}", e)))?;

        Ok(EncryptedData {
            ciphertext: general_purpose::STANDARD.encode(ciphertext),
            nonce: general_purpose::STANDARD.encode(nonce),
            salt: salt.to_string(),
        })
    }

    /// Decrypts data with a password
    pub fn decrypt(encrypted: &EncryptedData, password: &str) -> Result<String, CliError> {
        // Decode the base64 elements
        let ciphertext = general_purpose::STANDARD
            .decode(&encrypted.ciphertext)
            .map_err(|e| CliError::Command(format!("Failed to decode ciphertext: {}", e)))?;

        let nonce_bytes = general_purpose::STANDARD
            .decode(&encrypted.nonce)
            .map_err(|e| CliError::Command(format!("Failed to decode nonce: {}", e)))?;

        let nonce = Nonce::from_slice(&nonce_bytes);
        let salt = encrypted.salt.as_str();

        // Derive key from password using Argon2
        let argon2 = Argon2::default();
        let parsed_salt = SaltString::from_b64(salt)
            .map_err(|e| CliError::Command(format!("Failed to parse salt: {}", e)))?;

        let key = argon2
            .hash_password(password.as_bytes(), &parsed_salt)
            .map_err(|e| CliError::Command(format!("Password hashing failed: {}", e)))?
            .hash
            .ok_or_else(|| CliError::Command("Failed to generate key".to_string()))?
            .as_bytes()
            .to_vec();

        // Decrypt the data
        let cipher = ChaCha20Poly1305::new_from_slice(&key[0..32])
            .map_err(|e| CliError::Command(format!("Failed to create cipher: {}", e)))?;

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| CliError::DecryptionFailed)?;

        String::from_utf8(plaintext)
            .map_err(|e| CliError::Command(format!("Failed to decode plaintext: {}", e)))
    }
}
