//! TUI Demo Application
//!
//! This demo showcases the terminal management functionality of the MANTRA DEX SDK TUI.
//! It demonstrates proper terminal initialization, cleanup, and panic handling.

use mantra_dex_sdk::config::MantraNetworkConfig;
use mantra_dex_sdk::tui::{check_terminal_support, run_tui};
use mantra_dex_sdk::MantraDexClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("MANTRA DEX SDK TUI Demo");
    println!("======================");

    // Check terminal support before initializing TUI
    println!("Checking terminal support...");
    if let Err(e) = check_terminal_support() {
        eprintln!("Terminal not supported: {}", e);
        eprintln!("Please run this in a proper terminal with at least 80x24 characters.");
        std::process::exit(1);
    }
    println!("✓ Terminal support OK");

    // Initialize configuration
    let config = match env::var("MANTRA_NETWORK") {
        Ok(network) if network == "mainnet" => MantraNetworkConfig::mainnet(),
        _ => MantraNetworkConfig::testnet(),
    };

    println!("Network: {}", config.chain_id);
    println!("RPC: {}", config.rpc_endpoint);

    // Initialize DEX client
    let client = MantraDexClient::new(config.clone()).await?;

    println!("✓ DEX client initialized");
    println!("\nStarting TUI...");
    println!("Press any key to continue or Ctrl+C to exit");

    // Wait for user input
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Run the TUI
    match run_tui(client, config).await {
        Ok(()) => {
            println!("TUI exited normally");
        }
        Err(e) => {
            eprintln!("TUI error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
