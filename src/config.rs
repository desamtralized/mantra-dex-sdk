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
    /// Create a new NetworkConstants with specified values
    pub fn new(
        network_name: String,
        network_id: String,
        default_rpc: String,
        default_gas_price: f64,
        default_gas_adjustment: f64,
        native_denom: String,
    ) -> Self {
        Self {
            network_name,
            network_id,
            default_rpc,
            default_gas_price,
            default_gas_adjustment,
            native_denom,
        }
    }

    /// Load network constants from the configuration file (legacy method)
    pub fn load(network: &str) -> Result<Self, ConfigError> {
        let config_dir = env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let settings = ConfigLoader::builder()
            // Add the config file
            .add_source(File::with_name(&format!("{}/network", config_dir)))
            .build()?;

        // Extract the network section
        settings.get::<NetworkConstants>(network)
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
    /// Contract addresses
    pub contracts: ContractAddresses,
}

impl MantraNetworkConfig {
    /// Create a new network config with specified values
    pub fn new(
        network_name: String,
        network_id: String,
        rpc_url: String,
        gas_price: f64,
        gas_adjustment: f64,
        native_denom: String,
        contracts: ContractAddresses,
    ) -> Self {
        Self {
            network_name,
            network_id,
            rpc_url,
            gas_price,
            gas_adjustment,
            native_denom,
            contracts,
        }
    }

    /// Create a new network config from network constants
    pub fn from_constants(constants: &NetworkConstants) -> Self {
        Self {
            network_name: constants.network_name.clone(),
            network_id: constants.network_id.clone(),
            rpc_url: constants.default_rpc.clone(),
            gas_price: constants.default_gas_price,
            gas_adjustment: constants.default_gas_adjustment,
            native_denom: constants.native_denom.clone(),
            contracts: ContractAddresses::default(),
        }
    }

    /// Update contract addresses
    pub fn with_contracts(mut self, contract_addresses: ContractAddresses) -> Self {
        self.contracts = contract_addresses;
        self
    }

    /// Set the pool manager contract address
    pub fn with_pool_manager(mut self, pool_manager: String) -> Self {
        self.contracts.pool_manager = pool_manager;
        self
    }

    /// Set the farm manager contract address
    pub fn with_farm_manager(mut self, farm_manager: Option<String>) -> Self {
        self.contracts.farm_manager = farm_manager;
        self
    }

    /// Set the fee collector contract address
    pub fn with_fee_collector(mut self, fee_collector: Option<String>) -> Self {
        self.contracts.fee_collector = fee_collector;
        self
    }

    /// Set the epoch manager contract address
    pub fn with_epoch_manager(mut self, epoch_manager: Option<String>) -> Self {
        self.contracts.epoch_manager = epoch_manager;
        self
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

impl Config {
    /// Create a new configuration with the specified network
    pub fn with_network(network: MantraNetworkConfig) -> Self {
        Self {
            network,
            mnemonic: None,
            tokens: HashMap::new(),
        }
    }

    /// Create a new configuration with a wallet
    pub fn with_wallet(network: MantraNetworkConfig, mnemonic: String) -> Self {
        Self {
            network,
            mnemonic: Some(mnemonic),
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

    /// Add token information
    pub fn add_token(&mut self, denom: String, token_info: TokenInfo) {
        self.tokens.insert(denom, token_info);
    }
}
