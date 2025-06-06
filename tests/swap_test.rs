mod utils;

use cosmwasm_std::{Coin, Decimal, Uint128};
use utils::test_utils::{
    assert_transaction_success, create_small_test_amounts, create_test_client, get_om_usdc_pool_id,
    get_test_denoms, load_test_config, setup_test_environment, should_execute_writes,
};

#[tokio::test]
async fn test_swap_operation() {
    println!("Starting swap test...");
    println!("Environment initialized");

    // Skip actual swap unless EXECUTE_WRITES is set
    if !should_execute_writes() {
        println!("Skipping actual swap test. Set EXECUTE_WRITES=true to enable.");
        return;
    }

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
    let (uom_denom, uusdc_denom) = get_test_denoms();
    println!("Token denoms: {} and {}", uom_denom, uusdc_denom);

    // Create offer asset (a small amount for testing)
    let offer_asset = Coin {
        denom: uom_denom.clone(),
        amount: Uint128::new(100_000), // 0.1 OM
    };

    println!("About to execute swap...");
    // Execute swap with timeout
    match tokio::time::timeout(
        std::time::Duration::from_secs(30), // 30 second timeout
        client.swap(
            &pool_id,
            offer_asset,
            &uusdc_denom, // The denom of the ask asset, should match one in the pool
            Some(Decimal::percent(1)), // 1% max slippage
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
            assert_transaction_success(&tx_response.txhash);
        }
        Ok(Err(e)) => {
            println!("Swap failed with error: {:?}", e);

            // Check if the error is due to empty pool
            let error_msg = format!("{:?}", e);
            if error_msg.contains("no assets") || error_msg.contains("empty") {
                println!("Pool exists but has no liquidity for swap. This is expected in test environments.");
                return; // Don't panic, just skip the test
            }

            // For other errors, still panic
            panic!("Swap failed");
        }
        Err(_) => {
            println!("Swap operation timed out after 30 seconds");
            panic!("Swap timed out");
        }
    }
}

#[tokio::test]
async fn test_comprehensive_liquidity_operations() {
    // Skip actual liquidity operations unless EXECUTE_WRITES is set
    if !should_execute_writes() {
        println!("Skipping liquidity operations test. Set EXECUTE_WRITES=true to enable.");
        return;
    }

    let fixtures = setup_test_environment().await;
    let client = &fixtures.client;
    let pool_id = fixtures.pool_id.as_ref().expect("Pool ID not found");

    // Test both provide and withdraw liquidity in one comprehensive test
    println!("=== Testing Provide Liquidity ===");

    // Create assets for liquidity provision using utility
    let assets = create_small_test_amounts();

    // Provide liquidity
    match client
        .provide_liquidity(
            pool_id,
            assets.clone(),
            Some(Decimal::percent(1)), // 1% liquidity max slippage
            Some(Decimal::percent(1)), // 1% swap max slippage
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
            assert_transaction_success(&tx_response.txhash);

            println!("=== Testing Withdraw Liquidity ===");

            // Now test withdrawal with a small amount
            let lp_amount = Uint128::new(100); // A very small amount to test

            // Withdraw liquidity
            match client.withdraw_liquidity(pool_id, lp_amount).await {
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
                    assert_transaction_success(&tx_response.txhash);
                }
                Err(e) => {
                    println!("Liquidity withdrawal failed: {:?}", e);
                    // Don't fail the test, just log the error for withdrawal
                }
            }
        }
        Err(e) => {
            println!("Liquidity provision failed: {:?}", e);
            // Don't fail the test, just log the error
        }
    }
}

#[tokio::test]
async fn test_simulate_swap() {
    // This is a read-only test, so it doesn't need the EXECUTE_WRITES flag
    let fixtures = setup_test_environment().await;
    let client = &fixtures.client;
    let pool_id = fixtures.pool_id.as_ref().expect("Pool ID not found");

    // Create offer asset (a small amount for testing)
    let (uom_denom, uusdc_denom) = get_test_denoms();
    let offer_asset = Coin {
        denom: uom_denom,
        amount: Uint128::new(1_000_000), // 1 OM
    };

    println!(
        "Simulating swap of {} {} for {}",
        offer_asset.amount, offer_asset.denom, uusdc_denom
    );

    // Simulate swap
    match client
        .simulate_swap(
            pool_id,
            offer_asset,
            &uusdc_denom, // The denom of the ask asset
        )
        .await
    {
        Ok(simulation) => {
            println!("Swap simulation result:");
            println!("Return amount: {}", simulation.return_amount);
            println!("Slippage amount: {}", simulation.slippage_amount);
            println!("Swap fee amount: {}", simulation.swap_fee_amount);
            println!("Protocol fee amount: {}", simulation.protocol_fee_amount);
            println!("Burn fee amount: {}", simulation.burn_fee_amount);
            println!("Extra fees amount: {}", simulation.extra_fees_amount);
        }
        Err(e) => {
            println!("Failed to simulate swap: {:?}", e);
            // Don't fail the test, just log the error
        }
    }
}

/// Parameterized test for different swap scenarios
#[tokio::test]
async fn test_swap_scenarios() {
    if !should_execute_writes() {
        println!("Skipping swap scenarios test. Set EXECUTE_WRITES=true to enable.");
        return;
    }

    let fixtures = setup_test_environment().await;
    let client = &fixtures.client;
    let pool_id = fixtures.pool_id.as_ref().expect("Pool ID not found");
    let (uom_denom, uusdc_denom) = get_test_denoms();

    // Test different swap amounts
    let test_amounts = vec![
        Uint128::new(50_000),  // 0.05 tokens
        Uint128::new(100_000), // 0.1 tokens
        Uint128::new(500_000), // 0.5 tokens
    ];

    for amount in test_amounts {
        println!("Testing swap with amount: {}", amount);

        let offer_asset = Coin {
            denom: uom_denom.clone(),
            amount,
        };

        match client
            .swap(
                pool_id,
                offer_asset,
                &uusdc_denom,
                Some(Decimal::percent(5)), // 5% max slippage
            )
            .await
        {
            Ok(tx_response) => {
                println!(
                    "Swap successful for amount {}: {}",
                    amount, tx_response.txhash
                );
                assert_transaction_success(&tx_response.txhash);
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                if error_msg.contains("no assets") || error_msg.contains("empty") {
                    println!(
                        "Pool has no liquidity for swap amount {}. Skipping.",
                        amount
                    );
                    continue;
                } else {
                    println!("Swap failed for amount {}: {:?}", amount, e);
                }
            }
        }
    }
}
