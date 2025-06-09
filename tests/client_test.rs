mod utils;

use mantra_dex_sdk::{MantraDexClient, MantraWallet};
use utils::test_utils::{
    create_test_client, create_test_network_config, get_or_create_om_usdc_pool_id, init_test_env,
    load_test_config,
};

#[tokio::test]
async fn test_client_creation() {
    init_test_env();

    let network_config = create_test_network_config();
    let client_result = MantraDexClient::new(network_config.clone()).await;

    assert!(
        client_result.is_ok(),
        "Failed to create client: {:?}",
        client_result.err()
    );

    let client = client_result.unwrap();

    // Verify network config
    let client_config = client.config();
    assert_eq!(
        client_config.network_name, network_config.network_name,
        "Client network name should match config"
    );
    assert_eq!(
        client_config.network_id, network_config.network_id,
        "Client network ID should match config"
    );
    assert_eq!(
        client_config.rpc_url, network_config.rpc_url,
        "Client RPC URL should match config"
    );
}

#[tokio::test]
async fn test_client_with_wallet() {
    init_test_env();

    let network_config = create_test_network_config();
    let client = MantraDexClient::new(network_config.clone())
        .await
        .expect("Failed to create client");

    let test_config = load_test_config();
    let mnemonic = test_config
        .wallets
        .get("primary")
        .expect("Primary wallet not found in test config");

    let wallet =
        MantraWallet::from_mnemonic(mnemonic, 0).expect("Failed to create wallet from mnemonic");

    let client_with_wallet = client.with_wallet(wallet);

    // Verify wallet is set
    let wallet_result = client_with_wallet.wallet();
    assert!(
        wallet_result.is_ok(),
        "Wallet should be set: {:?}",
        wallet_result.err()
    );
}

#[tokio::test]
async fn test_client_without_wallet() {
    init_test_env();

    let network_config = create_test_network_config();
    let client = MantraDexClient::new(network_config)
        .await
        .expect("Failed to create client");

    // Try to get wallet when none is set
    let wallet_result = client.wallet();
    assert!(wallet_result.is_err(), "Wallet should not be available");
}

#[tokio::test]
async fn test_client_query_pool() {
    init_test_env();

    let client = create_test_client().await;

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        // Query pool info
        let pool_result = client.get_pool(&pool_id).await;

        // This should succeed if the pool exists and the RPC works
        if let Ok(pool_info) = pool_result {
            assert_eq!(
                pool_info.pool_info.pool_identifier,
                pool_id.clone(),
                "Pool ID in response should match requested ID"
            );
            println!("Successfully queried pool: {}", pool_id);
        } else {
            println!("Warning: Pool query failed: {:?}", pool_result.err());
        }
    } else {
        println!("Warning: Could not get or create OM/USDC pool");
    }
}

#[tokio::test]
async fn test_client_query_pools() {
    init_test_env();

    let client = create_test_client().await;

    // Query all pools
    let pools_result = client.get_pools(Some(10)).await;

    match pools_result {
        Ok(pools) => {
            println!("Successfully queried {} pools", pools.len());
            for pool in pools {
                println!("Pool ID: {}", pool.pool_info.pool_identifier);
            }
        }
        Err(e) => {
            println!("Warning: Pools query failed: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_client_simulate_swap() {
    init_test_env();

    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        // Simulate a swap
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

        let swap_result = client
            .simulate_swap(
                &pool_id,
                cosmwasm_std::Coin {
                    denom: uom_denom,
                    amount: cosmwasm_std::Uint128::from(1000000u128),
                },
                &uusdc_denom,
            )
            .await;

        match swap_result {
            Ok(simulation) => {
                println!(
                    "Simulation successful: return amount = {}",
                    simulation.return_amount
                );
            }
            Err(e) => {
                println!("Warning: Simulation failed: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDC pool for simulation");
    }
}

#[tokio::test]
async fn test_client_get_last_block_height() {
    init_test_env();

    let client = create_test_client().await;
    let last_block_height = client.get_last_block_height().await;
    assert!(
        last_block_height.is_ok(),
        "Failed to get last block height: {:?}",
        last_block_height.err()
    );
    let height = last_block_height.unwrap();
    println!("Last block height: {:?}", height);
    assert!(
        height > 4091786,
        "Last block height should be greater than 0"
    );
}

#[tokio::test]
async fn test_client_get_balances() {
    init_test_env();

    let client = create_test_client().await;
    let balances = client.get_balances().await;
    assert!(
        balances.is_ok(),
        "Failed to get balances: {:?}",
        balances.err()
    );
    let balances = balances.unwrap();
    println!("Balances: {:?}", balances);
}
