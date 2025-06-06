mod utils;

use mantra_dex_sdk::{MantraDexClient, MantraWallet};
use utils::test_utils::{
    create_test_client, create_test_network_config, load_test_config, should_execute_writes,
};

#[tokio::test]
async fn test_client_creation() {
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
    let network_config = create_test_network_config();
    let client = MantraDexClient::new(network_config)
        .await
        .expect("Failed to create client");

    // Try to get wallet when none is set
    let wallet_result = client.wallet();
    assert!(wallet_result.is_err(), "Wallet should not be available");
}

#[tokio::test]
async fn test_client_get_last_block_height() {
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

#[tokio::test]
async fn test_comprehensive_client_workflow() {
    // Test a comprehensive client workflow including network configuration, wallet management, and queries
    let network_config = create_test_network_config();
    let test_config = load_test_config();

    // Create client
    let client = MantraDexClient::new(network_config.clone())
        .await
        .expect("Failed to create client");

    // Add wallet
    let mnemonic = test_config
        .wallets
        .get("primary")
        .expect("Primary wallet not found in test config");
    let wallet =
        MantraWallet::from_mnemonic(mnemonic, 0).expect("Failed to create wallet from mnemonic");
    let client = client.with_wallet(wallet);

    // Test basic queries
    let block_height = client
        .get_last_block_height()
        .await
        .expect("Failed to get block height");
    println!("Current block height: {}", block_height);

    let balances = client.get_balances().await.expect("Failed to get balances");
    println!("Wallet balances: {} tokens", balances.len());

    let pools = client
        .get_pools(Some(5))
        .await
        .expect("Failed to get pools");
    println!("Available pools: {}", pools.len());

    // Verify client state
    assert!(client.wallet().is_ok(), "Client should have wallet");
    assert!(block_height > 0, "Block height should be positive");
}
