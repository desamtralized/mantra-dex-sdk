mod commands;
mod config;
mod error;
mod utils;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use config::CliConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::commands::{
    balance::BalanceCommand,
    liquidity::{ProvideLiquidityCommand, WithdrawLiquidityCommand},
    pool::PoolCommand,
    swap::SwapCommand,
    wallet::WalletCommand,
};
use crate::error::CliError;
use crate::utils::ensure_session_password;

#[derive(Parser)]
#[command(name = "mantra-dex")]
#[command(about = "Mantra DEX CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config file
    #[arg(short, long, global = true)]
    config: Option<String>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Wallet management commands
    Wallet(WalletCommand),

    /// Pool operations and queries
    Pool(PoolCommand),

    /// Swap tokens
    Swap(SwapCommand),

    /// Liquidity operations
    #[command(subcommand)]
    Liquidity(LiquidityCommands),

    /// Check balances
    Balance(BalanceCommand),
}

#[derive(Subcommand, Clone)]
enum LiquidityCommands {
    /// Provide liquidity to a pool
    Provide(ProvideLiquidityCommand),

    /// Withdraw liquidity from a pool
    Withdraw(WithdrawLiquidityCommand),
}

#[tokio::main]
async fn main() -> Result<(), CliError> {
    // Setup logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize configuration from default path or CLI argument
    let config_path = if let Some(args) = std::env::args().nth(1) {
        if args == "--config" || args == "-c" {
            if let Some(path) = std::env::args().nth(2) {
                std::path::PathBuf::from(path)
            } else {
                CliConfig::default_path()
            }
        } else {
            CliConfig::default_path()
        }
    } else {
        CliConfig::default_path()
    };

    // Load or create config
    let mut config = CliConfig::load_or_create(&config_path)?;
    
    // Check if interactive mode is enabled
    let interactive = !std::env::args().any(|arg| arg == "--no-interactive");
    
    if interactive && std::env::args().len() == 1 {
        // Interactive mode - keep CLI running until exit command
        println!("Mantra DEX CLI - Interactive Mode (wallet session maintained)");
        println!("Type 'exit' or 'quit' to exit, 'help' for available commands");
        
        use rustyline::Editor;
        use rustyline::history::DefaultHistory;
        let mut rl = Editor::<(), DefaultHistory>::new().expect("Failed to create line editor");

        loop {
            let readline = rl.readline("mantra-dex> ");
            match readline {
                Ok(line) => {
                    let result = rl.add_history_entry(line.as_str());
                    if let Err(e) = result {
                        eprintln!("Error adding history entry: {}", e);
                    }
                    
                    let input = line.trim();
                    if input.is_empty() {
                        continue;
                    }
                    
                    if input == "exit" || input == "quit" {
                        break;
                    }
                    
                    if input == "help" {
                        print_help();
                        continue;
                    }
                    
                    // Split input into args
                    let args: Vec<String> = shell_words::split(input)
                        .unwrap_or_else(|e| {
                            eprintln!("Error parsing command: {}", e);
                            vec![]
                        });
                    
                    if args.is_empty() {
                        continue;
                    }
                    
                    // Process the command, preserving the session password
                    let result = process_command(args, &mut config, &config_path).await;
                    if let Err(e) = result {
                        eprintln!("Error: {}", e);
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }
        
        println!("Goodbye!");
        Ok(())
    } else {
        // Standard single-command mode
        let cli = Cli::parse();
        
        // Update config with any CLI overrides
        if let Some(path) = cli.config {
            // Load the new config but preserve any session password
            let session_password = config.get_session_password().map(String::from);
            config = CliConfig::load_or_create(&std::path::PathBuf::from(path))?;
            
            // Restore the session password
            if let Some(password) = session_password {
                config.store_session_password(&password);
            }
        }

        // Check if we need to ensure a password is available
        let is_wallet_command = matches!(cli.command, Commands::Wallet(_));
        
        // Check if this is a read-only command that doesn't need wallet access
        let is_read_only = match &cli.command {
            Commands::Pool(cmd) => matches!(cmd.command, commands::pool::PoolCommands::List { .. } | commands::pool::PoolCommands::Info { .. }),
            _ => false,
        };
        
        if !is_wallet_command && !is_read_only {
            ensure_session_password(&mut config)?;
        }

        // Execute the command
        match cli.command {
            Commands::Wallet(cmd) => cmd.clone().execute(config).await,
            Commands::Pool(cmd) => {
                // Clone config but preserve session password
                let mut cmd_config = config.clone();
                if let Some(pwd) = config.get_session_password() {
                    cmd_config.store_session_password(pwd);
                }
                cmd.clone().execute(cmd_config).await
            },
            Commands::Swap(cmd) => {
                // Clone config but preserve session password
                let mut cmd_config = config.clone();
                if let Some(pwd) = config.get_session_password() {
                    cmd_config.store_session_password(pwd);
                }
                cmd.clone().execute(cmd_config).await
            },
            Commands::Liquidity(cmd) => match cmd {
                LiquidityCommands::Provide(provide_cmd) => {
                    // Clone config but preserve session password
                    let mut cmd_config = config.clone();
                    if let Some(pwd) = config.get_session_password() {
                        cmd_config.store_session_password(pwd);
                    }
                    provide_cmd.clone().execute(cmd_config).await
                },
                LiquidityCommands::Withdraw(withdraw_cmd) => {
                    // Clone config but preserve session password
                    let mut cmd_config = config.clone();
                    if let Some(pwd) = config.get_session_password() {
                        cmd_config.store_session_password(pwd);
                    }
                    withdraw_cmd.clone().execute(cmd_config).await
                },
            },
            Commands::Balance(cmd) => {
                // Clone config but preserve session password
                let mut cmd_config = config.clone();
                if let Some(pwd) = config.get_session_password() {
                    cmd_config.store_session_password(pwd);
                }
                cmd.clone().execute(cmd_config).await
            },
        }
    }
}

/// Process a command in the interactive shell
async fn process_command(
    args: Vec<String>, 
    config: &mut CliConfig, 
    config_path: &std::path::Path
) -> Result<(), CliError> {
    // Create a custom clap command for interactive mode
    let command = Cli::command();
    let matches = command.try_get_matches_from(
        std::iter::once(String::from("mantra-dex")).chain(args)
    );
   
    match matches {
        Ok(matches) => {
            let cli = Cli::from_arg_matches(&matches)
                .map_err(|e| CliError::Command(format!("Failed to parse command: {}", e)))?;
            
            // Check command type before executing
            let is_wallet_command = matches!(cli.command, Commands::Wallet(_));
            
            // Check if this is a read-only command that doesn't need wallet access
            let is_read_only = match &cli.command {
                Commands::Pool(cmd) => matches!(cmd.command, commands::pool::PoolCommands::List { .. } | commands::pool::PoolCommands::Info { .. }),
                _ => false,
            };
            
            // If it's not a wallet command or read-only command, make sure we have a password
            if !is_wallet_command && !is_read_only {
                ensure_session_password(config)?;
            }
            
            // Execute the command with current config
            let result = match cli.command {
                Commands::Wallet(cmd) => {
                    // Clone config but preserve session password
                    let mut cmd_config = config.clone();
                    if let Some(pwd) = config.get_session_password() {
                        cmd_config.store_session_password(pwd);
                    }
                    
                    let result = cmd.execute(cmd_config).await;
                    
                    // If the command succeeded, reload config and check for password capture
                    if result.is_ok() {
                        if let Ok(updated_config) = CliConfig::load_or_create(&config_path.to_path_buf()) {
                            // Keep the session password when updating the config
                            let session_password = config.get_session_password().map(String::from);
                            *config = updated_config;
                            
                            // Restore the session password
                            if let Some(password) = session_password {
                                config.store_session_password(&password);
                            }
                        }
                    }
                    
                    result
                },
                Commands::Pool(cmd) => {
                    // Clone config but preserve session password
                    let mut cmd_config = config.clone();
                    if let Some(pwd) = config.get_session_password() {
                        cmd_config.store_session_password(pwd);
                    }
                    cmd.execute(cmd_config).await
                },
                Commands::Swap(cmd) => {
                    // Clone config but preserve session password
                    let mut cmd_config = config.clone();
                    if let Some(pwd) = config.get_session_password() {
                        cmd_config.store_session_password(pwd);
                    }
                    cmd.execute(cmd_config).await
                },
                Commands::Liquidity(cmd) => match cmd {
                    LiquidityCommands::Provide(provide_cmd) => {
                        // Clone config but preserve session password
                        let mut cmd_config = config.clone();
                        if let Some(pwd) = config.get_session_password() {
                            cmd_config.store_session_password(pwd);
                        }
                        provide_cmd.execute(cmd_config).await
                    },
                    LiquidityCommands::Withdraw(withdraw_cmd) => {
                        // Clone config but preserve session password
                        let mut cmd_config = config.clone();
                        if let Some(pwd) = config.get_session_password() {
                            cmd_config.store_session_password(pwd);
                        }
                        withdraw_cmd.execute(cmd_config).await
                    },
                },
                Commands::Balance(cmd) => {
                    // Clone config but preserve session password
                    let mut cmd_config = config.clone();
                    if let Some(pwd) = config.get_session_password() {
                        cmd_config.store_session_password(pwd);
                    }
                    cmd.clone().execute(cmd_config).await
                },
            };
            
            // If command succeeded, reload the config to get any updates
            let config_path_buf = config_path.to_path_buf();
            if result.is_ok() && !is_wallet_command {
                if let Ok(updated_config) = CliConfig::load_or_create(&config_path_buf) {
                    // Keep the session password when updating the config
                    let session_password = config.get_session_password().map(String::from);
                    *config = updated_config;
                    
                    // Restore the session password
                    if let Some(password) = session_password {
                        config.store_session_password(&password);
                    }
                }
            }
            
            result
        }
        Err(e) => {
            Err(CliError::Command(format!("Invalid command: {}", e)))
        }
    }
}

/// Print help message for interactive mode
fn print_help() {
    println!("Available commands:");
    println!("  wallet create -n <name>      Create a new wallet");
    println!("  wallet import -n <name>      Import a wallet from mnemonic");
    println!("  wallet list                  List all wallets");
    println!("  wallet info                  Show current wallet info");
    println!("  wallet use <name>            Set active wallet");
    println!("  wallet export                Export wallet mnemonic");
    println!("  wallet remove <name>         Remove a wallet");
    println!("  pool create                  Create a new pool");
    println!("  pool list                    List all pools");
    println!("  pool info                    Show pool info");
    println!("  swap                         Swap tokens");
    println!("  liquidity provide            Provide liquidity to a pool");
    println!("  liquidity withdraw           Withdraw liquidity from a pool");
    println!("  balance                      Check token balances");
    println!("  help                         Show this help message");
    println!("  exit, quit                   Exit the CLI");
} 