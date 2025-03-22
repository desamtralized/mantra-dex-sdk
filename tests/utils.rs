use config::{Config as ConfigLoader, File};
use mantra_dex_sdk::{
    config::{ContractAddresses, MantraNetworkConfig, NetworkConstants},
    MantraDexClient, MantraWallet,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Once;

#[cfg(test)]
pub mod test_utils {
    use super::*;

    static INIT: Once = Once::new();

    /// Test configuration loaded from config/test.toml
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TestConfig {
        /// Test settings
        pub test: TestSettings,
        /// Test wallets
        pub wallets: HashMap<String, String>,
        /// Test tokens
        pub tokens: HashMap<String, TestToken>,
    }

    /// Test settings
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TestSettings {
        /// Network to use for testing
        pub network: String,
    }

    /// Test token information
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct TestToken {
        /// Token name
        pub name: String,
        /// Token symbol
        pub symbol: String,
        /// Token decimals
        pub decimals: u8,
        /// Token denom
        pub denom: Option<String>,
    }

    /// Load contract addresses from config/contracts.toml
    pub fn load_contract_addresses(network: &str) -> ContractAddresses {
        let config_dir =
            std::env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let settings = ConfigLoader::builder()
            .add_source(File::with_name(&format!("{}/contracts", config_dir)))
            .build()
            .expect("Failed to load contracts config");

        // First, try to get the nested structure
        let pool_manager = settings.get::<String>(&format!("{}.pool_manager.address", network));
        let farm_manager = settings.get::<String>(&format!("{}.farm_manager.address", network));
        let fee_collector = settings.get::<String>(&format!("{}.fee_collector.address", network));
        let epoch_manager = settings.get::<String>(&format!("{}.epoch_manager.address", network));

        ContractAddresses {
            pool_manager: pool_manager.unwrap_or_default(),
            farm_manager: farm_manager.ok(),
            fee_collector: fee_collector.ok(),
            epoch_manager: epoch_manager.ok(),
        }
    }

    /// Load test configuration from config/test.toml
    pub fn load_test_config() -> TestConfig {
        let config_dir =
            std::env::var("MANTRA_CONFIG_DIR").unwrap_or_else(|_| "config".to_string());

        let settings = ConfigLoader::builder()
            .add_source(File::with_name(&format!("{}/test", config_dir)))
            .build()
            .expect("Failed to load test config");

        settings
            .try_deserialize::<TestConfig>()
            .expect("Failed to deserialize test config")
    }

    /// Initialize test environment
    pub fn init_test_env() {
        INIT.call_once(|| {
            // Initialize environment for tests
            dotenv::from_path(".env.test").ok();
        });
    }

    /// Create a network config for testing
    pub fn create_test_network_config() -> MantraNetworkConfig {
        let test_config = load_test_config();
        let network = &test_config.test.network;

        // Load network constants
        let network_constants =
            NetworkConstants::load(network).expect("Failed to load network constants");

        // Create network config from constants
        let mut network_config = MantraNetworkConfig::from_constants(&network_constants);

        // Load contract addresses
        network_config.contracts = load_contract_addresses(network);

        network_config
    }

    /// Create a client with the primary test wallet
    #[cfg(test)]
    pub async fn create_test_client() -> MantraDexClient {
        let network_config = create_test_network_config();
        let test_config = load_test_config();

        // Log contract addresses for debugging
        println!(
            "Pool Manager Contract: {}",
            network_config.contracts.pool_manager
        );
        println!(
            "Farm Manager Contract: {:?}",
            network_config.contracts.farm_manager
        );
        println!(
            "Fee Collector Contract: {:?}",
            network_config.contracts.fee_collector
        );
        println!(
            "Epoch Manager Contract: {:?}",
            network_config.contracts.epoch_manager
        );

        // Get the primary wallet mnemonic
        let primary_mnemonic = test_config
            .wallets
            .get("primary")
            .expect("Primary wallet not found in test config");

        // Create wallet
        let wallet = MantraWallet::from_mnemonic(primary_mnemonic, 0)
            .expect("Failed to create wallet from mnemonic");

        println!("Wallet address: {}", wallet.address().unwrap());

        // Create client with wallet
        MantraDexClient::new(network_config)
            .await
            .expect("Failed to create client")
            .with_wallet(wallet)
    }

    /// Create a wallet from the test config
    pub fn create_test_wallet(wallet_name: &str) -> MantraWallet {
        let test_config = load_test_config();

        // Get the wallet mnemonic
        let mnemonic = test_config.wallets.get(wallet_name).expect(&format!(
            "Wallet '{}' not found in test config",
            wallet_name
        ));

        // Create wallet
        MantraWallet::from_mnemonic(mnemonic, 0).expect("Failed to create wallet from mnemonic")
    }

    pub async fn get_om_usdc_pool_id(client: &MantraDexClient) -> Option<String> {
        let test_config = load_test_config();
        let uom_denom = test_config
            .tokens
            .get("uom")
            .unwrap()
            .denom
            .clone()
            .unwrap();
        let uusdc_denom = test_config
            .tokens
            .get("uusdc")
            .unwrap()
            .denom
            .clone()
            .unwrap();
        println!("uom_denom: {}", uom_denom);
        println!("uusdc_denom: {}", uusdc_denom);
        let pools = client.get_pools(Some(100)).await.unwrap();
        let mut pool_id: Option<String> = None;
        for pool in pools {
            if pool.pool_info.assets.iter().any(|a| a.denom == uom_denom)
                && pool.pool_info.assets.iter().any(|a| a.denom == uusdc_denom)
            {
                pool_id = Some(pool.pool_info.pool_identifier.clone())
            }
        }
        println!("Pool ID: {:?}", pool_id);
        pool_id
    }
}
