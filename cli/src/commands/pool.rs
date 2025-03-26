use clap::{Args, Subcommand};

use crate::config::CliConfig;
use crate::error::CliError;
use crate::utils::{create_client, format_amount, print_table};

#[derive(Args, Clone)]
pub struct PoolCommand {
    #[command(subcommand)]
    pub command: PoolCommands,
}

#[derive(Subcommand, Clone)]
pub enum PoolCommands {
    /// List all pools
    List {
        /// Maximum number of pools to show
        #[arg(short, long)]
        limit: Option<u32>,
    },
    
    /// Get information about a specific pool
    Info {
        /// Pool ID
        id: String,
    },
}

impl PoolCommand {
    pub async fn execute(self, config: CliConfig) -> Result<(), CliError> {
        // Create client
        let client = create_client(&config).await?;
        
        match self.command {
            PoolCommands::List { limit } => {
                // Get pools
                let pools = client.get_pools(limit).await
                    .map_err(|e| CliError::Sdk(e))?;
                
                if pools.is_empty() {
                    println!("No pools found.");
                    return Ok(());
                }
                
                // Print pools
                let mut rows = Vec::new();
                for pool in pools {
                    // Format pool data
                    rows.push(vec![
                        pool.pool_info.pool_identifier.clone(),
                        format!("{:?}", pool.pool_info.pool_type),
                        pool.pool_info.asset_denoms.iter()
                            .map(|d| d.clone())
                            .collect::<Vec<_>>()
                            .join(", "),
                        pool.total_share.to_string(),
                    ]);
                }
                
                print_table(vec!["Pool ID", "Type", "Assets", "Total Shares"], rows);
                Ok(())
            },
            
            PoolCommands::Info { id } => {
                // Get pool info
                let pool = client.get_pool(&id).await
                    .map_err(|e| CliError::Sdk(e))?;
                
                // Print pool details
                println!("\nPool ID: {}", pool.pool_info.pool_identifier);
                println!("Type: {:?}", pool.pool_info.pool_type);
                println!("Total Shares: {}", pool.total_share);
                
                println!("\nAssets:");
                let mut asset_rows = Vec::new();
                for asset in pool.pool_info.assets {
                    asset_rows.push(vec![
                        asset.denom.clone(),
                        asset.amount.to_string(),
                        format_amount(asset.amount, &asset.denom, &config),
                    ]);
                }
                
                print_table(vec!["Denom", "Amount", "Formatted"], asset_rows);
                
                // Print pool fee info if available
                let pool_fees = pool.pool_info.pool_fees;
                println!("\nPool Fees:");
                println!("Swap Fee: {}%", pool_fees.swap_fee.to_string());
                println!("Protocol Fee: {}%", pool_fees.protocol_fee.to_string());
                println!("Burn Fee: {}%", pool_fees.burn_fee.to_string());
                for fee in pool_fees.extra_fees {
                    println!("Extra Fee: {}%", fee.to_string());
                }
                
                Ok(())
            },
        }
    }
}