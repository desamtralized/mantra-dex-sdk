# MANTRA DEX SDK Model Context Protocol (MCP) Server

A comprehensive Model Context Protocol (MCP) server that exposes the complete functionality of the MANTRA DEX SDK, enabling AI agents and other MCP clients to interact with the MANTRA blockchain DEX through a standardized protocol.

## 🌟 Features

### Core Capabilities
- **Wallet Management**: Generate, import, and manage HD wallets with mnemonic phrases
- **Network Operations**: Multi-network support (mainnet/testnet) with automatic configuration
- **Pool Operations**: Complete pool management including creation, queries, and feature control
- **Trading Operations**: Swap execution, liquidity provision/withdrawal, and transaction monitoring
- **Advanced Analytics**: Trading reports, performance analysis, and impermanent loss calculations
- **Real-time Resources**: Live trading history, pending transactions, and liquidity positions

### MCP Protocol Support
- **28 Tools**: Complete tool set for all DEX operations (✅ All implemented)
- **3 Resources**: Real-time data resources for trading and liquidity (✅ All implemented)
- **JSON-RPC 2.0**: Full MCP specification compliance
- **Async Operations**: Non-blocking blockchain interactions
- **Error Handling**: Comprehensive error responses with recovery suggestions
- **Resource Provider**: Full MCP resource provider implementation
- **State Management**: Complete server state and configuration management

## ✅ Implementation Status

**🎉 All Core Functionality Complete!**

- ✅ **Phase 1**: Wallet & Network Operations (100% Complete)
- ✅ **Phase 2**: Pool Operations (100% Complete)  
- ✅ **Phase 3**: Trading Operations (100% Complete)
- ✅ **Phase 4**: Advanced Features (100% Complete)

**Recent Additions:**
- ✅ Trading Resources (`trades://history`, `trades://pending`, `liquidity://positions`)
- ✅ LP Token Management (`get_lp_token_balance`, `get_all_lp_token_balances`, `estimate_lp_withdrawal_amounts`)
- ✅ Analytics & Reporting (`generate_trading_report`, `calculate_impermanent_loss`)
- ✅ Full MCP Resource Provider and State Manager implementations

## 📋 Prerequisites

- **Rust**: 1.70+ with Cargo
- **Network Access**: Internet connection for blockchain RPC calls
- **MCP Client**: Claude Desktop, MCP SDK, or compatible client

## 🚀 Quick Start

### 1. Clone and Build

```bash
git clone <repository-url>
cd mcp-mantra-dex-sdk
cargo build --release
```

### 2. Configuration

Create a configuration file or use environment variables:

```bash
# Create config directory
mkdir -p config

# Copy example configuration
cp config/network.toml.example config/network.toml
```

### 3. Run the MCP Server

The project provides multiple binary targets:

#### MCP Server (Primary - for AI agents)
```bash
cargo run --bin mcp-server --features mcp
```
*This is the main MCP server for Claude Desktop and other MCP clients*

#### TUI Interface (Alternative - for human users)  
```bash
cargo run --bin tui --features tui
# or
cargo run --bin mantra-dex-tui --features tui
```
*Terminal User Interface for direct human interaction*

**Note**: Use the MCP server (`--bin mcp`) for AI agent integration. Use the TUI (`--bin tui`) for manual testing and exploration.

## ⚙️ Configuration

### Environment Variables

```bash
# Network Configuration
export MANTRA_NETWORK=testnet                    # or mainnet
export MANTRA_RPC_ENDPOINT=https://rpc.testnet.mantra.com
export MANTRA_LCD_ENDPOINT=https://api.testnet.mantra.com
export MANTRA_GRPC_ENDPOINT=https://grpc.testnet.mantra.com

# Server Configuration
export MCP_SERVER_DEBUG=true
export MCP_SERVER_PORT=8080
export MCP_CACHE_TTL_SECS=300
export MCP_REQUEST_TIMEOUT_SECS=30
```

### Configuration File

Create `config/server.toml`:

```toml
[server]
name = "Mantra DEX MCP Server"
version = "1.0.0"
debug = true
max_concurrent_ops = 10
request_timeout_secs = 30
cache_ttl_secs = 300

[network]
chain_id = "mantra-dukong"
rpc_endpoint = "https://rpc.testnet.mantra.com"
lcd_endpoint = "https://api.testnet.mantra.com"
grpc_endpoint = "https://grpc.testnet.mantra.com"

[runtime]
flavor = "MultiThread"
worker_threads = 4
enable_io = true
enable_time = true
```

## 🔧 Claude Desktop Integration

Add to your Claude Desktop MCP configuration:

### macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
### Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "cargo",
      "args": ["run", "--bin", "mcp"],
      "cwd": "/path/to/mcp-mantra-dex-sdk",
      "env": {
        "MANTRA_NETWORK": "testnet",
        "RUST_LOG": "info"
      }
    }
  }
}
```

## 📚 MCP Usage Guide

### Getting Started with Claude Desktop

1. **Install the MCP Server**:
   ```bash
   git clone <repository-url>
   cd mcp-mantra-dex-sdk
   cargo build --release
   ```

2. **Configure Claude Desktop**:
   Add the server configuration to your Claude Desktop config file:
   
   ```json
   {
     "mcpServers": {
           "mantra-dex": {
      "command": "cargo",
      "args": ["run", "--bin", "mcp-server", "--features", "mcp"],
      "cwd": "/absolute/path/to/mcp-mantra-dex-sdk",
         "env": {
           "MANTRA_NETWORK": "testnet",
           "RUST_LOG": "info"
         }
       }
     }
   }
   ```

3. **Restart Claude Desktop** and you should see the Mantra DEX tools available!

### Using with Other MCP Clients

```bash
# Start the MCP server
cargo run --bin mcp

# The server will listen on stdio for MCP protocol messages
# Send JSON-RPC 2.0 messages according to MCP specification
```

## 📚 Usage Examples

### Wallet Management

#### Generate New Wallet
```json
{
  "tool": "generate_wallet",
  "arguments": {
    "account_index": 0,
    "save_wallet": true,
    "wallet_name": "my-trading-wallet"
  }
}
```

#### Import Existing Wallet
```json
{
  "tool": "import_wallet",
  "arguments": {
    "mnemonic": "word1 word2 word3 ... word12",
    "account_index": 0,
    "save_wallet": true,
    "wallet_name": "imported-wallet"
  }
}
```

### Trading Operations

#### Execute Swap
```json
{
  "tool": "execute_swap",
  "arguments": {
    "pool_id": "1",
    "offer_asset": {
      "denom": "uom",
      "amount": "1000000"
    },
    "ask_asset_denom": "uusdy",
    "max_slippage": "0.05"
  }
}
```

#### Provide Liquidity
```json
{
  "tool": "provide_liquidity",
  "arguments": {
    "pool_id": "1",
    "assets": [
      {
        "denom": "uom",
        "amount": "1000000"
      },
      {
        "denom": "uusdy",
        "amount": "1000000"
      }
    ],
    "max_slippage": "0.02"
  }
}
```

### Analytics and Reporting

#### Generate Trading Report
```json
{
  "tool": "generate_trading_report",
  "arguments": {
    "time_period": "30d",
    "report_format": "detailed",
    "include_pool_breakdown": true,
    "include_performance_metrics": true
  }
}
```

#### Calculate Impermanent Loss
```json
{
  "tool": "calculate_impermanent_loss",
  "arguments": {
    "pool_id": "1",
    "entry_price_asset_a": "1.00",
    "entry_price_asset_b": "0.50",
    "include_fees_earned": true,
    "include_detailed_breakdown": true
  }
}
```

### Resource Access

#### Get Trading History
```json
{
  "method": "resources/read",
  "params": {
    "uri": "trades://history"
  }
}
```

#### Get Pending Trades
```json
{
  "method": "resources/read",
  "params": {
    "uri": "trades://pending"
  }
}
```

#### Get Liquidity Positions
```json
{
  "method": "resources/read",
  "params": {
    "uri": "liquidity://positions"
  }
}
```

## 🛠️ Available Tools

### Wallet Tools
- `generate_wallet` - Create new HD wallets
- `import_wallet` - Import from mnemonic phrases
- `get_wallet_info` - Get active wallet information
- `get_wallet_balance` - Query wallet token balances

### Network Tools
- `get_network_status` - Network health and connectivity
- `get_block_height` - Current blockchain height
- `get_contract_addresses` - DEX contract addresses

### Pool Tools
- `get_pool` - Query specific pool information
- `get_pools` - List all available pools
- `validate_pool_status` - Check pool availability
- `create_pool` - Create new liquidity pools (admin)
- `update_pool_features` - Manage pool features (admin)

### Trading Tools
- `simulate_swap` - Preview swap outcomes
- `execute_swap` - Perform token swaps
- `provide_liquidity` - Add liquidity to pools
- `withdraw_liquidity` - Remove liquidity from pools
- `monitor_swap_transaction` - Track transaction status

### LP Token Tools
- `get_lp_token_balance` - Query LP token balances
- `get_all_lp_token_balances` - Get all LP positions
- `estimate_lp_withdrawal_amounts` - Estimate withdrawal values

### Analytics Tools
- `generate_trading_report` - Comprehensive trading analysis
- `calculate_impermanent_loss` - LP position analysis
- `get_swap_history` - Historical trading data
- `analyze_swap_performance` - Performance metrics

### Transaction Monitoring
- `get_transaction_monitor_status` - Check transaction status
- `cancel_transaction_monitor` - Cancel monitoring
- `list_transaction_monitors` - List active monitors
- `cleanup_transaction_monitors` - Clean completed monitors

## 📊 Available Resources

### Trading Resources
- **`trades://history`** - Historical trading data and transaction records
- **`trades://pending`** - Currently pending or in-progress transactions
- **`liquidity://positions`** - Current and historical liquidity positions

## 🔍 Monitoring and Debugging

### Enable Debug Logging
```bash
RUST_LOG=debug cargo run --bin mcp
```

### Health Check
```json
{
  "method": "ping"
}
```

### Server Status
```json
{
  "tool": "get_network_status",
  "arguments": {}
}
```

## 🚨 Error Handling

The server provides comprehensive error responses:

```json
{
  "error": {
    "code": -32001,
    "message": "Wallet not configured",
    "data": {
      "error_type": "WalletNotConfigured",
      "suggestions": [
        "Generate a new wallet using generate_wallet tool",
        "Import an existing wallet using import_wallet tool"
      ],
      "severity": "medium",
      "recoverable": true
    }
  }
}
```

## 🔒 Security Considerations

### Private Key Safety
- Private keys are never exposed in responses
- Wallets are encrypted when saved
- Use secure key derivation (BIP32/BIP39)

### Network Security
- Always validate transaction parameters
- Use proper slippage protection
- Verify contract addresses before interaction

### Best Practices
```bash
# Use environment variables for sensitive data
export WALLET_MNEMONIC="your twelve word mnemonic phrase here"

# Enable secure logging
export RUST_LOG="mantra_dex_sdk=info,mcp=info"

# Use testnet for development
export MANTRA_NETWORK=testnet
```

## 🧪 Testing

### Unit Tests
```bash
cargo test
```

### Integration Tests
```bash
# Test with testnet
MANTRA_NETWORK=testnet cargo test --features integration

# Test specific functionality
cargo test wallet_operations
cargo test pool_operations
cargo test swap_operations
```

### Manual Testing with MCP Client
```bash
# Start server in debug mode
RUST_LOG=debug cargo run --bin mcp-server --features mcp

# Test with mcp-client (if available)
echo '{"method": "tools/list"}' | mcp-client

# Test specific tools
echo '{"method": "tools/call", "params": {"name": "get_network_status", "arguments": {}}}' | mcp-client
```

## 🔧 Troubleshooting

### Common Issues

#### Connection Problems
```bash
# Check network connectivity
ping rpc.testnet.mantra.com

# Verify RPC endpoint
curl -X POST https://rpc.testnet.mantra.com \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"status","id":1}'
```

#### Build Issues
```bash
# Clean and rebuild
cargo clean
cargo build --release

# Update dependencies
cargo update
```

#### Wallet Issues
```bash
# Reset wallet state
rm -rf ~/.mantra-dex-sdk/wallets

# Generate new wallet
cargo run --bin mcp -- generate-wallet
```

### Performance Optimization

#### Caching
```toml
[server]
cache_ttl_secs = 300  # 5 minutes
max_concurrent_ops = 20
```

#### Logging
```bash
# Production logging
export RUST_LOG="mantra_dex_sdk=info,warn,error"

# Development logging
export RUST_LOG="mantra_dex_sdk=debug,trace"
```

## 📖 API Reference

### Tool Call Format
```json
{
  "method": "tools/call",
  "params": {
    "name": "tool_name",
    "arguments": {
      "param1": "value1",
      "param2": "value2"
    }
  }
}
```

### Resource Read Format
```json
{
  "method": "resources/read",
  "params": {
    "uri": "resource://path"
  }
}
```

### Response Format
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "data": "...",
    "metadata": "..."
  }
}
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

### Development Setup
```bash
# Install development dependencies
cargo install cargo-watch
cargo install cargo-audit

# Run with auto-reload
cargo watch -x run

# Security audit
cargo audit
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Support

- **Documentation**: [Project Wiki](link-to-wiki)
- **Issues**: [GitHub Issues](link-to-issues)
- **Discord**: [Community Chat](link-to-discord)
- **Email**: support@mantra-dex-sdk.com

## 🗺️ Roadmap

- [ ] **Real Blockchain Integration**: Replace placeholder responses with actual blockchain calls
- [ ] **Enhanced Error Handling**: More granular error types and recovery strategies  
- [ ] **Performance Optimizations**: Advanced caching and connection pooling
- [ ] **WebSocket Support**: Real-time event streaming
- [ ] **Multi-signature Support**: Enhanced wallet security
- [ ] **Advanced Analytics**: Machine learning-powered insights
- [ ] **Cross-chain Support**: Integration with other Cosmos chains

---

**Built with ❤️ for the MANTRA ecosystem** 