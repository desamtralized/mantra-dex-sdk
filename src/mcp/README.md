# Mantra DEX SDK MCP Server

A comprehensive Model Context Protocol (MCP) server that provides AI agents and other MCP clients with full access to the Mantra DEX on the Mantra Blockchain. This server exposes all DEX functionality through a standardized MCP interface, enabling AI-powered DeFi operations.

## 📖 Overview

The Mantra DEX SDK MCP Server bridges the gap between AI agents and the Mantra DEX, allowing for:

- **Wallet Management**: Create, import, and manage HD wallets with mnemonic phrases
- **DEX Trading Operations**: Execute swaps, provide liquidity, and manage positions
- **Pool Management**: Query pools, create new pools (admin), and manage pool features
- **Rewards System**: Query and claim rewards from liquidity provision
- **Transaction Monitoring**: Track transaction status and validate results
- **Analytics & Reporting**: Generate trading reports and performance analysis

## 🚀 Features

### Core Capabilities

#### 🔑 Wallet Operations
- Generate new HD wallets with BIP39 mnemonic phrases
- Import existing wallets from mnemonic
- Switch between multiple wallets
- Query wallet balances and information

#### 🏊‍♂️ DEX Operations
- **Swapping**: Single-hop and multi-hop token swaps with slippage protection
- **Liquidity**: Provide and withdraw liquidity from pools
- **Rewards**: Query pending rewards and claim accumulated rewards
- **Pool Management**: Create pools, update features, and query pool status

#### 📊 Advanced Features
- Real-time transaction monitoring with status updates
- Comprehensive swap validation and analysis
- Trading history and performance analytics
- Impermanent loss calculations for liquidity positions
- LP token balance queries and withdrawal estimations

### 🛠 Available Tools

The MCP server provides 40+ tools organized into categories:

**Wallet Management:**
- `generate_wallet` - Create new HD wallets
- `import_wallet` - Import from mnemonic phrase
- `get_wallet_info` - Get active wallet details
- `get_wallet_balance` - Query token balances

**Pool Operations:**
- `get_pool` - Query specific pool information
- `get_pools` - List all available pools
- `validate_pool_status` - Check pool availability
- `create_pool` - Create new liquidity pools (admin)

**Trading Operations:**
- `simulate_swap` - Preview swap outcomes
- `execute_swap` - Perform token swaps
- `provide_liquidity` - Add liquidity to pools
- `withdraw_liquidity` - Remove liquidity positions

**Transaction Monitoring:**
- `monitor_swap_transaction` - Track swap execution
- `get_transaction_monitor_status` - Check monitoring status
- `validate_swap_result` - Validate swap outcomes

**Analytics & Reporting:**
- `get_swap_history` - Query trading history
- `generate_trading_report` - Create performance reports
- `calculate_impermanent_loss` - Analyze LP position performance

### 📡 Available Resources

**Trading Resources:**
- `trades://history` - Access trading history data
- `trades://pending` - View pending transactions
- `liquidity://positions` - Query liquidity positions

**Network Resources:**
- `network://status` - Network health and connectivity
- `network://config` - Current network configuration
- `contracts://addresses` - Contract addresses for current network

## 📋 Prerequisites

- **Rust**: Version 1.70.0 or higher
- **Operating System**: Linux, macOS, or Windows
- **Network Access**: Internet connection for blockchain operations
- **MCP Client**: Cursor or other MCP-compatible client

## ⚡ Quick Start

### 1. Build the Server

```bash
# Clone the repository (if not already done)
cd /path/to/mantra-dex-sdk

# Build the MCP server binary
cargo build --release --bin mcp-server --features mcp

# The binary will be available at: target/release/mcp-server
```

### 2. Run the Server

**For Cursor Integration (stdio transport):**
```bash
# Run with stdio transport (recommended for Cursor)
./target/release/mcp-server --transport stdio --network testnet --debug

# Or using cargo run
cargo run --bin mcp-server --features mcp -- --transport stdio --network testnet --debug
```

**For HTTP API (http transport):**
```bash
# Run HTTP server on localhost:8080
./target/release/mcp-server --transport http --host 127.0.0.1 --port 8080 --network testnet

# Access at: http://127.0.0.1:8080
```

### 3. Test Connection

```bash
# Test basic functionality
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}' | ./target/release/mcp-server --transport stdio
```

## 🔧 Configuration

### Command Line Options

```bash
./target/release/mcp-server [OPTIONS]

Options:
  -t, --transport <TYPE>      Transport type: stdio or http [default: stdio]
  -n, --network <NETWORK>     Network: mainnet, testnet, mantra-dukong [default: mantra-dukong]
  -p, --port <PORT>           Port for HTTP server [default: 8080]
      --host <HOST>           Host for HTTP server [default: 127.0.0.1]
  -d, --debug                 Enable debug logging
      --log-format <FORMAT>   Log format: json, compact, pretty [default: compact]
      --log-file <FILE>       Log to file instead of stderr
      --disable-colors        Disable colored output
  -h, --help                  Print help
  -V, --version               Print version
```

### Environment Variables

```bash
# Network configuration
export MANTRA_NETWORK=testnet
export MANTRA_RPC_URL=https://rpc.testnet.mantra.com

# Logging configuration
export RUST_LOG=debug
export MCP_LOG_LEVEL=debug
export MCP_LOG_FORMAT=json

# Server configuration
export MCP_SERVER_PORT=8080
export MCP_SERVER_HOST=0.0.0.0
```

### Configuration File

Create `config/mcp.toml`:

```toml
[server]
name = "Mantra DEX MCP Server"
version = "0.1.0"
debug = true
max_concurrent_ops = 10
request_timeout_secs = 30
cache_ttl_secs = 300

[network]
name = "testnet"
rpc_url = "https://rpc.testnet.mantra.com"
chain_id = "mantra-dukong-1"

[transport]
type = "stdio"
http_host = "127.0.0.1"
http_port = 8080

[logging]
level = "info"
format = "compact"
enable_colors = true
```

## 🧪 Testing

### Unit Tests
```bash
# Run all tests
cargo test --lib

# Run MCP-specific tests
cargo test --lib mcp

# Run with debug output
cargo test --lib mcp -- --nocapture
```

### Integration Testing
```bash
# Test against testnet
cargo test --test integration_test --features testnet

# Test specific functionality
cargo test --test integration_test test_wallet_operations
```

### Manual Testing

**Test tool listing:**
```bash
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}' | \
  ./target/release/mcp-server --transport stdio --network testnet
```

**Test wallet generation:**
```bash
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "generate_wallet", "arguments": {}}}' | \
  ./target/release/mcp-server --transport stdio --network testnet
```

## 🔍 Usage Examples

### Wallet Management

```bash
# Generate a new wallet
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "generate_wallet", "arguments": {}}}' | ./target/release/mcp-server --transport stdio

# Import existing wallet
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "import_wallet", "arguments": {"mnemonic": "your twelve word mnemonic phrase here example test case", "account_index": 0}}}' | ./target/release/mcp-server --transport stdio

# Get wallet info
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_wallet_info", "arguments": {}}}' | ./target/release/mcp-server --transport stdio
```

### Pool Operations

```bash
# List all pools
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "get_pools", "arguments": {"limit": 10}}}' | ./target/release/mcp-server --transport stdio

# Get specific pool
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "get_pool", "arguments": {"pool_id": "1"}}}' | ./target/release/mcp-server --transport stdio

# Validate pool status
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "validate_pool_status", "arguments": {"pool_id": "1"}}}' | ./target/release/mcp-server --transport stdio
```

### Trading Operations

```bash
# Simulate a swap
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "simulate_swap", "arguments": {"pool_id": "1", "offer_asset_denom": "uom", "offer_asset_amount": "1000000", "ask_asset_denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY"}}}' | ./target/release/mcp-server --transport stdio

# Execute a swap
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "execute_swap", "arguments": {"pool_id": "1", "offer_asset_denom": "uom", "offer_asset_amount": "1000000", "ask_asset_denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY", "max_slippage": "0.05"}}}' | ./target/release/mcp-server --transport stdio
```

## 🚨 Error Handling

The server provides comprehensive error handling with detailed error codes:

### Common Error Codes
- `1001` - Wallet not configured
- `1002` - Invalid tool arguments
- `1003` - Network connection error
- `1004` - Transaction validation error
- `1005` - Pool not found
- `1006` - Insufficient balance
- `1007` - Slippage tolerance exceeded

### Error Response Format
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": 1001,
    "message": "Wallet not configured",
    "data": {
      "error_type": "WalletNotConfigured",
      "suggestions": ["Import or generate a wallet first"],
      "severity": "error"
    }
  }
}
```

## 🔒 Security Considerations

### Private Key Security
- **Never expose private keys**: The server never returns private keys in responses
- **Mnemonic handling**: Store mnemonics securely and never log them
- **Memory safety**: Private keys are cleared from memory after use

### Network Security
- **RPC endpoints**: Use trusted RPC endpoints
- **TLS/SSL**: Use HTTPS endpoints for mainnet operations
- **Rate limiting**: Built-in rate limiting to prevent abuse

### Validation
- **Input validation**: All inputs are validated before processing
- **Transaction validation**: Comprehensive transaction result validation
- **Slippage protection**: Built-in slippage protection for trades

## 📊 Monitoring and Logging

### Log Levels
- `ERROR` - Critical errors requiring attention
- `WARN` - Warnings and recoverable errors
- `INFO` - General operational information
- `DEBUG` - Detailed debugging information
- `TRACE` - Very detailed trace information

### Log Formats
- `compact` - Human-readable compact format
- `pretty` - Pretty-printed format with colors
- `json` - Structured JSON format for log aggregation

### Metrics
The server tracks various metrics:
- Request count and response times
- Transaction success/failure rates
- Active wallet count
- Pool query statistics
- Error rate by category

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/new-tool`
3. Make your changes and add tests
4. Run the test suite: `cargo test`
5. Commit your changes: `git commit -m "Add new tool"`
6. Push to the branch: `git push origin feature/new-tool`
7. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](../../LICENSE) file for details.

## 🆘 Support

- **Documentation**: Check the [User Guide](MCP_USER_GUIDE.md) and [Cursor Integration Guide](CURSOR_INTEGRATION_GUIDE.md)
- **Issues**: Report issues on the GitHub repository
- **Discord**: Join the Mantra community Discord for support

## 🔗 Links

- [Mantra Blockchain](https://mantrachain.io/)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [Cursor IDE](https://cursor.com/)
- [Mantra DEX Documentation](https://docs.mantrachain.io/)

---

**Built with ❤️ for the Mantra ecosystem** 