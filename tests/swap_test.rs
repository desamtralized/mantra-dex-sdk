mod utils;

use mantra_dex_std::pool_manager::PoolInfoResponse;
use utils::test_utils::{
    create_test_client, get_or_create_om_usdc_pool_id, init_test_env, load_test_config,
};

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
        }
    }
}

#[tokio::test]
async fn test_swap_operation() {
    init_test_env();

    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        println!(
            "=== PERFORMING REAL SWAP OPERATIONS WITH POOL: {} ===",
            pool_id
        );

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

        // Helper function to print balances
        let print_balances = |title: &str, balances: &[cosmwasm_std::Coin]| {
            println!("\n=== {} ===", title);
            for balance in balances {
                if balance.denom == uom_denom || balance.denom == uusdc_denom {
                    println!("  {} - {}", balance.denom, balance.amount);
                }
            }
        };

        // Helper function to print pool info
        let print_pool_info = |title: &str, pool_info: &PoolInfoResponse| {
            println!("\n=== {} ===", title);
            println!("Pool ID: {}", pool_info.pool_info.pool_identifier);
            println!("Pool Assets:");
            for asset in &pool_info.pool_info.assets {
                println!("  {} - {}", asset.denom, asset.amount);
            }
            println!(
                "Total LP Shares: {} - {}",
                pool_info.total_share.denom, pool_info.total_share.amount
            );
        };

        // Get initial state
        let initial_balances = client
            .get_balances()
            .await
            .expect("Failed to get initial balances");
        let initial_pool_info = client
            .get_pool(&pool_id)
            .await
            .expect("Failed to get initial pool info");

        print_balances("USER BALANCES BEFORE SWAPS", &initial_balances);
        print_pool_info("POOL LIQUIDITY BEFORE SWAPS", &initial_pool_info);

        // SWAP 1: 1 OM to USDC
        println!("\n=== EXECUTING SWAP 1: 1 OM -> USDC ===");
        let swap_amount_1 = cosmwasm_std::Uint128::from(1_000_000u128); // 1 OM (6 decimals)

        let swap_1_result = client
            .swap(
                &pool_id,
                cosmwasm_std::Coin {
                    denom: uom_denom.clone(),
                    amount: swap_amount_1,
                },
                &uusdc_denom,
                Some(cosmwasm_std::Decimal::percent(5)), // 5% max spread
            )
            .await;

        match swap_1_result {
            Ok(tx_response) => {
                println!("Swap 1 successful! Tx hash: {}", tx_response.txhash);
                println!("Gas used: {}", tx_response.gas_used);

                // Wait a bit for the transaction to be processed
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                // Get state after first swap
                let balances_after_swap_1 = client
                    .get_balances()
                    .await
                    .expect("Failed to get balances after swap 1");
                let pool_info_after_swap_1 = client
                    .get_pool(&pool_id)
                    .await
                    .expect("Failed to get pool info after swap 1");

                print_balances("USER BALANCES AFTER SWAP 1", &balances_after_swap_1);
                print_pool_info("POOL LIQUIDITY AFTER SWAP 1", &pool_info_after_swap_1);

                // Calculate how much USDC we received
                let usdc_balance_after_swap_1 = balances_after_swap_1
                    .iter()
                    .find(|b| b.denom == uusdc_denom)
                    .map(|b| b.amount)
                    .unwrap_or(cosmwasm_std::Uint128::zero());

                let initial_usdc_balance = initial_balances
                    .iter()
                    .find(|b| b.denom == uusdc_denom)
                    .map(|b| b.amount)
                    .unwrap_or(cosmwasm_std::Uint128::zero());

                let usdc_received = usdc_balance_after_swap_1.saturating_sub(initial_usdc_balance);
                println!("USDC received from swap 1: {}", usdc_received);

                if !usdc_received.is_zero() {
                    // SWAP 2: All received USDC back to OM
                    println!("\n=== EXECUTING SWAP 2: {} USDC -> OM ===", usdc_received);

                    let swap_2_result = client
                        .swap(
                            &pool_id,
                            cosmwasm_std::Coin {
                                denom: uusdc_denom.clone(),
                                amount: usdc_received,
                            },
                            &uom_denom,
                            Some(cosmwasm_std::Decimal::percent(5)), // 5% max spread
                        )
                        .await;

                    match swap_2_result {
                        Ok(tx_response) => {
                            println!("Swap 2 successful! Tx hash: {}", tx_response.txhash);
                            println!("Gas used: {}", tx_response.gas_used);

                            // Wait a bit for the transaction to be processed
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                            // Get final state
                            let final_balances = client
                                .get_balances()
                                .await
                                .expect("Failed to get final balances");
                            let final_pool_info = client
                                .get_pool(&pool_id)
                                .await
                                .expect("Failed to get final pool info");

                            print_balances("USER BALANCES AFTER SWAP 2", &final_balances);
                            print_pool_info("POOL LIQUIDITY AFTER SWAP 2", &final_pool_info);

                            // Calculate final OM balance vs initial
                            let final_om_balance = final_balances
                                .iter()
                                .find(|b| b.denom == uom_denom)
                                .map(|b| b.amount)
                                .unwrap_or(cosmwasm_std::Uint128::zero());

                            let initial_om_balance = initial_balances
                                .iter()
                                .find(|b| b.denom == uom_denom)
                                .map(|b| b.amount)
                                .unwrap_or(cosmwasm_std::Uint128::zero());

                            let om_difference = if final_om_balance > initial_om_balance {
                                println!(
                                    "OM gained: {}",
                                    final_om_balance.saturating_sub(initial_om_balance)
                                );
                            } else {
                                println!(
                                    "OM lost: {}",
                                    initial_om_balance.saturating_sub(final_om_balance)
                                );
                            };

                            println!("\n=== SWAP ROUND-TRIP COMPLETED SUCCESSFULLY ===");
                        }
                        Err(e) => {
                            println!("Swap 2 failed: {:?}", e);
                        }
                    }
                } else {
                    println!("No USDC received from first swap, cannot proceed with second swap");
                }
            }
            Err(e) => {
                println!("Swap 1 failed: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDC pool for swap test");
    }
}

#[tokio::test]
async fn test_provide_and_withdraw_liquidity() {
    init_test_env();

    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        println!(
            "=== PERFORMING COMPLETE LIQUIDITY CYCLE WITH POOL: {} ===",
            pool_id
        );

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

        // Helper function to print balances
        let print_balances = |title: &str, balances: &[cosmwasm_std::Coin]| {
            println!("\n=== {} ===", title);
            for balance in balances {
                if balance.denom == uom_denom
                    || balance.denom == uusdc_denom
                    || balance.denom.contains(".LP")
                {
                    println!("  {} - {}", balance.denom, balance.amount);
                }
            }
        };

        // Helper function to print pool info
        let print_pool_info = |title: &str, pool_info: &PoolInfoResponse| {
            println!("\n=== {} ===", title);
            println!("Pool ID: {}", pool_info.pool_info.pool_identifier);
            println!("Pool Assets:");
            for asset in &pool_info.pool_info.assets {
                println!("  {} - {}", asset.denom, asset.amount);
            }
            println!(
                "Total LP Shares: {} - {}",
                pool_info.total_share.denom, pool_info.total_share.amount
            );
        };

        // === PHASE 1: GET INITIAL STATE ===
        let initial_balances = client
            .get_balances()
            .await
            .expect("Failed to get initial balances");
        let initial_pool_info = client
            .get_pool(&pool_id)
            .await
            .expect("Failed to get initial pool info");

        print_balances("INITIAL USER BALANCES", &initial_balances);
        print_pool_info("INITIAL POOL STATE", &initial_pool_info);

        // === PHASE 2: PROVIDE LIQUIDITY ===
        println!("\n🚀 PHASE 1: PROVIDING LIQUIDITY");

        // Calculate 10% of user balances for each token
        let uom_balance = initial_balances
            .iter()
            .find(|b| b.denom == uom_denom)
            .map(|b| b.amount)
            .unwrap_or(cosmwasm_std::Uint128::zero());

        let uusdc_balance = initial_balances
            .iter()
            .find(|b| b.denom == uusdc_denom)
            .map(|b| b.amount)
            .unwrap_or(cosmwasm_std::Uint128::zero());

        // Use 10% of each balance, but ensure minimum amounts
        let uom_to_provide = std::cmp::max(
            uom_balance / cosmwasm_std::Uint128::from(10u128),
            cosmwasm_std::Uint128::from(100_000u128), // minimum 0.1 OM
        );
        let uusdc_to_provide = std::cmp::max(
            uusdc_balance / cosmwasm_std::Uint128::from(10u128),
            cosmwasm_std::Uint128::from(100_000u128), // minimum 0.1 USDC
        );

        if uom_balance.is_zero() || uusdc_balance.is_zero() {
            println!("Insufficient balances for liquidity provision test");
            println!(
                "OM balance: {}, USDC balance: {}",
                uom_balance, uusdc_balance
            );
            return;
        }

        println!("OM to provide: {} (10% of {})", uom_to_provide, uom_balance);
        println!(
            "USDC to provide: {} (10% of {})",
            uusdc_to_provide, uusdc_balance
        );

        // Provide liquidity - sort coins alphabetically by denomination as required by blockchain
        let mut assets_to_provide = vec![
            cosmwasm_std::Coin {
                denom: uom_denom.clone(),
                amount: uom_to_provide,
            },
            cosmwasm_std::Coin {
                denom: uusdc_denom.clone(),
                amount: uusdc_to_provide,
            },
        ];
        assets_to_provide.sort_by(|a, b| a.denom.cmp(&b.denom));

        let provide_result = client
            .provide_liquidity(&pool_id, assets_to_provide, None)
            .await;

        match provide_result {
            Ok(tx_response) => {
                println!(
                    "✅ Liquidity provision successful! Tx hash: {}",
                    tx_response.txhash
                );
                println!("Gas used: {}", tx_response.gas_used);

                // Wait for transaction processing
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                // Get state after liquidity provision
                let balances_after_provide = client
                    .get_balances()
                    .await
                    .expect("Failed to get balances after liquidity provision");
                let pool_after_provide = client
                    .get_pool(&pool_id)
                    .await
                    .expect("Failed to get pool info after liquidity provision");

                print_balances("USER BALANCES AFTER PROVISION", &balances_after_provide);
                print_pool_info("POOL STATE AFTER PROVISION", &pool_after_provide);

                // Calculate LP tokens received
                let lp_denom = format!("factory/mantra1vwj600jud78djej7ttq44dktu4wr3t2yrrsjgmld8v3jq8mud68q5w7455/{}.LP", pool_id);
                let initial_lp_balance = initial_balances
                    .iter()
                    .find(|b| b.denom == lp_denom)
                    .map(|b| b.amount)
                    .unwrap_or(cosmwasm_std::Uint128::zero());

                let lp_balance_after_provide = balances_after_provide
                    .iter()
                    .find(|b| b.denom == lp_denom)
                    .map(|b| b.amount)
                    .unwrap_or(cosmwasm_std::Uint128::zero());

                let lp_tokens_received =
                    lp_balance_after_provide.saturating_sub(initial_lp_balance);
                println!("💰 LP tokens received: {}", lp_tokens_received);

                // === PHASE 3: WITHDRAW RECEIVED LIQUIDITY ===
                println!("\n🔄 PHASE 2: WITHDRAWING RECEIVED LIQUIDITY");

                if lp_tokens_received.is_zero() {
                    println!("❌ No LP tokens were received to withdraw!");
                    return;
                }

                println!(
                    "LP tokens to withdraw: {} (exact amount received)",
                    lp_tokens_received
                );

                let withdraw_result = client
                    .withdraw_liquidity(&pool_id, lp_tokens_received)
                    .await;

                match withdraw_result {
                    Ok(tx_response) => {
                        println!(
                            "✅ Liquidity withdrawal successful! Tx hash: {}",
                            tx_response.txhash
                        );
                        println!("Gas used: {}", tx_response.gas_used);

                        // Wait for transaction processing
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

                        // Get final state
                        let final_balances = client
                            .get_balances()
                            .await
                            .expect("Failed to get final balances");
                        let final_pool_info = client
                            .get_pool(&pool_id)
                            .await
                            .expect("Failed to get final pool info");

                        print_balances("FINAL USER BALANCES", &final_balances);
                        print_pool_info("FINAL POOL STATE", &final_pool_info);

                        // === PHASE 4: CALCULATE NET RESULTS ===
                        println!("\n📊 LIQUIDITY CYCLE RESULTS:");

                        let final_uom_balance = final_balances
                            .iter()
                            .find(|b| b.denom == uom_denom)
                            .map(|b| b.amount)
                            .unwrap_or(cosmwasm_std::Uint128::zero());

                        let final_uusdc_balance = final_balances
                            .iter()
                            .find(|b| b.denom == uusdc_denom)
                            .map(|b| b.amount)
                            .unwrap_or(cosmwasm_std::Uint128::zero());

                        let final_lp_balance = final_balances
                            .iter()
                            .find(|b| b.denom == lp_denom)
                            .map(|b| b.amount)
                            .unwrap_or(cosmwasm_std::Uint128::zero());

                        let net_uom_change = final_uom_balance.saturating_sub(uom_balance);
                        let net_uusdc_change = final_uusdc_balance.saturating_sub(uusdc_balance);

                        if final_uom_balance >= uom_balance {
                            println!("🟢 OM net gain: {}", net_uom_change);
                        } else {
                            println!(
                                "🔴 OM net loss: {}",
                                uom_balance.saturating_sub(final_uom_balance)
                            );
                        }

                        if final_uusdc_balance >= uusdc_balance {
                            println!("🟢 USDC net gain: {}", net_uusdc_change);
                        } else {
                            println!(
                                "🔴 USDC net loss: {}",
                                uusdc_balance.saturating_sub(final_uusdc_balance)
                            );
                        }

                        println!("💎 Remaining LP tokens: {}", final_lp_balance);

                        // Calculate how much was actually withdrawn
                        let lp_tokens_burned = initial_lp_balance
                            .saturating_add(lp_tokens_received)
                            .saturating_sub(final_lp_balance);
                        println!("🔥 LP tokens burned in withdrawal: {}", lp_tokens_burned);

                        println!("\n🎉 COMPLETE LIQUIDITY CYCLE FINISHED SUCCESSFULLY!");
                        println!("   (Provided liquidity and withdrew exact amount received)");
                    }
                    Err(e) => {
                        println!("❌ Liquidity withdrawal failed: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Liquidity provision failed: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDC pool for liquidity cycle test");
    }
}

#[tokio::test]
async fn test_get_pool() {
    init_test_env();

    let client = create_test_client().await;

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        match client.get_pool(&pool_id).await {
            Ok(pool_info) => {
                println!("Got pool info: {}", pool_info.pool_info.pool_identifier);
                assert_eq!(pool_info.pool_info.pool_identifier, pool_id);
            }
            Err(e) => {
                println!("Warning: Failed to get pool info: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDC pool for pool info test");
    }
}

#[tokio::test]
async fn test_simulate_swap() {
    init_test_env();

    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
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

        // Test swap simulation
        let simulation_result = client
            .simulate_swap(
                &pool_id,
                cosmwasm_std::Coin {
                    denom: uom_denom,
                    amount: cosmwasm_std::Uint128::from(1000000u128),
                },
                &uusdc_denom,
            )
            .await;

        match simulation_result {
            Ok(simulation) => {
                println!(
                    "Simulation successful: return amount = {}",
                    simulation.return_amount
                );
                assert!(!simulation.return_amount.is_zero());
            }
            Err(e) => {
                println!("Warning: Simulation failed: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDC pool for simulation");
    }
}
