use cosmwasm_std::{Coin, Decimal, Uint128};
use mantra_dex_sdk::{
    client::MantraDexClient, config::MantraNetworkConfig, error::Error, wallet::MantraWallet,
};
use std::str::FromStr;

/// Comprehensive example demonstrating MANTRA DEX SDK v3.0.0 features
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize configuration and client
    let config = MantraNetworkConfig::testnet();
    let client = MantraDexClient::new(config).await?;

    // Create wallet from mnemonic
    let wallet = MantraWallet::from_mnemonic("your mnemonic phrase here", 0)?;
    let client = client.with_wallet(wallet);

    println!("=== MANTRA DEX SDK v3.0.0 Examples ===\n");

    // Example 1: Basic Operations with Updated Parameters
    basic_operations_example(&client).await?;

    // Example 2: Enhanced Fee Structure
    fee_structure_example(&client).await?;

    // Example 3: Pool Status Management
    pool_status_example(&client).await?;

    // Example 4: Epoch-based Rewards
    epoch_rewards_example(&client).await?;

    // Example 5: Per-Pool Feature Toggles
    feature_toggle_example(&client).await?;

    Ok(())
}

/// Example 1: Basic operations with updated parameter names
async fn basic_operations_example(client: &MantraDexClient) -> Result<(), Error> {
    println!("1. Basic Operations with Updated Parameters");

    let pool_id = "pool_1";
    let offer_asset = Coin {
        denom: "uom".to_string(),
        amount: Uint128::new(1000000), // 1 OM
    };

    // Swap with new parameter name: max_slippage (was max_spread in v2.x)
    println!("   - Performing swap with max_slippage parameter");
    let max_slippage = Some(Decimal::percent(5)); // 5% slippage tolerance
    let _swap_result = client
        .swap(pool_id, offer_asset.clone(), "uusdc", max_slippage)
        .await;

    // Provide liquidity with new parameter names
    println!("   - Providing liquidity with new parameter names");
    let assets = vec![
        Coin {
            denom: "uom".to_string(),
            amount: Uint128::new(1000000),
        },
        Coin {
            denom: "uusdc".to_string(),
            amount: Uint128::new(1000000),
        },
    ];

    // New parameter names: liquidity_max_slippage and swap_max_slippage
    let liquidity_max_slippage = Some(Decimal::percent(2)); // 2% for liquidity operations
    let swap_max_slippage = Some(Decimal::percent(3)); // 3% for internal swaps

    let _liquidity_result = client
        .provide_liquidity(pool_id, assets, liquidity_max_slippage, swap_max_slippage)
        .await;

    println!("   ✓ Basic operations completed\n");
    Ok(())
}

/// Example 2: Enhanced fee structure with validation
async fn fee_structure_example(client: &MantraDexClient) -> Result<(), Error> {
    println!("2. Enhanced Fee Structure");

    // Create validated pool fees using the new structure
    println!("   - Creating validated pool fees");
    let protocol_fee = Decimal::percent(1); // 1%
    let swap_fee = Decimal::percent(3); // 3%
    let burn_fee = Some(Decimal::percent(1)); // 1%
    let extra_fees = Some(vec![
        Decimal::percent(2), // 2% extra fee
        Decimal::percent(1), // 1% additional fee
    ]);

    let pool_fees =
        client.create_validated_pool_fees(protocol_fee, swap_fee, burn_fee, extra_fees)?;

    println!("   - Validating fee structure (total must be ≤ 20%)");
    client.validate_pool_fees(&pool_fees)?;

    // Create pool with validated fees
    println!("   - Creating pool with validated fee structure");
    let asset_denoms = vec!["uom".to_string(), "uusdc".to_string()];
    let asset_decimals = vec![6, 6];
    let pool_type = mantra_dex_std::pool_manager::PoolType::ConstantProduct;
    let pool_identifier = Some("example_pool_v3".to_string());

    let _create_result = client
        .create_pool(
            asset_denoms,
            asset_decimals,
            pool_fees,
            pool_type,
            pool_identifier,
        )
        .await;

    println!("   ✓ Fee structure validation completed\n");
    Ok(())
}

/// Example 3: Pool status management
async fn pool_status_example(client: &MantraDexClient) -> Result<(), Error> {
    println!("3. Pool Status Management");

    let pool_id = "pool_1";

    // Check pool status before operations
    println!("   - Validating pool status before operations");
    client.validate_pool_status(pool_id).await?;

    // Get pool information including status
    println!("   - Retrieving pool information with status");
    let pool_info = client.get_pool(pool_id).await?;
    let status = client.get_pool_status(&pool_info);

    println!("   - Pool status: {:?}", status);
    if status.is_available() {
        println!("   - Pool is available for operations");
    } else {
        println!("   - Pool is disabled for operations");
    }

    println!("   ✓ Pool status validation completed\n");
    Ok(())
}

/// Example 4: Epoch-based rewards functionality
async fn epoch_rewards_example(client: &MantraDexClient) -> Result<(), Error> {
    println!("4. Epoch-based Rewards");

    let address = client.wallet()?.address()?.to_string();

    // Get current epoch
    println!("   - Getting current epoch");
    let current_epoch = client.get_current_epoch().await?;
    println!("   - Current epoch: {}", current_epoch);

    // Query rewards with epoch parameter
    println!("   - Querying rewards up to specific epoch");
    let until_epoch = current_epoch.saturating_sub(1); // Previous epoch
    let _rewards = client
        .query_rewards_until_epoch(&address, until_epoch)
        .await;

    // Query all rewards (backward compatibility)
    println!("   - Querying all rewards (backward compatibility)");
    let _all_rewards = client.query_all_rewards(&address).await;

    // Claim rewards with epoch parameter
    println!("   - Claiming rewards up to specific epoch");
    let _claim_result = client.claim_rewards_until_epoch(until_epoch).await;

    // Claim all rewards (backward compatibility)
    println!("   - Claiming all rewards (backward compatibility)");
    let _claim_all_result = client.claim_rewards_all().await;

    // Validate epoch parameter
    println!("   - Validating epoch parameter");
    client.validate_epoch(until_epoch).await?;

    println!("   ✓ Epoch-based rewards functionality completed\n");
    Ok(())
}

/// Example 5: Per-pool feature toggles
async fn feature_toggle_example(client: &MantraDexClient) -> Result<(), Error> {
    println!("5. Per-Pool Feature Toggles");

    let pool_identifier = "example_pool_v3";

    // Update specific pool features
    println!("   - Updating pool-specific features");
    let _update_result = client
        .update_pool_features(
            pool_identifier,
            Some(true),  // Enable withdrawals
            Some(true),  // Enable deposits
            Some(false), // Disable swaps
        )
        .await;

    // Individual feature controls
    println!("   - Enabling individual pool operations");
    let _enable_swaps = client.enable_pool_swaps(pool_identifier).await;
    let _enable_deposits = client.enable_pool_deposits(pool_identifier).await;
    let _enable_withdrawals = client.enable_pool_withdrawals(pool_identifier).await;

    // Bulk operations
    println!("   - Enabling all pool operations");
    let _enable_all = client.enable_all_pool_operations(pool_identifier).await;

    println!("   - Disabling all pool operations");
    let _disable_all = client.disable_all_pool_operations(pool_identifier).await;

    // Individual disable operations
    println!("   - Disabling individual pool operations");
    let _disable_swaps = client.disable_pool_swaps(pool_identifier).await;
    let _disable_deposits = client.disable_pool_deposits(pool_identifier).await;
    let _disable_withdrawals = client.disable_pool_withdrawals(pool_identifier).await;

    println!("   ✓ Per-pool feature toggles completed\n");
    Ok(())
}

/// Helper function for transaction execution (from original example)
async fn execute_transaction_example(client: &MantraDexClient) -> Result<String, Error> {
    // This demonstrates the transaction execution pattern
    // that can be used with any of the above operations

    let wallet = client.wallet()?;
    let address = wallet.address()?.to_string();

    // Example: Get balances to show wallet integration
    let balances = client.get_balances().await?;
    println!("Wallet balances: {:?}", balances);

    // Example: Get last block height
    let height = client.get_last_block_height().await?;
    println!("Last block height: {}", height);

    Ok("Transaction executed successfully".to_string())
}
