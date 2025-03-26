use clap::Args;
use mantra_dex_sdk::{Coin, Decimal, Uint128};
use std::str::FromStr;
use dialoguer::Confirm;

use crate::config::CliConfig;
use crate::error::CliError;
use crate::utils::{create_client, parse_coin, parse_decimal, print_success};

#[derive(Args, Clone)]
pub struct ProvideLiquidityCommand {
    /// Pool ID to provide liquidity to
    #[arg(short, long)]
    pool_id: String,
    
    /// Assets to provide in format "amount1:denom1,amount2:denom2" (e.g., "1000:uom,5000:uusdt")
    #[arg(short, long)]
    assets: String,
    
    /// Slippage tolerance (e.g., 0.01 for 1%)
    #[arg(short, long)]
    slippage: Option<String>,
    
    /// Skip confirmation prompt
    #[arg(short, long)]
    yes: bool,
}

impl ProvideLiquidityCommand {
    pub async fn execute(self, config: CliConfig) -> Result<(), CliError> {
        let client = create_client(&config).await?;
        
        // Parse assets
        let assets: Vec<Coin> = self.assets.split(',')
            .map(|s| parse_coin(s).map_err(|e| CliError::Parse(format!("Invalid asset: {}", e))))
            .collect::<Result<_, _>>()?;
            
        if assets.is_empty() {
            return Err(CliError::Command("No assets specified".to_string()));
        }
        
        // Parse slippage tolerance
        let slippage = if let Some(slippage_str) = &self.slippage {
            parse_decimal(slippage_str)
                .map_err(|e| CliError::Parse(format!("Invalid slippage value: {}", e)))?
        } else {
            Decimal::percent(1) // Default to 1%
        };
            
        // Confirm the operation
        if !self.yes {
            println!("\nYou are about to provide liquidity to pool {}:", self.pool_id);
            for asset in &assets {
                println!("- {}", asset);
            }
            println!("Slippage tolerance: {}", slippage);
            
            if !Confirm::new()
                .with_prompt("Do you want to proceed?")
                .default(false)
                .interact()
                .unwrap_or(false)
            {
                println!("Operation cancelled.");
                return Ok(());
            }
        }
        
        // Execute the operation
        let result = client
            .provide_liquidity(&self.pool_id, assets, Some(slippage))
            .await
            .map_err(CliError::Sdk)?;
            
        print_success(&format!(
            "Successfully provided liquidity to pool {}. Transaction hash: {}",
            self.pool_id, result.txhash
        ));
        
        Ok(())
    }
}

#[derive(Args, Clone)]
pub struct WithdrawLiquidityCommand {
    /// Pool ID to withdraw liquidity from
    #[arg(short, long)]
    pool_id: String,
    
    /// Amount of LP tokens to withdraw
    #[arg(short, long)]
    amount: String,
    
    /// Skip confirmation prompt
    #[arg(short, long)]
    yes: bool,
}

impl WithdrawLiquidityCommand {
    pub async fn execute(self, config: CliConfig) -> Result<(), CliError> {
        let client = create_client(&config).await?;
        
        // Parse amount
        let amount = Uint128::from_str(&self.amount)
            .map_err(|_| CliError::Parse(format!("Invalid amount: {}", self.amount)))?;
            
        // Confirm the operation
        if !self.yes {
            println!("\nYou are about to withdraw {} shares from pool {}.", amount, self.pool_id);
            
            if !Confirm::new()
                .with_prompt("Do you want to proceed?")
                .default(false)
                .interact()
                .unwrap_or(false)
            {
                println!("Operation cancelled.");
                return Ok(());
            }
        }
        
        // Execute withdrawal
        let result = client
            .withdraw_liquidity(&self.pool_id, amount)
            .await
            .map_err(CliError::Sdk)?;

        println!("Transaction hash: {}", result.txhash);
        print_success(&format!("Successfully withdrawn liquidity from pool {}", self.pool_id));
        
        println!("\nWithdrawn assets:");
        let balances = client.get_balances().await?;
        for asset in balances {
            println!("- {}", asset);
        }
        
        Ok(())
    }
} 