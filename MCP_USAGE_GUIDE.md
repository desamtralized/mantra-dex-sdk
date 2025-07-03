# Mantra DEX MCP Server - Usage Guide

This guide provides detailed instructions for using the Mantra DEX MCP Server with various MCP clients, particularly Claude Desktop.

## 🚀 Quick Setup for Claude Desktop

### Step 1: Build the MCP Server

```bash
git clone <repository-url>
cd mcp-mantra-dex-sdk
cargo build --release
```

### Step 2: Configure Claude Desktop

Edit your Claude Desktop configuration file:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`
**Linux**: `~/.config/Claude/claude_desktop_config.json`

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

### Step 3: Restart Claude Desktop

After saving the configuration, restart Claude Desktop. You should see the Mantra DEX tools become available.

## 💬 Using with Claude Desktop

### Example Conversations

**Generate a wallet:**
> "Generate a new wallet for me and save it with the name 'my-trading-wallet'"

**Check pools:**
> "Show me all available pools on the Mantra DEX"

**Execute a swap:**
> "Swap 1 OM for USDY with 3% slippage tolerance"


## 🛠️ Manual MCP Usage

### Starting the Server
```bash
cargo run --bin mcp-server --features mcp
```

### Basic Protocol Messages

#### List Tools
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list"
}
```

#### Call a Tool
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "generate_wallet",
    "arguments": {
      "account_index": 0,
      "save_wallet": true,
      "wallet_name": "test-wallet"
    }
  }
}
```

## 📊 Available Tools (9 Total)

### Wallet Management (4 tools)
- `generate_wallet` - Create new HD wallets
- `import_wallet` - Import from mnemonic phrases  
- `get_wallet_info` - Get active wallet information
- `get_wallet_balance` - Query wallet token balances

### Network Operations (3 tools)
- `get_network_status` - Network health and connectivity
- `get_block_height` - Current blockchain height
- `get_contract_addresses` - DEX contract addresses

### Pool Management (2 tools)
- `get_pool` - Query specific pool information
- `get_pools` - List all available pools

### Trading Operations (4 tools)
- `simulate_swap` - Preview swap outcomes
- `execute_swap` - Perform token swaps
- `provide_liquidity` - Add liquidity to pools
- `withdraw_liquidity` - Remove liquidity from pools

### LP Token Management (2 tools)
- `get_lp_token_balance` - Query LP token balances
- `get_all_lp_token_balances` - Get all LP positions



## 📋 Available Resources (0 Total)

*No resources are currently available - analytics and monitoring resources have been removed to simplify the server.*

## 🔧 Troubleshooting

### Common Issues

1. **"Server not found" in Claude Desktop**
   - Verify the `cwd` path is absolute and correct
   - Ensure `cargo` is in your PATH
   - Test: `cargo build --release`

2. **Network connection issues**
   - Test: `ping rpc.testnet.mantra.com`
   - Check firewall settings

3. **Wallet-related errors**
   - Generate/import a wallet first
   - Check wallet balance for gas fees
   - Verify correct network (testnet/mainnet)

### Debug Mode

Enable debug logging:
```json
{
  "env": {
    "RUST_LOG": "debug"
  }
}
```

## 🔒 Security Best Practices

- **Never share mnemonic phrases**
- **Use testnet for development**
- **Keep private keys secure**
- **Validate transaction parameters**
- **Use appropriate slippage settings**

## 🎯 Getting Started Checklist

1. ✅ Build the MCP server: `cargo build --release --features mcp`
2. ✅ Configure Claude Desktop with absolute path
3. ✅ Restart Claude Desktop
4. ✅ Generate or import a wallet
5. ✅ Check network status
6. ✅ Explore available pools
7. ✅ Try a simple swap on testnet

Happy trading with the Mantra DEX MCP Server! 🚀 