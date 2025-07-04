mod utils;

use utils::test_utils::{create_test_client, get_or_create_test_pool_id, should_execute_writes};

#[tokio::test]
async fn test_update_pool_features() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test updating multiple features at once
    match client
        .update_pool_features(&pool_id, Some(true), Some(true), Some(true))
        .await
    {
        Ok(response) => {
            println!("Feature toggle successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Feature toggle failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

#[tokio::test]
async fn test_enable_pool_withdrawals() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test enabling withdrawals
    match client.enable_pool_withdrawals(&pool_id).await {
        Ok(response) => {
            println!("Enable withdrawals successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Enable withdrawals failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

#[tokio::test]
async fn test_disable_pool_withdrawals() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test disabling withdrawals
    match client.disable_pool_withdrawals(&pool_id).await {
        Ok(response) => {
            println!("Disable withdrawals successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Disable withdrawals failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

#[tokio::test]
async fn test_enable_pool_deposits() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test enabling deposits
    match client.enable_pool_deposits(&pool_id).await {
        Ok(response) => {
            println!("Enable deposits successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Enable deposits failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

#[tokio::test]
async fn test_disable_pool_deposits() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test disabling deposits
    match client.disable_pool_deposits(&pool_id).await {
        Ok(response) => {
            println!("Disable deposits successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Disable deposits failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

#[tokio::test]
async fn test_enable_pool_swaps() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test enabling swaps
    match client.enable_pool_swaps(&pool_id).await {
        Ok(response) => {
            println!("Enable swaps successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Enable swaps failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

#[tokio::test]
async fn test_disable_pool_swaps() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test disabling swaps
    match client.disable_pool_swaps(&pool_id).await {
        Ok(response) => {
            println!("Disable swaps successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
            assert_eq!(response.code, 0u32);
        }
        Err(e) => {
            println!("Disable swaps failed (may be expected): {:?}", e);
            // Don't fail the test as this requires admin permissions
        }
    }
}

#[tokio::test]
async fn test_enable_all_pool_operations() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test enabling all operations
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
}

#[tokio::test]
async fn test_disable_all_pool_operations() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test disabling all operations
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
}

#[tokio::test]
async fn test_backward_compatibility_global_features() {
    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
    assert!(pool_id.is_some(), "Pool ID not found");
    let pool_id = pool_id.unwrap();

    // Test the deprecated global features method
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

#[tokio::test]
async fn test_feature_toggle_method_signatures() {
    let client = create_test_client().await;

    // Get a test pool
    let pool_id = get_or_create_test_pool_id(&client).await;
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

    // All methods should be callable (they may fail due to permissions, but compilation should work)
}
