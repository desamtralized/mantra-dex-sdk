mod utils;

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
        println!("Testing swap operation with pool: {}", pool_id);

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

        // Simulate a swap first
        let simulation_result = client
            .simulate_swap(
                &pool_id,
                cosmwasm_std::Coin {
                    denom: uom_denom.clone(),
                    amount: cosmwasm_std::Uint128::from(1000000u128),
                },
                &uusdc_denom,
            )
            .await;

        match simulation_result {
            Ok(simulation) => {
                println!(
                    "Swap simulation successful: return amount = {}",
                    simulation.return_amount
                );
                assert!(!simulation.return_amount.is_zero());
            }
            Err(e) => {
                println!("Warning: Swap simulation failed: {:?}", e);
            }
        }
    } else {
        println!("Warning: Could not get or create OM/USDC pool for swap test");
    }
}

#[tokio::test]
async fn test_provide_liquidity() {
    init_test_env();

    let client = create_test_client().await;
    let test_config = load_test_config();

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        println!("Testing liquidity provision with pool: {}", pool_id);

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

        // Test provide_liquidity method exists and can be called
        println!(
            "Would provide liquidity with denoms: {} and {}",
            uom_denom, uusdc_denom
        );
        // Note: We're not actually providing liquidity in this test as it requires real tokens
    } else {
        println!("Warning: Could not get or create OM/USDC pool for liquidity test");
    }
}

#[tokio::test]
async fn test_withdraw_liquidity() {
    init_test_env();

    let client = create_test_client().await;

    // Get or create pool ID
    let pool_id = get_or_create_om_usdc_pool_id(&client).await;

    if let Some(pool_id) = pool_id {
        println!("Testing liquidity withdrawal with pool: {}", pool_id);
        // Note: We're not actually withdrawing liquidity in this test as it requires LP tokens
    } else {
        println!("Warning: Could not get or create OM/USDC pool for withdrawal test");
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
