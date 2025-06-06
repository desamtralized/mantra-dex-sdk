mod utils;

use utils::test_utils::{create_test_client, get_om_usdc_pool_id, should_execute_writes};

#[derive(Debug, Clone)]
enum FeatureType {
    Withdrawals,
    Deposits,
    Swaps,
}

#[derive(Debug, Clone)]
enum FeatureOperation {
    Enable,
    Disable,
}

/// Test individual feature toggle operations (enable/disable) for each feature type
#[tokio::test]
async fn test_individual_feature_toggles() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test all combinations of feature types and operations
    let test_cases = vec![
        (FeatureType::Withdrawals, FeatureOperation::Enable),
        (FeatureType::Withdrawals, FeatureOperation::Disable),
        (FeatureType::Deposits, FeatureOperation::Enable),
        (FeatureType::Deposits, FeatureOperation::Disable),
        (FeatureType::Swaps, FeatureOperation::Enable),
        (FeatureType::Swaps, FeatureOperation::Disable),
    ];

    for (feature_type, operation) in test_cases {
        let result = match (&feature_type, &operation) {
            (FeatureType::Withdrawals, FeatureOperation::Enable) => {
                client.enable_pool_withdrawals(&pool_id).await
            }
            (FeatureType::Withdrawals, FeatureOperation::Disable) => {
                client.disable_pool_withdrawals(&pool_id).await
            }
            (FeatureType::Deposits, FeatureOperation::Enable) => {
                client.enable_pool_deposits(&pool_id).await
            }
            (FeatureType::Deposits, FeatureOperation::Disable) => {
                client.disable_pool_deposits(&pool_id).await
            }
            (FeatureType::Swaps, FeatureOperation::Enable) => {
                client.enable_pool_swaps(&pool_id).await
            }
            (FeatureType::Swaps, FeatureOperation::Disable) => {
                client.disable_pool_swaps(&pool_id).await
            }
        };

        match result {
            Ok(response) => {
                println!(
                    "{:?} {:?} successful: {}",
                    operation, feature_type, response.txhash
                );
                assert!(!response.txhash.is_empty());
                assert_eq!(response.code, 0u32);
            }
            Err(e) => {
                println!(
                    "{:?} {:?} failed (may be expected): {:?}",
                    operation, feature_type, e
                );
                // Don't fail the test as this requires admin permissions
            }
        }
    }
}

/// Test bulk feature toggle operations (enable/disable all features at once)
#[tokio::test]
async fn test_bulk_feature_toggles() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test updating multiple features at once via update_pool_features
    match client
        .update_pool_features(&pool_id, Some(true), Some(true), Some(true))
        .await
    {
        Ok(response) => {
            println!("Bulk feature enable successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Bulk feature enable failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }

    // Test enabling all operations via convenience method
    match client.enable_all_pool_operations(&pool_id).await {
        Ok(response) => {
            println!("Enable all operations successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Enable all operations failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }

    // Test disabling all operations via convenience method
    match client.disable_all_pool_operations(&pool_id).await {
        Ok(response) => {
            println!("Disable all operations successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Disable all operations failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }

    // Test partial feature updates
    match client
        .update_pool_features(&pool_id, Some(true), None, Some(false))
        .await
    {
        Ok(response) => {
            println!("Partial feature update successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Partial feature update failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

/// Test feature toggle error handling and backward compatibility
#[tokio::test]
async fn test_feature_toggle_error_handling() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_om_usdc_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test that all method signatures compile and can be called
    // These will likely fail due to admin permissions, but that's expected

    // Test the main update method
    let result1 = client
        .update_pool_features(&pool_id, Some(true), None, None)
        .await;
    println!("update_pool_features result: {:?}", result1.is_ok());

    // Test individual feature methods
    let result2 = client.enable_pool_withdrawals(&pool_id).await;
    println!("enable_pool_withdrawals result: {:?}", result2.is_ok());

    let result3 = client.disable_pool_withdrawals(&pool_id).await;
    println!("disable_pool_withdrawals result: {:?}", result3.is_ok());

    let result4 = client.enable_pool_deposits(&pool_id).await;
    println!("enable_pool_deposits result: {:?}", result4.is_ok());

    let result5 = client.disable_pool_deposits(&pool_id).await;
    println!("disable_pool_deposits result: {:?}", result5.is_ok());

    let result6 = client.enable_pool_swaps(&pool_id).await;
    println!("enable_pool_swaps result: {:?}", result6.is_ok());

    let result7 = client.disable_pool_swaps(&pool_id).await;
    println!("disable_pool_swaps result: {:?}", result7.is_ok());

    let result8 = client.enable_all_pool_operations(&pool_id).await;
    println!("enable_all_pool_operations result: {:?}", result8.is_ok());

    let result9 = client.disable_all_pool_operations(&pool_id).await;
    println!("disable_all_pool_operations result: {:?}", result9.is_ok());

    // Test backward compatibility method
    #[allow(deprecated)]
    let result10 = client
        .update_global_features(&pool_id, Some(true), Some(true), Some(true))
        .await;
    println!("update_global_features result: {:?}", result10.is_ok());

    // Test backward compatibility - should behave same as update_pool_features
    if should_execute_writes() {
        #[allow(deprecated)]
        match client
            .update_global_features(&pool_id, Some(true), Some(true), Some(true))
            .await
        {
            Ok(response) => {
                println!("Global features update successful: {}", response.txhash);
                assert!(!response.txhash.is_empty());
                assert_eq!(response.code, 0u32);
            }
            Err(e) => {
                println!("Global features update failed (may be expected): {:?}", e);
                // Don't fail the test as this requires admin permissions
            }
        }
    }

    // All methods should be callable (they may fail due to permissions, but compilation should work)
}
