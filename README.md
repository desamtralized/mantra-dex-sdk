# MANTRA Dex SDK

A comprehensive Rust SDK for interacting with the MANTRA blockchain DEX, featuring wallet management, trading operations, liquidity management, and optional MCP server and TUI interfaces.

## Features

- **Complete DEX Operations**: Swap execution, liquidity provision/withdrawal, pool management
- **HD Wallet Management**: BIP32/BIP39 compatible wallet generation and import
- **Multi-Network Support**: Configurable testnet/mainnet connectivity
- **Real-time Analytics**: Trading reports, performance analysis, impermanent loss calculations
- **MCP Server Integration**: Model Context Protocol server for AI agent interaction
- **Terminal UI**: Interactive command-line interface for manual operations

## Architecture

### Core SDK (`src/`)

```
src/
├── client.rs          # Main DEX client (1183 lines) - all blockchain operations
├── config.rs          # Network configuration and constants management  
├── wallet.rs          # HD wallet operations and key management
├── error.rs           # Centralized error types and handling
└── lib.rs             # Module exports and feature-gated re-exports
```

### Feature-Gated Modules

#### MCP Server (`--features mcp`)
```
src/mcp/
├── server.rs          # Core MCP server with JSON-RPC 2.0 support
├── sdk_adapter.rs     # Adapter layer between MCP and SDK
└── client_wrapper.rs  # MCP client wrapper functionality
```

#### Terminal UI (`--features tui`)
```
src/tui/
├── app.rs             # Central application state management (2680 lines)
├── events.rs          # Event handling and async operations (929 lines)
├── screens/           # Individual screen implementations
└── components/        # Reusable UI components
```

## Quick Start

### Installation

```bash
git clone <repository-url>
cd mantra-dex-sdk
cargo build --release
```

### Basic SDK Usage

```rust
use mantra_dex_sdk::{Client, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client
    let config = Config::testnet();
    let client = Client::new(config).await?;
    
    // Get pool information
    let pools = client.get_pools(None, None).await?;
    println!("Available pools: {}", pools.len());
    
    // Check wallet balance
    if let Some(balance) = client.get_balances(None, false).await? {
        println!("Wallet balance: {:?}", balance);
    }
    
    Ok(())
}
```

## Build Targets

### SDK Library (Default)
```bash
cargo build              # Build SDK library only
cargo test               # Run SDK tests
```

### MCP Server
```bash
cargo check --features mcp                    # Compilation validation
cargo build --features mcp                    # Build MCP server
cargo run --bin mcp-server --features mcp     # Run MCP server
```

### Terminal UI
```bash
cargo run --bin mantra-dex-tui --features tui  # Primary TUI entry point
cargo run --bin tui --features tui             # Alternative TUI entry point
```

## Core Modules

### Client (`src/client.rs`)
The main interface for all DEX operations:
- **Pool Operations**: Query pools, create pools (admin), manage pool features
- **Trading**: Execute swaps, simulate trades, monitor transactions  
- **Liquidity Management**: Provide/withdraw liquidity, manage LP tokens
- **Wallet Integration**: Balance queries, transaction signing
- **Analytics**: Generate trading reports, calculate impermanent loss

```rust
// Example: Execute a swap
let swap_result = client.execute_swap(
    "1",                    // pool_id
    ("uom", "1000000"),     // offer_asset (denom, amount)
    "uusdy",                // ask_asset_denom
    Some("0.05")            // max_slippage (5%)
).await?;
```

### Wallet (`src/wallet.rs`)
HD wallet functionality with secure key management:
- **Generation**: Create new wallets with mnemonic phrases
- **Import**: Import existing wallets from mnemonic
- **Key Derivation**: BIP32/BIP39 compliant key derivation
- **Security**: Encrypted storage, private key protection

```rust
// Example: Generate new wallet
let wallet = client.generate_wallet(0, true, Some("my-wallet")).await?;
println!("Address: {}", wallet.address);
```

### Configuration (`src/config.rs`)
Network and runtime configuration:
- **Multi-Network**: Testnet/mainnet support with chain_id migration
- **Endpoints**: Configurable RPC, LCD, and gRPC endpoints
- **Validation**: Bech32 address validation for Cosmos addresses

```rust
// Example: Custom network configuration
let config = Config {
    chain_id: "mantra-dukong".to_string(),
    rpc_endpoint: "https://rpc.testnet.mantra.com".to_string(),
    lcd_endpoint: "https://api.testnet.mantra.com".to_string(),
    grpc_endpoint: "https://grpc.testnet.mantra.com".to_string(),
    ..Config::default()
};
```

## Key Dependencies

### Core SDK
- **`mantra-dex-std`**: DEX standard library and types
- **`cosmrs`**: Cosmos SDK integration with RPC, BIP32, CosmWasm support
- **`tokio`**: Async runtime for concurrent operations

### MCP Server (Optional)
- **`rust-mcp-sdk`**: MCP server implementation
- **`rust-mcp-schema`**: MCP schema definitions
- **`axum`, `hyper`**: HTTP server infrastructure

### Terminal UI (Optional)
- **`ratatui`**: Modern terminal UI framework
- **`crossterm`**: Cross-platform terminal control
- **`tui-input`**: Advanced text input handling

## Development Workflow

### Adding New SDK Features
1. Implement core functionality in `src/client.rs`
2. Add error types to `src/error.rs`
3. Update configuration in `src/config.rs` if needed
4. Add comprehensive unit tests
5. Update `src/lib.rs` exports

### Testing Strategy
- **SDK Core**: Full test coverage for business logic
- **MCP Server**: Integration tests for protocol compliance
- **TUI**: Manual testing only (no automated UI tests)

```bash
cargo test                           # Run all SDK tests
cargo test --features mcp          # Test MCP functionality  
cargo test wallet_operations       # Test specific modules
```

## Environment Configuration

```bash
# Network settings
export MANTRA_NETWORK=testnet
export MANTRA_RPC_ENDPOINT=https://rpc.testnet.mantra.com
export MANTRA_LCD_ENDPOINT=https://api.testnet.mantra.com

# Development settings  
export RUST_LOG=debug
export MCP_SERVER_DEBUG=true
```

## Security Best Practices

- **Private Keys**: Never exposed in responses, encrypted storage
- **Validation**: All transaction parameters validated before execution
- **Slippage Protection**: Configurable slippage limits for trades
- **Address Verification**: Proper bech32 address validation

## Contributing

1. Fork and clone the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes following existing code conventions
4. Add tests for new functionality
5. Ensure all tests pass (`cargo test`)
6. Submit a pull request

### Code Conventions
- Follow existing patterns in `src/client.rs` for new operations
- Add proper error handling with descriptive error types
- Use existing libraries and utilities where possible
- Maintain comprehensive test coverage for business logic

## License

MIT License - see [LICENSE](LICENSE) file for details.

---

Built for the MANTRA ecosystem 🚀