# Mantra DEX SDK

A Rust SDK for interacting with the Mantra DEX on the Mantra Dukong Network.

## Overview

This SDK provides a comprehensive set of tools for developers to easily interact with the Mantra DEX, allowing for operations such as:

- Query pool information and liquidity
- Execute swaps and multi-hop swaps
- Provide and withdraw liquidity
- Manage wallets and sign transactions
- Query balances and account information

The SDK is designed to be used without requiring configuration files - all network parameters can be passed directly when initializing the client. This makes it easy to integrate into CLI applications, web services, or other tools without managing external configuration.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
mantra-dex-sdk = "0.1.0"
```

## Usage

### Creating a client

```rust
use mantra_dex_sdk::{MantraDexClient, MantraNetworkConfig};

async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// placeholder
    Ok(())
}
```

### Setting up a wallet

```rust
use mantra_dex_sdk::{MantraDexClient, MantraNetworkConfig, MantraWallet};

async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create or load wallet from mnemonic
    let wallet = MantraWallet::from_mnemonic("your mnemonic phrase here", 0)?;
    
    // Create client with wallet
    let config = MantraNetworkConfig::default_mainnet();
    let client = MantraDexClient::new(config).await?.with_wallet(wallet);
    
    // Get wallet address
    let address = client.wallet()?.address()?.to_string();
    println!("Wallet address: {}", address);
    
    Ok(())
}
```

### Query operations

```rust
use mantra_dex_sdk::{Coin, Uint128};

async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... initialize client
    
    // Query pools
    let pools = client.get_pools(Some(10)).await?;
    println!("First 10 pools: {:?}", pools);
    
    // Get specific pool
    let pool = client.get_pool("pool123").await?;
    println!("Pool info: {:?}", pool);
    
    // Query balances
    let balances = client.get_balances().await?;
    println!("Balances: {:?}", balances);
    
    // Simulate swap
    let simulation = client.simulate_swap(
        "pool123",
        Coin {
            denom: "uom".to_string(),
            amount: Uint128::from(1000000u128),
        },
        "uusdt"
    ).await?;
    println!("Expected return: {:?}", simulation.return_amount);
    
    Ok(())
}
```

### Execute operations

```rust
use mantra_dex_sdk::{Coin, Decimal, Uint128, SwapOperation};

async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... initialize client with wallet
    
    // Execute swap
    let result = client.swap(
        "pool123",
        Coin {
            denom: "uom".to_string(),
            amount: Uint128::from(1000000u128),
        },
        "uusdt",
        Some(Decimal::percent(1)), // 1% max spread
    ).await?;
    println!("Swap tx hash: {}", result.txhash);
    
    // Provide liquidity
    let assets = vec![
        Coin {
            denom: "uom".to_string(),
            amount: Uint128::from(1000000u128),
        },
        Coin {
            denom: "uusdc".to_string(),
            amount: Uint128::from(1000000u128),
        },
    ];
    
    let result = client.provide_liquidity(
        "pool123",
        assets,
        Some(Decimal::percent(1)), // 1% slippage tolerance
    ).await?;
    println!("Liquidity provision tx hash: {}", result.txhash);
    
    // Withdraw liquidity
    let result = client.withdraw_liquidity(
        "pool123",
        Uint128::from(500000u128), // LP token amount
    ).await?;
    println!("Liquidity withdrawal tx hash: {}", result.txhash);
    
    // Execute multi-hop swap
    let operations = vec![
        SwapOperation {
            pool_id: "o.uom.uusdc".to_string(), 
            denom_in: "uom".to_string(),
            denom_out: "uusdc".to_string(),
        },
        SwapOperation {
            pool_id: "pool456".to_string(),
            denom_in: "uusdc".to_string(),
            denom_out: "uatom".to_string(),
        },
    ];
    
    let result = client.execute_swap_operations(
        operations,
        Uint128::from(1000000u128),
    ).await?;
    println!("Multi-hop swap tx hash: {}", result.txhash);
    
    Ok(())
}
```

## API Reference

### MantraDexClient

The main client interface for interacting with the Mantra DEX:

- `new(config: MantraNetworkConfig) -> Result<Self, Error>` - Create a new client
- `with_wallet(wallet: MantraWallet) -> Self` - Add a wallet to the client
- `get_pools(limit: Option<u32>) -> Result<Vec<PoolInfoResponse>, Error>` - Query available pools
- `get_pool(pool_id: &str) -> Result<PoolInfoResponse, Error>` - Get info for a specific pool
- `simulate_swap(pool_id: &str, offer_asset: Coin, ask_asset_denom: &str) -> Result<SimulationResponse, Error>` - Simulate a swap
- `swap(pool_id: &str, offer_asset: Coin, ask_asset_denom: &str, max_spread: Option<Decimal>) -> Result<TxResponse, Error>` - Execute a swap
- `provide_liquidity(pool_id: &str, assets: Vec<Coin>, slippage_tolerance: Option<Decimal>) -> Result<TxResponse, Error>` - Provide liquidity
- `withdraw_liquidity(pool_id: &str, lp_amount: Uint128) -> Result<TxResponse, Error>` - Withdraw liquidity
- `execute_swap_operations(operations: Vec<SwapOperation>, amount: Uint128) -> Result<TxResponse, Error>` - Execute multi-hop swaps

### MantraWallet

Wallet management for Mantra DEX:

- `from_mnemonic(mnemonic: &str, account_index: u32) -> Result<Self, Error>` - Create wallet from mnemonic
- `address() -> Result<cosmrs::AccountId, Error>` - Get wallet address
- `sign_tx(...)` - Sign a transaction

## License

Licensed under the MIT License. 