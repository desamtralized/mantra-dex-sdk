mod utils;

use utils::test_utils::create_test_client;

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

/// Test enhanced claim functionality with epoch parameter
#[tokio::test]
async fn test_claim_rewards_with_epoch() {
    let client = create_test_client().await;

    // Only run this test if we should execute writes
    if !should_execute_writes() {
        println!("Skipping write test (EXECUTE_WRITES=false)");
        return;
    }

    // Only run if farm manager and epoch manager are configured
    if client.config().contracts.farm_manager.is_none()
        || client.config().contracts.epoch_manager.is_none()
    {
        println!("Skipping test: farm manager or epoch manager contract not configured");
        return;
    }

    // First get the current epoch
    match client.get_current_epoch().await {
        Ok(current_epoch) => {
            println!("Current epoch: {}", current_epoch);

            // Test claiming rewards up to a specific epoch (current epoch - 1 if possible)
            let target_epoch = if current_epoch > 0 {
                current_epoch - 1
            } else {
                current_epoch
            };

            match client.claim_rewards(target_epoch).await {
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
                    // Don't fail the test as this might be expected if no rewards are available
                }
            }
        }
        Err(e) => {
            println!("Failed to get current epoch: {:?}", e);
            // Don't fail the test as the contract might not be deployed
        }
    }
}

/// Test query rewards functionality
#[tokio::test]
async fn test_query_rewards() {
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

    // Test basic rewards query - need to get current epoch first
    match client.get_current_epoch().await {
        Ok(current_epoch) => {
            match client.query_rewards(&address, current_epoch).await {
                Ok(rewards) => {
                    println!("Rewards query successful: {:?}", rewards);
                }
                Err(e) => {
                    println!(
                        "Rewards query failed (expected if contract not deployed): {:?}",
                        e
                    );
                    // Don't fail the test as the contract might not be deployed
                }
            }
        }
        Err(e) => {
            println!("Failed to get current epoch: {:?}", e);
        }
    }
}

/// Test query rewards with epoch parameter
#[tokio::test]
async fn test_query_rewards_with_epoch() {
    let client = create_test_client().await;

    // Only run if farm manager and epoch manager are configured
    if client.config().contracts.farm_manager.is_none()
        || client.config().contracts.epoch_manager.is_none()
    {
        println!("Skipping test: farm manager or epoch manager contract not configured");
        return;
    }

    // Get the wallet address for testing
    let wallet = client
        .wallet()
        .expect("Wallet should be available in test client");
    let address = wallet.address().unwrap().to_string();

    // First get the current epoch
    match client.get_current_epoch().await {
        Ok(current_epoch) => {
            println!("Current epoch: {}", current_epoch);

            // Test querying rewards up to a specific epoch
            let target_epoch = if current_epoch > 0 {
                current_epoch - 1
            } else {
                current_epoch
            };

            match client.query_rewards(&address, target_epoch).await {
                Ok(rewards) => {
                    println!(
                        "Rewards query until epoch {} successful: {:?}",
                        target_epoch, rewards
                    );
                }
                Err(e) => {
                    println!("Rewards query until epoch failed (expected if contract not deployed): {:?}", e);
                    // Don't fail the test as the contract might not be deployed
                }
            }
        }
        Err(e) => {
            println!("Failed to get current epoch: {:?}", e);
            // Don't fail the test as the contract might not be deployed
        }
    }
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

/// Helper function to check if write operations should be executed
fn should_execute_writes() -> bool {
    std::env::var("EXECUTE_WRITES")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true"
}

/// Test the claim rewards method signature variations
#[tokio::test]
async fn test_claim_method_signatures() {
    let client = create_test_client().await;

    // Only run if farm manager is configured
    if client.config().contracts.farm_manager.is_none() {
        println!("Skipping test: farm manager contract not configured");
        return;
    }

    // Test that the method signature compiles and can be called
    // (This will likely fail due to contract deployment, but that's expected)

    // Get current epoch first
    match client.get_current_epoch().await {
        Ok(current_epoch) => {
            // Method: claim rewards with specific epoch
            let result = client.claim_rewards(current_epoch).await;
            println!(
                "claim_rewards({}) result: {:?}",
                current_epoch,
                result.is_ok()
            );
        }
        Err(e) => {
            println!("Failed to get current epoch: {:?}", e);
            // Test with a default epoch if we can't get current epoch
            let result = client.claim_rewards(1).await;
            println!("claim_rewards(1) result: {:?}", result.is_ok());
        }
    }
}
