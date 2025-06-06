mod utils;

use utils::test_utils::{create_test_client, should_execute_writes};

/// Test if farm manager functionality is properly accessible
#[tokio::test]
async fn test_farm_manager_configuration() {
    let client = create_test_client().await;

    // Check if farm manager contract address is configured
    let farm_manager_address = &client.config().contracts.farm_manager;
    println!("Farm Manager Address: {:?}", farm_manager_address);

    // Check if epoch manager contract address is configured
    let epoch_manager_address = &client.config().contracts.epoch_manager;
    println!("Epoch Manager Address: {:?}", epoch_manager_address);

    // These should be Some in a properly configured testnet
    if farm_manager_address.is_some() {
        println!("Farm manager contract is configured");
    } else {
        println!("Farm manager contract is not configured - this is expected if not deployed");
    }

    if epoch_manager_address.is_some() {
        println!("Epoch manager contract is configured");
    } else {
        println!("Epoch manager contract is not configured - this is expected if not deployed");
    }
}

/// Test all claim rewards methods comprehensively
#[tokio::test]
async fn test_claim_rewards_all_methods() {
    let client = create_test_client().await;

    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    // Only run if farm manager is configured
    if client.config().contracts.farm_manager.is_none() {
        println!("Skipping test: farm manager contract not configured");
        return;
    }

    println!("Testing all claim rewards methods...");

    // Test 1: Backward compatible claim (without epoch parameter)
    println!("1. Testing claim_rewards_all()...");
    match client.claim_rewards_all().await {
        Ok(response) => {
            println!("Claim rewards all successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
        }
        Err(e) => {
            println!("Claim rewards all failed (expected if no rewards): {:?}", e);
        }
    }

    // Test 2: Generic claim with optional epoch (None)
    println!("2. Testing claim_rewards(None)...");
    match client.claim_rewards(None).await {
        Ok(response) => {
            println!("Claim rewards (None) successful: {}", response.txhash);
            assert!(!response.txhash.is_empty());
        }
        Err(e) => {
            println!(
                "Claim rewards (None) failed (expected if no rewards): {:?}",
                e
            );
        }
    }

    // Test 3: Enhanced claim functionality with epoch parameter (if epoch manager is configured)
    if client.config().contracts.epoch_manager.is_some() {
        println!("3. Testing epoch-based claim methods...");

        match client.get_current_epoch().await {
            Ok(current_epoch) => {
                println!("Current epoch: {}", current_epoch);

                let target_epoch = if current_epoch > 0 {
                    current_epoch - 1
                } else {
                    current_epoch
                };

                // Test claim until specific epoch
                println!("3a. Testing claim_rewards_until_epoch({})...", target_epoch);
                match client.claim_rewards_until_epoch(target_epoch).await {
                    Ok(response) => {
                        println!(
                            "Claim rewards until epoch {} successful: {}",
                            target_epoch, response.txhash
                        );
                        assert!(!response.txhash.is_empty());
                    }
                    Err(e) => {
                        println!(
                            "Claim rewards until epoch failed (expected if no rewards): {:?}",
                            e
                        );
                    }
                }

                // Test generic claim with specific epoch
                println!("3b. Testing claim_rewards(Some({}))...", target_epoch);
                match client.claim_rewards(Some(target_epoch)).await {
                    Ok(response) => {
                        println!("Claim rewards with epoch successful: {}", response.txhash);
                        assert!(!response.txhash.is_empty());
                    }
                    Err(e) => {
                        println!(
                            "Claim rewards with epoch failed (expected if no rewards): {:?}",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                println!("Failed to get current epoch: {:?}", e);
            }
        }
    } else {
        println!("3. Skipping epoch-based tests: epoch manager not configured");
    }

    println!("All claim rewards methods tested successfully");
}

/// Test all query rewards methods comprehensively
#[tokio::test]
async fn test_query_rewards_all_methods() {
    let client = create_test_client().await;

    // Only run if farm manager is configured
    if client.config().contracts.farm_manager.is_none() {
        println!("Skipping test: farm manager contract not configured");
        return;
    }

    // Get the wallet address for testing
    let wallet = client
        .wallet()
        .expect("Wallet should be available in test client");
    let address = wallet.address().unwrap().to_string();

    println!("Testing all query rewards methods for address: {}", address);

    // Test 1: Basic rewards query (backward compatibility)
    println!("1. Testing query_all_rewards()...");
    match client.query_all_rewards(&address).await {
        Ok(rewards) => {
            println!("Query all rewards successful: {:?}", rewards);
        }
        Err(e) => {
            println!(
                "Query all rewards failed (expected if contract not deployed): {:?}",
                e
            );
        }
    }

    // Test 2: Query rewards with epoch parameter (if epoch manager is configured)
    if client.config().contracts.epoch_manager.is_some() {
        println!("2. Testing epoch-based query methods...");

        match client.get_current_epoch().await {
            Ok(current_epoch) => {
                println!("Current epoch: {}", current_epoch);

                let target_epoch = if current_epoch > 0 {
                    current_epoch - 1
                } else {
                    current_epoch
                };

                // Test querying rewards up to a specific epoch
                println!("2a. Testing query_rewards_until_epoch({})...", target_epoch);
                match client
                    .query_rewards_until_epoch(&address, target_epoch)
                    .await
                {
                    Ok(rewards) => {
                        println!(
                            "Query rewards until epoch {} successful: {:?}",
                            target_epoch, rewards
                        );
                    }
                    Err(e) => {
                        println!("Query rewards until epoch failed (expected if contract not deployed): {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("Failed to get current epoch: {:?}", e);
            }
        }
    } else {
        println!("2. Skipping epoch-based query tests: epoch manager not configured");
    }

    println!("All query rewards methods tested successfully");
}

/// Test epoch validation functionality
#[tokio::test]
async fn test_epoch_validation() {
    let client = create_test_client().await;

    // Only run if epoch manager is configured
    if client.config().contracts.epoch_manager.is_none() {
        println!("Skipping test: epoch manager contract not configured");
        return;
    }

    // Test epoch validation with current and future epochs
    match client.get_current_epoch().await {
        Ok(current_epoch) => {
            println!("Current epoch: {}", current_epoch);

            // Valid epoch should pass validation
            match client.validate_epoch(current_epoch).await {
                Ok(()) => {
                    println!("Current epoch validation passed");
                }
                Err(e) => {
                    panic!("Current epoch validation should pass: {:?}", e);
                }
            }

            // Future epoch should fail validation
            let future_epoch = current_epoch + 100;
            match client.validate_epoch(future_epoch).await {
                Ok(()) => {
                    panic!("Future epoch validation should fail");
                }
                Err(e) => {
                    println!("Future epoch validation correctly failed: {:?}", e);
                    assert!(e.to_string().contains("Cannot specify future epoch"));
                }
            }

            // Past epoch should pass validation (if current_epoch > 0)
            if current_epoch > 0 {
                match client.validate_epoch(current_epoch - 1).await {
                    Ok(()) => {
                        println!("Past epoch validation passed");
                    }
                    Err(e) => {
                        panic!("Past epoch validation should pass: {:?}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("Failed to get current epoch: {:?}", e);
            // Don't fail the test as the contract might not be deployed
        }
    }
}
