use clap::Args;
use mantra_dex_sdk::Decimal;

use crate::config::CliConfig;
use crate::error::CliError;
use crate::utils::{create_client, parse_coin, parse_decimal, print_success};

#[derive(Args, Clone)]
pub struct SwapCommand {
    /// Pool ID
    #[arg(short, long)]
    pool_id: String,

    /// Offer asset in the format "amount:denom" (e.g., "1000:uom")
    #[arg(short, long)]
    offer_asset: String,

    /// Denomination of the ask asset
    #[arg(short, long)]
    ask_denom: String,

    /// Maximum spread (slippage) allowed (e.g., "0.01" for 1%)
    #[arg(short, long, default_value = "0.01")]
    max_spread: String,

    /// Skip confirmation prompt
    #[arg(short, long)]
    yes: bool,

    /// Only simulate the swap without executing it
    #[arg(short, long)]
    simulate: bool,
}

impl SwapCommand {
    pub async fn execute(self, config: CliConfig) -> Result<(), CliError> {
        let client = create_client(&config).await?;

        // Parse offer asset
        let offer_asset = parse_coin(&self.offer_asset)
            .map_err(|e| CliError::Parse(format!("Invalid offer asset: {}", e)))?;

        // Parse max spread
        let max_spread = parse_decimal(&self.max_spread)
            .map_err(|e| CliError::Parse(format!("Invalid max spread: {}", e)))?;

        if self.simulate {
            // Simulate the swap
            println!("Simulating swap of {} to {}", offer_asset, self.ask_denom);

            let simulation = client
                .simulate_swap(&self.pool_id, offer_asset.clone(), &self.ask_denom)
                .await
                .map_err(CliError::Sdk)?;

            println!("\nSimulation Results:");
            println!("Return Amount: {}", simulation.return_amount);
            println!("Spread Amount: {}", simulation.spread_amount);
            println!("Swap Fee: {}", simulation.swap_fee_amount);
            println!("Protocol Fee: {}", simulation.protocol_fee_amount);

            // Calculate effective price
            let input_amount = offer_asset.amount;
            let output_amount = simulation.return_amount;

            if !output_amount.is_zero() {
                let price_ratio = Decimal::from_ratio(input_amount, output_amount);
                println!(
                    "\nEffective Price: {} {} per {}",
                    price_ratio, offer_asset.denom, self.ask_denom
                );

                let inverse_price = Decimal::from_ratio(output_amount, input_amount);
                println!(
                    "Inverse Price: {} {} per {}",
                    inverse_price, self.ask_denom, offer_asset.denom
                );
            }

            Ok(())
        } else {
            // Execute the swap
            println!("Swapping {} to {}", offer_asset, self.ask_denom);
            println!("Maximum spread: {}", max_spread);

            let result = client
                .swap(
                    &self.pool_id,
                    offer_asset.clone(),
                    &self.ask_denom,
                    Some(max_spread),
                )
                .await
                .map_err(CliError::Sdk)?;

            print_success(&format!(
                "Swap completed: {} → {} {}",
                offer_asset, result.txhash, self.ask_denom
            ));

            Ok(())
        }
    }
}
