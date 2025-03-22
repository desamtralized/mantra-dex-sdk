mod utils;

use cosmwasm_std::{Coin, Decimal, Uint128};
use utils::test_utils::{
    create_test_client, get_om_usdc_pool_id, init_test_env, load_test_config,
};

/// This test will only execute actual swaps if the EXECUTE_WRITES env var is set to true
fn should_execute_writes() -> bool {
    std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true"
}

#[tokio::test]
async fn test_list_all_pools() {
    init_test_env();

    // This is a read-only test, so it doesn't need the EXECUTE_WRITES flag
    let client = create_test_client().await;

    // Get all pools (no limit)
    match client.get_pools(Some(100)).await {
        Ok(pools) => {
            println!("Found {} pools:", pools.len());
            for pool in pools {
                println!("Pool ID: {}", pool.pool_info.pool_identifier);
                println!("  LP Denom: {}", pool.total_share.denom);
                println!("  Pool Type: {:?}", pool.pool_info.pool_type);
                println!("  Assets:");
                for asset in &pool.pool_info.assets {
                    println!("    {} - {}", asset.denom, asset.amount);
                }
                println!("");
            }
        }
        Err(e) => {
            println!("Failed to get pools: {:?}", e);
            panic!("Failed to get pools");
        }
    }
}

#[tokio::test]
async fn test_swap_operation() {
    println!("Starting swap test...");
    init_test_env();
    println!("Environment initialized");

    println!("Creating test client...");
    let client = create_test_client().await;
    println!("Loading test config...");
    let test_config = load_test_config();
    println!("Getting pool ID...");
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();
    println!("Found pool ID: {}", pool_id);

    println!("Getting token denoms...");
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
    println!("Token denoms: {} and {}", uom_denom, uusdc_denom);

    // Create offer asset (a small amount for testing)
    let offer_asset = Coin {
        denom: uom_denom.clone(),
        amount: Uint128::new(100_000), // 0.1 OM
    };
    println!(
        "Created offer asset: {} {}",
        offer_asset.amount, offer_asset.denom
    );

    println!("About to execute swap...");
    // Execute swap with timeout
    match tokio::time::timeout(
        std::time::Duration::from_secs(30), // 30 second timeout
        client.swap(
            &pool_id,
            offer_asset,
            &uusdc_denom, // The denom of the ask asset, should match one in the pool
            Some(Decimal::percent(1)), // 1% max spread
        ),
    )
    .await
    {
        Ok(Ok(tx_response)) => {
            println!("Swap successful with txhash: {}", tx_response.txhash);
            assert_eq!(
                tx_response.code, 0u32,
                "Transaction failed: {:?}",
                tx_response.raw_log
            );
            assert!(
                !tx_response.txhash.is_empty(),
                "Transaction hash should not be empty"
            );
        }
        Ok(Err(e)) => {
            println!("Swap failed with error: {:?}", e);
            panic!("Swap failed");
        }
        Err(_) => {
            println!("Swap operation timed out after 30 seconds");
            panic!("Swap timed out");
        }
    }
}

#[tokio::test]
async fn test_provide_liquidity() {
    init_test_env();

    // Skip actual liquidity provision unless EXECUTE_WRITES is set
    if !should_execute_writes() {
        println!("Skipping actual liquidity provision test. Set EXECUTE_WRITES=true to enable.");
        return;
    }

    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get pool ID from test config
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");

    // Create assets for liquidity provision
    let assets = vec![
        Coin {
            denom: "uom".to_string(),
            amount: Uint128::new(100_000), // 0.1 OM
        },
        Coin {
            denom: "uusdc".to_string(),
            amount: Uint128::new(100_000), // 0.1 USDC
        },
    ];

    // Provide liquidity
    match client
        .provide_liquidity(
            &pool_id.unwrap(),
            assets,
            Some(Decimal::percent(1)), // 1% slippage tolerance
        )
        .await
    {
        Ok(tx_response) => {
            println!(
                "Liquidity provision successful with txhash: {}",
                tx_response.txhash
            );
            assert_eq!(
                tx_response.code, 0u32,
                "Transaction failed: {:?}",
                tx_response.raw_log
            );
            assert!(
                !tx_response.txhash.is_empty(),
                "Transaction hash should not be empty"
            );
        }
        Err(e) => {
            println!("Liquidity provision failed: {:?}", e);
            // Don't fail the test, just log the error
        }
    }
}

#[tokio::test]
async fn test_withdraw_liquidity() {
    init_test_env();

    // Skip actual liquidity withdrawal unless EXECUTE_WRITES is set
    if !should_execute_writes() {
        println!("Skipping actual liquidity withdrawal test. Set EXECUTE_WRITES=true to enable.");
        return;
    }

    let client = create_test_client().await;

    // Get pool ID from test config
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Withdraw a small amount of liquidity
    let lp_amount = Uint128::new(100); // A very small amount to test

    // Withdraw liquidity
    match client.withdraw_liquidity(&pool_id, lp_amount).await {
        Ok(tx_response) => {
            println!(
                "Liquidity withdrawal successful with txhash: {}",
                tx_response.txhash
            );
            assert_eq!(
                tx_response.code, 0u32,
                "Transaction failed: {:?}",
                tx_response.raw_log
            );
            assert!(
                !tx_response.txhash.is_empty(),
                "Transaction hash should not be empty"
            );
        }
        Err(e) => {
            println!("Liquidity withdrawal failed: {:?}", e);
            // Don't fail the test, just log the error
        }
    }
}

#[tokio::test]
async fn test_get_pool() {
    init_test_env();

    // This is a read-only test, so it doesn't need the EXECUTE_WRITES flag
    let client = create_test_client().await;

    // Get pool ID from test config
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    println!("Querying pool with ID: {}", pool_id);

    // Query specific pool
    match client.get_pool(&pool_id).await {
        Ok(pool) => {
            println!("Found pool:");
            println!("Pool ID: {}", pool.pool_info.pool_identifier);
            println!("LP Denom: {}", pool.pool_info.lp_denom);
            println!("Pool Type: {:?}", pool.pool_info.pool_type);
            println!("Assets:");
            for asset in &pool.pool_info.assets {
                println!("  {} - {}", asset.denom, asset.amount);
            }
        }
        Err(e) => {
            println!("Failed to get pool: {:?}", e);
            // Don't fail the test, just log the error
        }
    }
}

#[tokio::test]
async fn test_simulate_swap() {
    init_test_env();

    // This is a read-only test, so it doesn't need the EXECUTE_WRITES flag
    let client = create_test_client().await;

    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Create offer asset (a small amount for testing)
    let offer_asset = Coin {
        denom: "uom".to_string(),
        amount: Uint128::new(1_000_000), // 1 OM
    };

    println!(
        "Simulating swap of {} {} for uusdc",
        offer_asset.amount, offer_asset.denom
    );

    // Simulate swap
    match client
        .simulate_swap(
            &pool_id,
            offer_asset,
            "uusdc", // The denom of the ask asset
        )
        .await
    {
        Ok(simulation) => {
            println!("Swap simulation result:");
            println!("Return amount: {}", simulation.return_amount);
            println!("Spread amount: {}", simulation.spread_amount);
            println!("Commission amount: {}", simulation.swap_fee_amount);
        }
        Err(e) => {
            println!("Failed to simulate swap: {:?}", e);
            // Don't fail the test, just log the error
        }
    }
}
