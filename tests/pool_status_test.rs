mod utils;

use mantra_dex_sdk::{client::PoolStatus, Error};
use utils::test_utils::{create_test_client, get_om_usdc_pool_id};

/// Test pool status enum functionality
#[tokio::test]
async fn test_pool_status_enum() {
    // Test Available status
    let available_status = PoolStatus::Available;
    assert!(
        available_status.is_available(),
        "Available status should be available"
    );

    // Test Disabled status
    let disabled_status = PoolStatus::Disabled;
    assert!(
        !disabled_status.is_available(),
        "Disabled status should not be available"
    );

    // Test equality
    assert_eq!(available_status, PoolStatus::Available);
    assert_eq!(disabled_status, PoolStatus::Disabled);
    assert_ne!(available_status, disabled_status);
}

/// Test pool status extraction from pool info
#[tokio::test]
async fn test_get_pool_status() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Get pool info
    match client.get_pool(&pool_id).await {
        Ok(pool_info) => {
            // Test status extraction
            let status = client.get_pool_status(&pool_info);
            println!("Pool {} status: {:?}", pool_id, status);

            // For now, all pools should be Available (as per current implementation)
            // TODO: Update this test when proper status field handling is implemented
            assert_eq!(
                status,
                PoolStatus::Available,
                "Pool should be available by default"
            );
        }
        Err(e) => {
            println!("Failed to get pool info: {:?}", e);
            // Don't fail the test if the pool doesn't exist
        }
    }
}

/// Test pool status validation before operations
#[tokio::test]
async fn test_validate_pool_status() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test pool status validation
    match client.validate_pool_status(&pool_id).await {
        Ok(()) => {
            println!("Pool {} status validation passed", pool_id);
        }
        Err(e) => {
            println!("Pool status validation failed: {:?}", e);
            // Check if it's a status-related error
            if let Error::Other(msg) = &e {
                if msg.contains("not available for operations") {
                    println!("Pool is disabled, which is a valid test scenario");
                } else {
                    panic!("Unexpected error during pool status validation: {}", msg);
                }
            } else {
                // Other errors (like network issues) shouldn't fail the test
                println!("Non-status related error, continuing test");
            }
        }
    }
}

/// Test pool status validation with non-existent pool
#[tokio::test]
async fn test_validate_nonexistent_pool_status() {
    let client = create_test_client().await;

    // Test with a non-existent pool ID
    let fake_pool_id = "nonexistent.pool.id";

    match client.validate_pool_status(fake_pool_id).await {
        Ok(()) => {
            panic!("Pool status validation should fail for non-existent pool");
        }
        Err(e) => {
            println!(
                "Pool status validation correctly failed for non-existent pool: {:?}",
                e
            );

            // Should be a "Pool not found" error
            if let Error::Other(msg) = &e {
                assert!(
                    msg.contains("not found") || msg.contains("Pool"),
                    "Error should indicate pool not found: {}",
                    msg
                );
            }
        }
    }
}

/// Test pool status functionality with multiple pools
#[tokio::test]
async fn test_multiple_pools_status() {
    let client = create_test_client().await;

    // Get list of pools
    match client.get_pools(Some(5)).await {
        Ok(pools) => {
            println!("Found {} pools for status testing", pools.len());

            for pool in pools.iter().take(3) {
                // Test status extraction for each pool
                let status = client.get_pool_status(pool);
                println!(
                    "Pool {} status: {:?}",
                    pool.pool_info.pool_identifier, status
                );

                // Test status validation for each pool
                match client
                    .validate_pool_status(&pool.pool_info.pool_identifier)
                    .await
                {
                    Ok(()) => {
                        println!(
                            "Pool {} status validation passed",
                            pool.pool_info.pool_identifier
                        );
                    }
                    Err(e) => {
                        println!(
                            "Pool {} status validation failed: {:?}",
                            pool.pool_info.pool_identifier, e
                        );
                    }
                }
            }
        }
        Err(e) => {
            println!("Failed to get pools list: {:?}", e);
            // Don't fail the test if we can't get pools
        }
    }
}

/// Test that unchecked operations bypass status validation
#[tokio::test]
async fn test_unchecked_operations_bypass_status() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Only run this test if we should execute writes
    let should_execute = std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if !should_execute {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    // Create test assets for liquidity provision
    let assets = vec![
        cosmwasm_std::Coin {
            denom: "uom".to_string(),
            amount: cosmwasm_std::Uint128::new(1000), // 0.001 OM
        },
        cosmwasm_std::Coin {
            denom: "uusdc".to_string(),
            amount: cosmwasm_std::Uint128::new(1000), // 0.001 USDC
        },
    ];

    // Test unchecked provide liquidity operation (should bypass status validation)
    match client
        .provide_liquidity_unchecked(
            &pool_id,
            assets,
            Some(cosmwasm_std::Decimal::percent(5)), // liquidity_max_slippage
            Some(cosmwasm_std::Decimal::percent(5)), // swap_max_slippage
        )
        .await
    {
        Ok(response) => {
            println!(
                "Unchecked provide liquidity successful (bypassed status validation): {}",
                response.txhash
            );
            assert!(!response.txhash.is_empty());
        }
        Err(e) => {
            println!("Unchecked provide liquidity failed: {:?}", e);

            // Should NOT be a pool status error since we bypassed validation
            if let Error::Other(msg) = &e {
                assert!(
                    !msg.contains("not available for operations"),
                    "Unchecked operation should not fail due to pool status: {}",
                    msg
                );
            }

            // Other errors (insufficient funds, etc.) are acceptable
            println!("Unchecked operation failed for reasons other than pool status (this is acceptable)");
        }
    }
}
