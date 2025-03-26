use clap::Args;
use mantra_dex_sdk::Uint128;

use crate::config::CliConfig;
use crate::error::CliError;
use crate::utils::{create_client, format_amount, print_table};

#[derive(Args, Clone)]
pub struct BalanceCommand {
    /// Specific denom to check (optional)
    #[arg(short, long)]
    denom: Option<String>,
}

impl BalanceCommand {
    pub async fn execute(self, config: CliConfig) -> Result<(), CliError> {
        let client = create_client(&config).await?;
        
        // Get balances
        let balances = client.get_balances().await
            .map_err(CliError::Sdk)?;
            
        if balances.is_empty() {
            println!("No balances found.");
            return Ok(());
        }
        
        // Filter by denom if specified
        let filtered_balances = match self.denom {
            Some(ref denom) => balances.iter()
                .filter(|b| b.denom == *denom)
                .cloned()
                .collect(),
            None => balances,
        };
        
        if filtered_balances.is_empty() {
            println!("No balances found for the specified denom.");
            return Ok(());
        }
        
        // Format and display balances
        let mut rows = Vec::new();
        for balance in filtered_balances {
            // Convert to the SDK's Uint128 type
            let sdk_amount = Uint128::new(balance.amount.u128());
            let formatted_amount = format_amount(sdk_amount, &balance.denom, &config);
            
            rows.push(vec![
                balance.denom.clone(),
                balance.amount.to_string(),
                formatted_amount,
            ]);
        }
        
        print_table(
            vec!["Denom", "Raw Amount", "Formatted Amount"],
            rows
        );
        
        Ok(())
    }
} 