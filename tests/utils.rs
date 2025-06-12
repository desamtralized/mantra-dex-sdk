use config::{Config as ConfigLoader, File};
use cosmwasm_std::{Coin, Decimal, Uint128};
use lazy_static::lazy_static;
use mantra_dex_sdk::{
    config::{ContractAddresses, MantraNetworkConfig, NetworkConstants},
    MantraDexClient, MantraWallet,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex as TokioMutex;

lazy_static! {
    /// Global mutex to ensure that tests that perform writes run sequentially.
    pub static ref GLOBAL_TEST_MUTEX: TokioMutex<()> = TokioMutex::new(());
}

#[cfg(test)]
pub mod test_utils {
    use super::*;

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
    #[allow(dead_code)]
    pub async fn create_test_client() -> MantraDexClient {
        let network_config = create_test_network_config();
        let test_config = load_test_config();

        // Get the primary wallet mnemonic
        let primary_mnemonic = test_config
            .wallets
            .get("primary")
            .expect("Primary wallet not found in test config");

        // Create wallet
        let wallet = MantraWallet::from_mnemonic(primary_mnemonic, 0)
            .expect("Failed to create wallet from mnemonic");

        // Create client with wallet
        MantraDexClient::new(network_config)
            .await
            .expect("Failed to create client")
            .with_wallet(wallet)
    }

    /// Create a wallet from the test config
    #[allow(dead_code)]
    pub fn create_test_wallet(wallet_name: &str) -> MantraWallet {
        let test_config = load_test_config();

        // Get the wallet mnemonic
        let mnemonic = test_config
            .wallets
            .get(wallet_name)
            .unwrap_or_else(|| panic!("Wallet '{}' not found in test config", wallet_name));

        // Create wallet
        MantraWallet::from_mnemonic(mnemonic, 0).expect("Failed to create wallet from mnemonic")
    }

    /// Get or create the OM/USDC pool for testing
    #[allow(dead_code)]
    pub async fn get_or_create_om_usdc_pool_id(client: &MantraDexClient) -> Option<String> {
        // Try to create or find the test pool
        match create_test_pool_if_needed(client).await {
            Ok(pool_id) => Some(pool_id),
            Err(_e) => None,
        }
    }

    /// Create a test pool with OM and USDC if one doesn't exist
    #[allow(dead_code)]
    pub async fn create_test_pool_if_needed(
        client: &MantraDexClient,
    ) -> Result<String, Box<dyn std::error::Error>> {
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

        // First, try to find an existing pool
        if let Ok(pools) = client.get_pools(Some(100)).await {
            for pool in pools {
                if pool.pool_info.assets.iter().any(|a| a.denom == uom_denom)
                    && pool.pool_info.assets.iter().any(|a| a.denom == uusdc_denom)
                {
                    return Ok(pool.pool_info.pool_identifier);
                }
            }
        }

        // No pool found, create one by providing initial liquidity
        let pool_id = "uom.usdc.pool".to_string();
        let actual_pool_id = format!("o.{}", pool_id);

        // Attempt to create the pool. If it already exists, that's fine.
        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee {
                share: cosmwasm_std::Decimal::percent(1),
            },
            swap_fee: mantra_dex_std::fee::Fee {
                share: cosmwasm_std::Decimal::percent(2),
            },
            burn_fee: mantra_dex_std::fee::Fee {
                share: cosmwasm_std::Decimal::zero(),
            },
            extra_fees: vec![],
        };
        let pool_type = mantra_dex_std::pool_manager::PoolType::ConstantProduct {};

        match client
            .create_pool(
                vec![uom_denom.clone(), uusdc_denom.clone()],
                vec![6, 6],
                pool_fees,
                pool_type,
                Some(pool_id.clone()),
            )
            .await
        {
            Ok(res) => println!("Pool creation tx successful: {}", res.txhash),
            Err(e) => {
                let err_msg = e.to_string();
                if !err_msg.contains("pool already exists") {
                    return Err(Box::new(e));
                }
                println!("Pool already exists, which is fine.");
            }
        }

        // Now, attempt to provide liquidity with retries.
        let initial_assets = vec![
            Coin {
                denom: uom_denom.clone(),
                amount: Uint128::new(100_000_000), // 100 OM
            },
            Coin {
                denom: uusdc_denom.clone(),
                amount: Uint128::new(100_000_000), // 100 USDC
            },
        ];

        for i in 0..5 {
            match client
                .provide_liquidity_unchecked(
                    &actual_pool_id,
                    initial_assets.clone(),
                    Some(Decimal::percent(5)),
                    Some(Decimal::percent(5)),
                )
                .await
            {
                Ok(res) => {
                    println!(
                        "Liquidity provision tx successful after {} retries: {}",
                        i, res.txhash
                    );
                    return Ok(actual_pool_id);
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    if err_msg.contains("pool does not have enough liquidity")
                        || err_msg.contains("share is not found")
                    {
                        // This might happen if the pool was just created and the node hasn't processed it fully.
                        println!(
                            "Liquidity provision failed (retry {}/5): {}. Retrying in 2s...",
                            i + 1,
                            e
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    } else {
                        // For other errors (like sequence mismatch), fail immediately.
                        return Err(Box::new(e));
                    }
                }
            }
        }

        Err("Failed to provide liquidity after 5 retries.".into())
    }

    /// Check if we should execute write operations (create pools, swaps, etc.)
    #[allow(dead_code)]
    pub fn should_execute_writes() -> bool {
        std::env::var("EXECUTE_WRITES")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true"
    }
}
