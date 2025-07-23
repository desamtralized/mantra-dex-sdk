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
    /// Chain ID (for transaction signing)
    pub chain_id: String,
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

        // Try multiple paths for the config file
        let config_paths = vec![
            format!("{}/network", config_dir),
            "config/network".to_string(),
            "../config/network".to_string(),
            "../../config/network".to_string(),
        ];

        for config_path in &config_paths {
            if let Ok(settings) = ConfigLoader::builder()
                .add_source(File::with_name(config_path))
                .build()
            {
                if let Ok(constants) = settings.get::<NetworkConstants>(network) {
                    return Ok(constants);
                }
            }
        }

        // If we can't load from config files, return hardcoded constants
        match network {
            "mantra-dukong" => Ok(NetworkConstants {
                network_name: "mantra-dukong".to_string(),
                chain_id: "mantra-dukong-1".to_string(),
                default_rpc: "https://rpc.dukong.mantrachain.io:443".to_string(),
                default_gas_price: 0.01,
                default_gas_adjustment: 1.5,
                native_denom: "uom".to_string(),
            }),
            _ => Err(ConfigError::NotFound(format!(
                "Network configuration for '{}' not found",
                network
            ))),
        }
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
    /// Skip Adapter contracts
    pub skip_entry_point: Option<String>,
    pub skip_ibc_hooks_adapter: Option<String>,
    pub skip_mantra_dex_adapter: Option<String>,
}

impl Default for ContractAddresses {
    fn default() -> Self {
        Self {
            pool_manager: "".to_string(),
            farm_manager: None,
            fee_collector: None,
            epoch_manager: None,
            skip_entry_point: None,
            skip_ibc_hooks_adapter: None,
            skip_mantra_dex_adapter: None,
        }
    }
}

/// Network configuration for Mantra DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraNetworkConfig {
    /// Network name (e.g., mantra-dukong)
    pub network_name: String,
    /// Chain ID (e.g., mantra-dukong)
    pub chain_id: String,
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Gas price in native token
    pub gas_price: f64,
    /// Gas adjustment for transactions
    pub gas_adjustment: f64,
    /// Native token denom
    pub native_denom: String,
    /// Contract addresses
    pub contracts: ContractAddresses,
}

impl MantraNetworkConfig {
    /// Create a new network config from network constants
    pub fn from_constants(constants: &NetworkConstants) -> Result<Self, Error> {
        // Attempt to load contract addresses for this network from `config/contracts.toml`
        let contracts = Self::load_contract_addresses(&constants.network_name)?;

        Ok(Self {
            network_name: constants.network_name.clone(),
            chain_id: constants.chain_id.clone(),
            rpc_url: constants.default_rpc.clone(),
            gas_price: constants.default_gas_price,
            gas_adjustment: constants.default_gas_adjustment,
            native_denom: constants.native_denom.clone(),
            contracts,
        })
    }

    /// Load contract addresses for the given network from the contracts configuration file.
    /// Returns an error if the contract addresses cannot be loaded.
    fn load_contract_addresses(network: &str) -> Result<ContractAddresses, Error> {
        // Determine configuration directory – fall back to local `config` directory inside the project
        let config_dir = env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        // Try multiple paths for the config file
        let config_paths = vec![
            format!("{}/contracts", config_dir),
            "config/contracts".to_string(),
            "../config/contracts".to_string(),
            "../../config/contracts".to_string(),
        ];

        for config_path in &config_paths {
            if let Ok(settings) = ConfigLoader::builder()
                .add_source(File::with_name(config_path))
                .build()
            {
                let pool_manager_key = format!("{}.pool_manager.address", network);
                let farm_manager_key = format!("{}.farm_manager.address", network);
                let fee_collector_key = format!("{}.fee_collector.address", network);
                let epoch_manager_key = format!("{}.epoch_manager.address", network);
                let skip_entry_point_key = format!("{}.skip_entry_point.address", network);
                let skip_ibc_hooks_adapter_key =
                    format!("{}.skip_ibc_hooks_adapter.address", network);
                let skip_mantra_dex_adapter_key =
                    format!("{}.skip_mantra_dex_adapter.address", network);

                if let Ok(pool_manager) = settings.get::<String>(&pool_manager_key) {
                    return Ok(ContractAddresses {
                        pool_manager,
                        farm_manager: settings.get::<String>(&farm_manager_key).ok(),
                        fee_collector: settings.get::<String>(&fee_collector_key).ok(),
                        epoch_manager: settings.get::<String>(&epoch_manager_key).ok(),
                        skip_entry_point: settings.get::<String>(&skip_entry_point_key).ok(),
                        skip_ibc_hooks_adapter: settings
                            .get::<String>(&skip_ibc_hooks_adapter_key)
                            .ok(),
                        skip_mantra_dex_adapter: settings
                            .get::<String>(&skip_mantra_dex_adapter_key)
                            .ok(),
                    });
                }
            }
        }

        Err(Error::Config(format!(
            "Contract addresses for network '{}' not found in configuration",
            network
        )))
    }
}

impl Default for MantraNetworkConfig {
    fn default() -> Self {
        match NetworkConstants::default_dukong() {
            Ok(constants) => Self::from_constants(&constants).unwrap_or_else(|_| Self {
                network_name: constants.network_name,
                chain_id: constants.chain_id,
                rpc_url: constants.default_rpc,
                gas_price: constants.default_gas_price,
                gas_adjustment: constants.default_gas_adjustment,
                native_denom: constants.native_denom,
                contracts: ContractAddresses::default(),
            }),
            Err(_) => Self {
                network_name: "mantra-dukong".to_string(),
                chain_id: "mantra-dukong-1".to_string(),
                rpc_url: "https://rpc.dukong.mantrachain.io:443".to_string(),
                gas_price: 0.01,
                gas_adjustment: 1.5,
                native_denom: "uom".to_string(),
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
