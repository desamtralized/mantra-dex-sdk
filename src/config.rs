use config::{Config as ConfigLoader, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::error::Error;

/// Network constants loaded from configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConstants {
    /// Network name
    pub network_name: String,
    /// Network ID
    pub network_id: String,
    /// Default RPC endpoint
    pub default_rpc: String,
    /// Default gas price (in uaum)
    pub default_gas_price: f64,
    /// Default gas adjustment
    pub default_gas_adjustment: f64,
    /// Native token denom
    pub native_denom: String,
}

impl NetworkConstants {
    /// Load network constants from the configuration file
    pub fn load(network: &str) -> Result<Self, ConfigError> {
        let config_dir = env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let settings = ConfigLoader::builder()
            // Add the config file
            .add_source(File::with_name(&format!("{}/network", config_dir)))
            .build()?;

        // Extract the network section
        settings.get::<NetworkConstants>(network)
    }

    /// Get the default Mantra Dukong network constants
    pub fn default_dukong() -> Result<Self, ConfigError> {
        Self::load("mantra-dukong")
    }
}

/// Contract address configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractAddresses {
    /// Pool manager contract address
    pub pool_manager: String,
    /// Farm manager contract address
    pub farm_manager: Option<String>,
    /// Fee collector contract address
    pub fee_collector: Option<String>,
    /// Epoch manager contract address
    pub epoch_manager: Option<String>,
}

impl Default for ContractAddresses {
    fn default() -> Self {
        Self {
            pool_manager: "".to_string(),
            farm_manager: None,
            fee_collector: None,
            epoch_manager: None,
        }
    }
}

/// Network configuration for Mantra DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraNetworkConfig {
    /// Network name (e.g., mantra-dukong)
    pub network_name: String,
    /// Network ID (e.g., mantra-dukong-1)
    pub network_id: String,
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Gas price in native token
    pub gas_price: f64,
    /// Gas adjustment for transactions
    pub gas_adjustment: f64,
    /// Native token denom
    pub native_denom: String,
    /// Time to wait in ms after a transaction is broadcast
    pub tx_delay_ms: u64,
    /// Contract addresses
    pub contracts: ContractAddresses,
}

impl MantraNetworkConfig {
    /// Create a new network config from network constants
    pub fn from_constants(constants: &NetworkConstants) -> Self {
        // Attempt to load contract addresses for this network from `config/contracts.toml`
        let contracts = Self::load_contract_addresses(&constants.network_name);

        Self {
            network_name: constants.network_name.clone(),
            network_id: constants.network_id.clone(),
            rpc_url: constants.default_rpc.clone(),
            gas_price: constants.default_gas_price,
            gas_adjustment: constants.default_gas_adjustment,
            native_denom: constants.native_denom.clone(),
            tx_delay_ms: 2000,
            contracts,
        }
    }

    /// Load contract addresses for the given network from the contracts configuration file.
    /// If the file or the specific addresses cannot be read, this will return default (empty) addresses
    /// so the caller can still proceed without panicking.
    fn load_contract_addresses(network: &str) -> ContractAddresses {
        // Determine configuration directory – fall back to local `config` directory inside the project
        let config_dir = env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        // Build configuration loader for `contracts.toml`
        let settings = ConfigLoader::builder()
            .add_source(File::with_name(&format!("{}/contracts", config_dir)))
            .build();

        match settings {
            Ok(settings) => {
                let pool_manager_key = format!("{}.pool_manager.address", network);
                let farm_manager_key = format!("{}.farm_manager.address", network);
                let fee_collector_key = format!("{}.fee_collector.address", network);
                let epoch_manager_key = format!("{}.epoch_manager.address", network);

                ContractAddresses {
                    pool_manager: settings
                        .get::<String>(&pool_manager_key)
                        .unwrap_or_default(),
                    farm_manager: settings.get::<String>(&farm_manager_key).ok(),
                    fee_collector: settings.get::<String>(&fee_collector_key).ok(),
                    epoch_manager: settings.get::<String>(&epoch_manager_key).ok(),
                }
            }
            Err(_) => ContractAddresses::default(),
        }
    }
}

impl Default for MantraNetworkConfig {
    fn default() -> Self {
        match NetworkConstants::default_dukong() {
            Ok(constants) => Self::from_constants(&constants),
            Err(_) => Self {
                network_name: "mantra-dukong".to_string(),
                network_id: "mantra-dukong-1".to_string(),
                rpc_url: "https://rpc.dukong.mantrachain.io/".to_string(),
                gas_price: 0.025,
                gas_adjustment: 1.3,
                native_denom: "uom".to_string(),
                tx_delay_ms: 2000,
                contracts: ContractAddresses::default(),
            },
        }
    }
}

/// Complete configuration with wallet info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Active network configuration
    pub network: MantraNetworkConfig,
    /// Wallet mnemonic (seed phrase)
    pub mnemonic: Option<String>,
    /// Known tokens and their metadata
    pub tokens: HashMap<String, TokenInfo>,
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

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self {
            network: MantraNetworkConfig::default(),
            mnemonic: None,
            tokens: HashMap::new(),
        }
    }

    /// Load configuration from a file
    pub fn load(path: &PathBuf) -> Result<Self, Error> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse config: {}", e)))?;
        Ok(config)
    }

    /// Save configuration to a file
    pub fn save(&self, path: &PathBuf) -> Result<(), Error> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {}", e)))?;

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
        path.push("mantra-dex");
        path.push("config.toml");
        path
    }
}
