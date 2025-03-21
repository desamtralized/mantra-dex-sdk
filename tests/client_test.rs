mod utils;

use cosmwasm_std::{Coin, Uint128};
use mantra_dex_sdk::{MantraDexClient, MantraWallet};
use utils::{create_test_client, create_test_network_config, get_om_usdc_pool_id, init_test_env, load_test_config};

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
    let test_config = load_test_config();

    // Get pool ID from test config
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Query pool info
    let pool_result = client.get_pool(&pool_id).await;

    // This should succeed if the pool exists and the RPC works
    if let Ok(pool_info) = pool_result {
        assert_eq!(
            pool_info.pool_info.pool_identifier,
            pool_id.clone(),
            "Pool ID in response should match requested ID"
        );
    } else {
        // If the test fails, we'll print the error but not fail the test
        // This is because the pool might not exist on the test network
        eprintln!("Warning: Pool query failed: {:?}", pool_result.err());
        eprintln!(
            "This is not a test failure if the pool doesn't exist or the network is unavailable."
        );
    }
}

#[tokio::test]
async fn test_client_query_pools() {
    init_test_env();

    let client = create_test_client().await;

    // Query pools with limit
    let pools_result = client.get_pools(Some(10)).await;

    // This should succeed if the RPC works
    if let Ok(pools) = pools_result {
        // Just check that we got a valid response (may be empty if no pools)
        assert!(pools.len() > 0, "Should return at least one pool");
        assert!(pools.len() <= 10, "Should return at most 10 pools");
    } else {
        // If the test fails, we'll print the error but not fail the test
        eprintln!("Warning: Pools query failed: {:?}", pools_result.err());
        eprintln!("This is not a test failure if the network is unavailable.");
        panic!("Pools query failed");
    }
}

#[tokio::test]
async fn test_client_simulate_swap() {
    init_test_env();

    let test_config = load_test_config();
    let client = create_test_client().await;

    // Get pool ID from test config
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();
    let uom_denom = test_config.tokens.get("uom").unwrap().denom.clone().unwrap();
    let uusdc_denom = test_config.tokens.get("uusdc").unwrap().denom.clone().unwrap();

    // Create offer asset
    let offer_asset = Coin {
        denom: uom_denom.clone(),
        amount: Uint128::new(1_000_000), // 1 OM
    };

    // Simulate swap
    let simulation_result = client
        .simulate_swap(
            &pool_id,
            offer_asset,
            &uusdc_denom,
        )
        .await;

    // This should succeed if the pool exists and the RPC works
    if let Ok(simulation) = simulation_result {
        // Check simulation response
        assert!(
            !simulation.return_amount.is_zero(),
            "Return amount should not be zero"
        );
        assert!(
            !simulation.spread_amount.is_zero() || simulation.swap_fee_amount.is_zero(),
            "Spread amount and commission should not both be zero"
        );
        println!("Simulation: {:?}", simulation);
    } else {
        // If the test fails, we'll print the error but not fail the test
        eprintln!(
            "Warning: Swap simulation failed: {:?}",
            simulation_result.err()
        );
        eprintln!(
            "This is not a test failure if the pool doesn't exist or the network is unavailable."
        );
        panic!("Swap simulation failed");
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
