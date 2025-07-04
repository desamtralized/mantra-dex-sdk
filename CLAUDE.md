# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the Mantra DEX SDK - a comprehensive Rust SDK for interacting with the Mantra blockchain DEX. The project provides both a library SDK and includes MCP (Model Context Protocol) server and TUI (Terminal User Interface) features through optional feature flags.

## Build Commands

### Core SDK (Default)
```bash
cargo build              # Build SDK library only
cargo test               # Run SDK tests
```

### MCP Server Development
```bash
# Compilation validation (preferred for development)
cargo check --features mcp
cargo build --features mcp
cargo test --features mcp

# Running MCP server (use with caution - can hang)
cargo run --bin mcp-server --features mcp
```

### TUI Development
```bash
# Primary TUI entry point
cargo run --bin mantra-dex-tui --features tui

# Alternative TUI entry point  
cargo run --bin tui --features tui
```

### Testing Strategy
- **SDK Core**: Full test coverage for all business logic
- **TUI Components**: Manual testing only - no automated tests
- **MCP Server**: Integration tests for MCP protocol compliance

## Architecture

### Core SDK Structure
- **`src/client.rs`** (1183 lines): Main DEX client with all blockchain operations
- **`src/config.rs`**: Network configuration and constants management
- **`src/wallet.rs`**: HD wallet operations and key management
- **`src/error.rs`**: Centralized error types and handling
- **`src/lib.rs`**: Module exports and feature-gated re-exports

### Feature-Gated Modules
- **`src/mcp/`**: MCP server implementation (requires `mcp` feature)
- **`src/tui/`**: Terminal User Interface (requires `tui` feature)

### MCP Server Components
- **`src/mcp/server.rs`**: Core MCP server with JSON-RPC 2.0 support
- **`src/mcp/sdk_adapter.rs`**: Adapter layer between MCP and SDK
- **`src/mcp/client_wrapper.rs`**: MCP client wrapper functionality

### TUI Architecture
- **`src/tui/app.rs`** (2680 lines): Central application state management
- **`src/tui/events.rs`** (929 lines): Event handling and async operations
- **`src/tui/screens/`**: Individual screen implementations
- **`src/tui/components/`**: Reusable UI components

## Development Workflow

### Adding New SDK Features
1. Implement core functionality in `src/client.rs`
2. Add error types to `src/error.rs`
3. Update configuration in `src/config.rs` if needed
4. Add comprehensive tests for business logic
5. Update `src/lib.rs` exports

### MCP Server Development
- Use `cargo check --features mcp` for compilation validation
- Never use `cargo run --bin mcp-server` for compilation checks
- MCP server provides 28 tools and 3 resources for DEX operations
- Full JSON-RPC 2.0 compliance with async operations

### TUI Development
- Focus on state management in `src/tui/app.rs`
- Implement event handling in `src/tui/events.rs`
- Use unified focus management across all screens
- Manual testing only - no automated TUI tests

## Key Dependencies

### Core SDK
- `mantra-dex-std`: DEX standard library
- `cosmrs`: Cosmos SDK integration with RPC, BIP32, CosmWasm
- `tokio`: Async runtime

### MCP Server (Feature-Gated)
- `rust-mcp-sdk`: MCP server implementation
- `rust-mcp-schema`: MCP schema definitions
- `axum`, `hyper`: HTTP server support

### TUI (Feature-Gated)
- `ratatui`: Terminal UI framework
- `crossterm`: Cross-platform terminal control
- `tui-input`: Text input handling

## Network Configuration

The SDK supports multiple networks through configuration:
- Uses `chain_id` for network identification (migrated from `network_id`)
- Supports testnet/mainnet with configurable RPC endpoints
- Proper bech32 address validation for Cosmos addresses

## Testing Guidelines

### What to Test
- All SDK business logic and client operations
- Wallet functionality and key management
- Network configuration and validation
- Error handling and edge cases
- MCP protocol compliance

### What NOT to Test
- TUI components, screens, or navigation
- UI rendering or visual components
- Event handling in TUI context
- Manual testing preferred for TUI functionality

## Security Considerations

- Private keys never exposed in responses
- Wallet encryption when saved to disk
- BIP32/BIP39 key derivation standards
- Proper transaction parameter validation
- Slippage protection for trading operations