use clap::{Args, Subcommand};
use mantra_dex_sdk::{MantraWallet, NetworkConstants};
use dialoguer::{Confirm, Password};

use crate::config::CliConfig;
use crate::error::CliError;
use crate::utils::{print_success, print_table};
use colored::Colorize;

#[derive(Args, Clone)]
pub struct WalletCommand {
    #[command(subcommand)]
    command: WalletCommands,
}

#[derive(Subcommand, Clone)]
enum WalletCommands {
    /// Create a new wallet
    Create {
        /// Wallet name
        #[arg(short, long)]
        name: String,
    },
    
    /// Import an existing wallet from mnemonic
    Import {
        /// Wallet name
        #[arg(short, long)]
        name: String,
        
        /// Mnemonic phrase (if not provided, will prompt for input)
        #[arg(short, long)]
        mnemonic: Option<String>,
    },
    
    /// List all wallets
    List,
    
    /// Get details of a specific wallet
    Info {
        /// Wallet name (if not provided, uses active wallet)
        #[arg(short, long)]
        name: Option<String>,
    },
    
    /// Set a wallet as the active wallet
    Use {
        /// Wallet name
        name: String,
    },
    
    /// Export wallet mnemonic
    Export {
        /// Wallet name (if not provided, uses active wallet)
        #[arg(short, long)]
        name: Option<String>,
    },
    
    /// Remove a wallet
    Remove {
        /// Wallet name
        name: String,
        
        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },
}

/// Helper function to prompt for a password
fn prompt_for_password(confirm: bool) -> Result<String, CliError> {
    let prompt = Password::new().with_prompt("Enter password for wallet encryption");
    
    let password = if confirm {
        prompt.with_confirmation("Confirm password", "Passwords don't match")
            .interact()
            .map_err(|e| CliError::Command(format!("Input error: {}", e)))?
    } else {
        prompt.interact().map_err(|e| CliError::Command(format!("Input error: {}", e)))?
    };
    
    // Validate password length
    if password.len() < 8 {
        return Err(CliError::Command("Password must be at least 8 characters long".to_string()));
    }
    
    Ok(password)
}

/// Helper function to unlock a wallet with password
fn unlock_wallet(config: &CliConfig, name: &str) -> Result<(MantraWallet, String), CliError> {
    // Try to use the session password first if available
    let password = if let Some(session_pwd) = config.get_session_password() {
        // Verify the session password works for this wallet
        if config.verify_wallet_password(name, session_pwd)? {
            session_pwd.to_string()
        } else {
            // Session password doesn't work for this wallet, prompt for a new one
            Password::new()
                .with_prompt(&format!("Enter password to unlock wallet '{}'", name))
                .interact()
                .map_err(|e| CliError::Command(format!("Input error: {}", e)))?
        }
    } else {
        // No session password, prompt for one
        Password::new()
            .with_prompt(&format!("Enter password to unlock wallet '{}'", name))
            .interact()
            .map_err(|e| CliError::Command(format!("Input error: {}", e)))?
    };
        
    // Get the mnemonic - this will validate the password
    let mnemonic = config.get_wallet_mnemonic(name, &password)?;
    let network_constants = load_network_constants(&config)?;
    let wallet = MantraWallet::from_mnemonic(&mnemonic, 0, &network_constants)
        .map_err(CliError::Sdk)?;
        
    Ok((wallet, password))
}

impl WalletCommand {
    pub async fn execute(self, mut config: CliConfig) -> Result<(), CliError> {
        let config_path = CliConfig::default_path();
        
        match self.command {
            WalletCommands::Create { name } => {
                // Generate a new wallet
                let network_constants = load_network_constants(&config)?;
                let (wallet, mnemonic) = MantraWallet::generate(&network_constants)
                    .map_err(CliError::Sdk)?;
                
                // Show the mnemonic to the user
                println!("\n{}\n", "Your mnemonic phrase (KEEP THIS SAFE):".yellow());
                println!("{}\n", mnemonic);
                
                // Confirm the user has saved the mnemonic
                if !Confirm::new()
                    .with_prompt("Have you saved your mnemonic phrase in a safe place?")
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    return Err(CliError::Command(
                        "Wallet creation cancelled - please save your mnemonic before continuing".to_string()
                    ));
                }
                
                // Get password for encryption
                let password = prompt_for_password(true)?;
                
                // Store the password in the session
                config.store_session_password(&password);
                
                // Add wallet to config
                config.add_wallet(&name, &mnemonic, &password)?;
                config.save(&config_path)?;
                
                print_success(&format!("Wallet '{}' created, encrypted, and set as active", name));
                
                // Show wallet info
                let address = wallet.address()
                    .map_err(CliError::Sdk)?
                    .to_string();
                
                println!("\nWallet address: {}", address);
                Ok(())
            },
            
            WalletCommands::Import { name, mnemonic } => {
                // Get mnemonic from args or prompt
                let mnemonic = match mnemonic {
                    Some(m) => m,
                    None => {
                        Password::new()
                            .with_prompt("Enter your mnemonic phrase")
                            .interact()
                            .map_err(|e| CliError::Command(format!("Input error: {}", e)))?
                    }
                };

                // Validate mnemonic by creating a wallet
                let network_constants = load_network_constants(&config)?;
                let wallet = MantraWallet::from_mnemonic(&mnemonic, 0, &network_constants)
                    .map_err(|e| CliError::Wallet(format!("Invalid mnemonic: {}", e)))?;
                
                // Get password for encryption
                let password = prompt_for_password(true)?;
                
                // Store the password in the session
                config.store_session_password(&password);
                
                // Add wallet to config
                config.add_wallet(&name, &mnemonic, &password)?;
                config.save(&config_path)?;
                
                print_success(&format!("Wallet '{}' imported, encrypted, and set as active", name));
                
                // Show wallet info
                let address = wallet.address()
                    .map_err(CliError::Sdk)?
                    .to_string();
                
                println!("\nWallet address: {}", address);
                Ok(())
            },
            
            WalletCommands::List => {
                if config.wallets.is_empty() {
                    println!("No wallets configured. Use 'wallet create' or 'wallet import' to add a wallet.");
                    return Ok(());
                }
                
                let mut rows = Vec::new();
                
                // Show wallet list (addresses require decryption)
                for (name, _) in &config.wallets {
                    let address = if let Some(active) = config.active_wallet.as_ref() {
                        if active == name {
                            // For active wallet, prompt for password to show address
                            match unlock_wallet(&config, name) {
                                Ok((wallet, _)) => {
                                    wallet.address()
                                        .map_err(CliError::Sdk)?
                                        .to_string()
                                },
                                Err(_) => "*** LOCKED ***".to_string()
                            }
                        } else {
                            "*** LOCKED ***".to_string()
                        }
                    } else {
                        "*** LOCKED ***".to_string()
                    };
                    
                    let active = if Some(name) == config.active_wallet.as_ref() {
                        "✓".green().to_string()
                    } else {
                        "".to_string()
                    };
                    
                    rows.push(vec![
                        active,
                        name.to_string(),
                        address,
                    ]);
                }
                
                print_table(vec!["Active", "Name", "Address"], rows);
                Ok(())
            },
            
            WalletCommands::Info { name } => {
                let wallet_name = match name {
                    Some(n) => n,
                    None => config.active_wallet.clone()
                        .ok_or_else(|| CliError::Wallet("No active wallet set".to_string()))?,
                };
                
                // Unlock the wallet
                let (wallet, password) = unlock_wallet(&config, &wallet_name)?;
                
                // Store the password in the session
                config.store_session_password(&password);
                
                let info = wallet.info();
                
                println!("\nWallet: {}", wallet_name);
                println!("Address: {}", info.address);
                println!("Public Key: {}", info.public_key);
                
                Ok(())
            },
            
            WalletCommands::Use { name } => {
                // Check if wallet exists
                if !config.wallets.contains_key(&name) {
                    return Err(CliError::Wallet(format!("Wallet '{}' not found", name)));
                }
                
                // Verify wallet password before setting it as active
                let password = if let Some(session_pwd) = config.get_session_password() {
                    if config.verify_wallet_password(&name, session_pwd)? {
                        session_pwd.to_string()
                    } else {
                        // Session password doesn't work for this wallet, prompt for new one
                        prompt_for_password(false)?
                    }
                } else {
                    // No session password, prompt for one
                    Password::new()
                        .with_prompt(&format!("Enter password to unlock wallet '{}'", name))
                        .interact()
                        .map_err(|e| CliError::Command(format!("Input error: {}", e)))?
                };
                
                // Make sure the password works
                if !config.verify_wallet_password(&name, &password)? {
                    return Err(CliError::Wallet("Invalid password".to_string()));
                }
                
                // Store the password in the session
                config.store_session_password(&password);
                
                // Set the wallet as active
                config.set_active_wallet(&name)?;
                config.save(&config_path)?;
                
                print_success(&format!("Wallet '{}' set as active", name));
                Ok(())
            },
            
            WalletCommands::Export { name } => {
                let wallet_name = match name {
                    Some(n) => n,
                    None => config.active_wallet.clone()
                        .ok_or_else(|| CliError::Wallet("No active wallet set".to_string()))?,
                };
                
                // Security check - require confirmation
                if !Confirm::new()
                    .with_prompt("WARNING: You are about to display your private mnemonic. Make sure no one can see your screen. Continue?")
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    return Ok(());
                }
                
                // Unlock the wallet
                let (_, mnemonic) = unlock_wallet(&config, &wallet_name)?;
                
                // Store the password in the session if not already stored
                if config.get_session_password().is_none() {
                    // Get the password that was just used
                    let password = Password::new()
                        .with_prompt(&format!("Enter the same password again to store in session"))
                        .interact()
                        .map_err(|e| CliError::Command(format!("Input error: {}", e)))?;
                        
                    // Verify it works before storing
                    if config.verify_wallet_password(&wallet_name, &password)? {
                        config.store_session_password(&password);
                    }
                }
                
                println!("\n{}: {}\n", "Mnemonic for wallet".yellow(), wallet_name);
                println!("{}\n", mnemonic);
                
                Ok(())
            },
            
            WalletCommands::Remove { name, yes } => {
                // Confirm removal
                if !yes && !Confirm::new()
                    .with_prompt(&format!("Are you sure you want to remove wallet '{}'?", name))
                    .default(false)
                    .interact()
                    .unwrap_or(false)
                {
                    return Ok(());
                }
                
                // Verify the password for security
                let password = if let Some(session_pwd) = config.get_session_password() {
                    if config.verify_wallet_password(&name, session_pwd)? {
                        session_pwd.to_string()
                    } else {
                        // Session password doesn't work for this wallet, prompt for it
                        Password::new()
                            .with_prompt(&format!("Enter password for wallet '{}' to confirm removal", name))
                            .interact()
                            .map_err(|e| CliError::Command(format!("Input error: {}", e)))?
                    }
                } else {
                    // No session password, prompt for one
                    Password::new()
                        .with_prompt(&format!("Enter password for wallet '{}' to confirm removal", name))
                        .interact()
                        .map_err(|e| CliError::Command(format!("Input error: {}", e)))?
                };
                
                // Verify the password
                if !config.verify_wallet_password(&name, &password)? {
                    return Err(CliError::Wallet("Invalid password, removal cancelled".to_string()));
                }
                
                config.remove_wallet(&name)?;
                config.save(&config_path)?;
                
                // If this was the active wallet, clear the session password
                if config.active_wallet.is_none() {
                    config.clear_session_password();
                }
                
                print_success(&format!("Wallet '{}' removed", name));
                Ok(())
            },
        }
    }

    
} 

pub fn load_network_constants(config: &CliConfig) -> Result<NetworkConstants, CliError> {
    let network_name: String = config.network.network_name.clone();
    let network_id: String = config.network.network_id.clone();
    let default_rpc: String = config.network.rpc_url.clone();
    let default_gas_price: f64 = config.network.gas_price;
    let default_gas_adjustment: f64 = config.network.gas_adjustment;
    let native_denom: String = config.network.native_denom.clone();
    Ok(NetworkConstants::new(network_name, network_id, default_rpc, default_gas_price, default_gas_adjustment, native_denom))
}