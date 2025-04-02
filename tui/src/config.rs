use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::{Result, TuiError};

/// Structure to hold encrypted wallet data
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedWallet {
    /// Base64 encoded encrypted data
    pub ciphertext: String,
    /// Base64 encoded nonce
    pub nonce: String,
    /// Base64 encoded salt used for key derivation
    pub salt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TuiConfig {
    #[serde(default)]
    pub wallets: HashMap<String, EncryptedWallet>,
    
    #[serde(default)]
    pub active_wallet: Option<String>,
    
    #[serde(default)]
    pub network: NetworkConfig,
    
    // Session password is not serialized
    #[serde(skip)]
    session_password: Option<String>,
    
    #[serde(default)]
    pub tokens: HashMap<String, TokenInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct NetworkConfig {
    #[serde(default = "default_network_name")]
    pub network_name: String,
    
    #[serde(default = "default_network_id")]
    pub network_id: String,
    
    #[serde(default = "default_rpc_url")]
    pub rpc_url: String,
    
    #[serde(default = "default_gas_price")]
    pub gas_price: f64,
    
    #[serde(default = "default_gas_adjustment")]
    pub gas_adjustment: f64,
    
    #[serde(default = "default_native_denom")]
    pub native_denom: String,
    
    #[serde(default)]
    pub contracts: ContractsConfig,
}

fn default_network_name() -> String {
    "mantra-dukong".to_string()
}

fn default_network_id() -> String {
    "mantra-dukong-1".to_string()
}

fn default_rpc_url() -> String {
    "https://rpc.dukong.mantrachain.io/".to_string()
}

fn default_gas_price() -> f64 {
    0.025
}

fn default_gas_adjustment() -> f64 {
    1.3
}

fn default_native_denom() -> String {
    "uom".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ContractsConfig {
    #[serde(default = "default_pool_manager")]
    pub pool_manager: String,
}

fn default_pool_manager() -> String {
    "mantra1us7rryvauhpe82fff0t6gjthdraqmtm5gw8c808f6eqzuxmulacqzkzdal".to_string()
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            wallets: HashMap::new(),
            active_wallet: None,
            network: NetworkConfig::default(),
            session_password: None,
            tokens: HashMap::new(),
        }
    }
}

impl TuiConfig {
    pub fn default_path() -> PathBuf {
        let mut path = config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("mantra-dex-tui");
        path.push("config.toml");
        path
    }

    pub fn load(path: &PathBuf) -> Result<Self> {
        // Check if the config file exists
        if !path.exists() {
            // Create default config
            let config = TuiConfig::default();
            
            // Create parent directories if they don't exist
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    TuiError::Config(format!("Failed to create config directory: {}", e))
                })?;
            }
            
            // Save the default config
            config.save(path)?;
            return Ok(config);
        }

        // Read the config file
        let mut file = File::open(path).map_err(|e| {
            TuiError::Config(format!("Failed to open config file: {}", e))
        })?;
        
        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|e| {
            TuiError::Config(format!("Failed to read config file: {}", e))
        })?;

        // Parse the TOML
        let config: TuiConfig = toml::from_str(&contents).map_err(|e| {
            TuiError::Config(format!("Failed to parse config file: {}", e))
        })?;

        Ok(config)
    }

    pub fn save(&self, path: &PathBuf) -> Result<()> {
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                TuiError::Config(format!("Failed to create config directory: {}", e))
            })?;
        }

        // Serialize the config to TOML
        let toml = toml::to_string(self).map_err(|e| {
            TuiError::Config(format!("Failed to serialize config: {}", e))
        })?;

        // Write to file
        let mut file = File::create(path).map_err(|e| {
            TuiError::Config(format!("Failed to create config file: {}", e))
        })?;
        
        file.write_all(toml.as_bytes()).map_err(|e| {
            TuiError::Config(format!("Failed to write config file: {}", e))
        })?;

        Ok(())
    }

    pub fn get_session_password(&self) -> Option<&str> {
        self.session_password.as_deref()
    }
} 