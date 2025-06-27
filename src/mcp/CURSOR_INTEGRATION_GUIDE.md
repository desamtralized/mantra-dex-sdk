# Mantra DEX MCP Server - Cursor Integration Guide

This guide provides step-by-step instructions for integrating the Mantra DEX MCP Server with Cursor IDE, enabling AI-powered DeFi operations directly within your development environment.

## 📋 Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation & Setup](#installation--setup)
3. [Configuration Methods](#configuration-methods)
4. [Using MCP Tools in Cursor](#using-mcp-tools-in-cursor)
5. [Advanced Configuration](#advanced-configuration)
6. [Troubleshooting](#troubleshooting)
7. [Best Practices](#best-practices)
8. [Examples & Use Cases](#examples--use-cases)

## 🔧 Prerequisites

Before integrating with Cursor, ensure you have:

1. **Cursor IDE**: Latest version with MCP support
2. **Rust Environment**: Version 1.70.0+ for building the server
3. **Mantra DEX SDK**: This project cloned and built
4. **Network Access**: Internet connection for blockchain operations

## ⚡ Installation & Setup

### Step 1: Build the MCP Server

```bash
# Navigate to your project directory
cd /path/to/mantra-dex-sdk

# Build the MCP server binary
cargo build --release --bin mcp

# Verify the binary was created
ls -la target/release/mcp
```

### Step 2: Test the Server

```bash
# Test basic functionality
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}' | \
  ./target/release/mcp --transport stdio --network testnet
```

If successful, you should see a JSON response listing available tools.

### Step 3: Configure Cursor MCP

You have two options for configuration:

#### Option A: Project-Specific Configuration

Create `.cursor/mcp.json` in your project directory:

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "./target/release/mcp",
      "args": [
        "--transport", "stdio",
        "--network", "testnet",
        "--debug"
      ],
      "env": {
        "RUST_LOG": "info",
        "MCP_LOG_LEVEL": "debug"
      }
    }
  }
}
```

#### Option B: Global Configuration

Create `~/.cursor/mcp.json` for use across all projects:

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "/full/path/to/mantra-dex-sdk/target/release/mcp",
      "args": [
        "--transport", "stdio",
        "--network", "testnet",
        "--debug"
      ],
      "env": {
        "RUST_LOG": "info",
        "MCP_LOG_LEVEL": "debug",
        "MANTRA_NETWORK": "testnet"
      }
    }
  }
}
```

## 🔧 Configuration Methods

### Basic Configuration

For simple usage with default settings:

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "./target/release/mcp",
      "args": ["--transport", "stdio", "--network", "testnet"]
    }
  }
}
```

### Network-Specific Configuration

#### Testnet Configuration
```json
{
  "mcpServers": {
    "mantra-dex-testnet": {
      "command": "./target/release/mcp",
      "args": [
        "--transport", "stdio",
        "--network", "testnet",
        "--debug"
      ],
      "env": {
        "MANTRA_NETWORK": "testnet",
        "MANTRA_RPC_URL": "https://rpc.testnet.mantra.com"
      }
    }
  }
}
```

#### Mainnet Configuration
```json
{
  "mcpServers": {
    "mantra-dex-mainnet": {
      "command": "./target/release/mcp",
      "args": [
        "--transport", "stdio",
        "--network", "mainnet"
      ],
      "env": {
        "MANTRA_NETWORK": "mainnet",
        "MANTRA_RPC_URL": "https://rpc.mainnet.mantra.com"
      }
    }
  }
}
```

### Development Configuration

For development with enhanced logging:

```json
{
  "mcpServers": {
    "mantra-dex-dev": {
      "command": "./target/release/mcp",
      "args": [
        "--transport", "stdio",
        "--network", "testnet",
        "--debug",
        "--log-format", "pretty"
      ],
      "env": {
        "RUST_LOG": "debug",
        "MCP_LOG_LEVEL": "trace",
        "MCP_LOG_FORMAT": "pretty"
      }
    }
  }
}
```

## 🤖 Using MCP Tools in Cursor

### Accessing MCP Tools

1. **Open Cursor IDE**
2. **Open the Command Palette** (`Cmd/Ctrl + Shift + P`)
3. **Start a new chat** or use the **Composer**
4. **Check available tools** in the MCP settings

### Available Tools in Cursor

The Cursor Agent will automatically detect and use these tools when relevant:

#### Wallet Management Tools
- `generate_wallet` - Create new wallets
- `import_wallet` - Import existing wallets
- `get_wallet_info` - Get wallet details
- `get_wallet_balance` - Check balances

#### Trading Tools
- `simulate_swap` - Preview swaps
- `execute_swap` - Execute swaps
- `provide_liquidity` - Add liquidity
- `withdraw_liquidity` - Remove liquidity

#### Analytics Tools
*Analytics tools have been removed to simplify the server - focus on core DEX operations*

### Example Interactions

#### Setting Up a Wallet

```
You: "I need to set up a new wallet for testing the Mantra DEX"

Agent: I'll help you create a new wallet for Mantra DEX testing. Let me generate a new wallet for you.

[Tool Call: generate_wallet]

Great! I've created a new wallet:
- Address: mantra1abc123...
- Mnemonic: abandon ability able... (keep this secure!)

Would you like me to check the balance or help you get testnet tokens?
```

#### Executing a Swap

```
You: "I want to swap 1 OM for aUSDY using pool 1"

Agent: I'll help you execute that swap. Let me first simulate it to show you the expected outcome, then execute if you approve.

[Tool Call: simulate_swap]

Simulation results:
- You'll receive approximately 0.995 aUSDY
- Price impact: 0.25%
- Fees: 0.003 OM

Should I proceed with the swap?

You: "Yes, proceed"

[Tool Call: execute_swap]

Swap executed successfully! Transaction hash: ABC123...
```


### Tool Approval Settings

#### Manual Approval (Default)
Tools require approval before execution:
- Click the arrow next to tool name to expand
- Review arguments and parameters
- Click "Approve" to execute

#### Auto-Run Mode
Enable auto-run for automatic tool execution:
1. Go to Cursor Settings
2. Navigate to MCP settings
3. Enable "Auto-run" for Mantra DEX tools

**⚠️ Caution**: Auto-run mode will execute transactions without confirmation. Use carefully, especially on mainnet.

## 🔧 Advanced Configuration

### Multiple Network Support

Configure multiple networks simultaneously:

```json
{
  "mcpServers": {
    "mantra-dex-testnet": {
      "command": "./target/release/mcp",
      "args": ["--transport", "stdio", "--network", "testnet"]
    },
    "mantra-dex-mainnet": {
      "command": "./target/release/mcp",
      "args": ["--transport", "stdio", "--network", "mainnet"]
    }
  }
}
```

### Custom RPC Endpoints

```json
{
  "mcpServers": {
    "mantra-dex-custom": {
      "command": "./target/release/mcp",
      "args": ["--transport", "stdio", "--network", "testnet"],
      "env": {
        "MANTRA_RPC_URL": "https://custom-rpc.mantra.com",
        "MANTRA_CHAIN_ID": "mantra-dukong"
      }
    }
  }
}
```

### Resource Configuration

Configure the server for specific resources:

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "./target/release/mcp",
      "args": [
        "--transport", "stdio",
        "--network", "testnet",
        "--cache-ttl-secs", "600",
        "--request-timeout-secs", "30"
      ],
      "env": {
        "MCP_CACHE_TTL": "600",
        "MCP_MAX_CONCURRENT_OPS": "5"
      }
    }
  }
}
```

## 🚨 Troubleshooting

### Common Issues

#### 1. MCP Server Not Found

**Error**: `MCP server not found or failed to start`

**Solutions**:
1. Verify the binary path is correct:
   ```bash
   ls -la ./target/release/mcp
   ```

2. Test the server manually:
   ```bash
   ./target/release/mcp --transport stdio --network testnet
   ```

3. Check Cursor logs for detailed error messages

#### 2. Tools Not Appearing

**Error**: No Mantra DEX tools visible in Cursor

**Solutions**:
1. Restart Cursor IDE
2. Check MCP configuration syntax:
   ```bash
   # Validate JSON syntax
   cat .cursor/mcp.json | jq .
   ```

3. Verify the server is running:
   ```bash
   echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/list"}' | \
     ./target/release/mcp --transport stdio
   ```

#### 3. Network Connection Errors

**Error**: `Network connection error` or `RPC endpoint not responding`

**Solutions**:
1. Check network connectivity:
   ```bash
   curl -s https://rpc.testnet.mantra.com/status
   ```

2. Try alternative RPC endpoints:
   ```json
   {
     "env": {
       "MANTRA_RPC_URL": "https://alternative-rpc.mantra.com"
     }
   }
   ```

3. Verify network configuration:
   ```bash
   ./target/release/mcp --transport stdio --network testnet
   # Then test: {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "get_network_status", "arguments": {}}}
   ```

#### 4. Wallet Configuration Issues

**Error**: `Wallet not configured` when trying to execute trades

**Solutions**:
1. Generate a new wallet first:
   ```
   Ask Cursor: "Generate a new wallet for testing"
   ```

2. Import an existing wallet:
   ```
   Ask Cursor: "Import my wallet with mnemonic [your-mnemonic-phrase]"
   ```

3. Check wallet status:
   ```
   Ask Cursor: "What's my current wallet information?"
   ```

#### 5. Permission Errors

**Error**: `Permission denied` when starting the server

**Solutions**:
1. Ensure the binary is executable:
   ```bash
   chmod +x ./target/release/mcp
   ```

2. Check file ownership:
   ```bash
   ls -la ./target/release/mcp
   ```

### Debug Mode

Enable debug mode for detailed troubleshooting:

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "./target/release/mcp",
      "args": [
        "--transport", "stdio",
        "--network", "testnet",
        "--debug",
        "--log-format", "pretty"
      ],
      "env": {
        "RUST_LOG": "debug",
        "MCP_LOG_LEVEL": "trace"
      }
    }
  }
}
```

## 📚 Best Practices

### Security Best Practices

1. **Network Separation**: Use different configurations for testnet and mainnet
2. **Wallet Management**: Never commit wallet mnemonics to version control
3. **Transaction Approval**: Use manual approval for mainnet operations
4. **RPC Endpoints**: Use trusted RPC endpoints for mainnet

### Configuration Best Practices

1. **Environment Variables**: Use environment variables for sensitive configuration
2. **Logging**: Enable appropriate logging for your use case
3. **Timeouts**: Set reasonable timeouts for blockchain operations
4. **Resource Limits**: Configure resource limits to prevent abuse

### Usage Best Practices

1. **Start Small**: Begin with small amounts on testnet
2. **Validate First**: Use simulation tools before executing trades
3. **Monitor Transactions**: Always monitor transaction status
4. **Keep Records**: Use the analytics tools to track performance

## 🎯 Examples & Use Cases

### DeFi Portfolio Management

```
You: "Help me manage my DeFi portfolio. I want to:
1. Check my current balances
2. Analyze my liquidity positions
3. Calculate impermanent loss
4. Rebalance if needed"

Agent: I'll help you manage your DeFi portfolio. Let me start by checking your current balances and positions.

[Multiple tool calls for comprehensive portfolio analysis]
```

### Automated Trading Strategies

```
You: "I want to set up a simple DCA strategy - swap 10 OM for aUSDY every day"

Agent: I can help you plan a DCA strategy. Let me first simulate the swap to show you the expected outcomes, then we can discuss automation options.

[Tool calls for simulation and analysis]
```

### Liquidity Position Management

```
You: "Check my current liquidity positions and balances"

Agent: I'll check your current LP token balances and positions.

[Tool calls for LP balance queries]
```

### Pool Information

```
You: "Tell me about pool 5 before I provide liquidity"

Agent: I'll get detailed information about pool 5 for you.

[Tool calls for pool information and current status]
```

## 🔗 Additional Resources

- [Cursor MCP Documentation](https://docs.cursor.com/context/model-context-protocol)
- [Mantra DEX MCP Server README](README.md)
- [Comprehensive User Guide](MCP_USER_GUIDE.md)
- [Model Context Protocol Specification](https://modelcontextprotocol.io/)

## 🆘 Support

If you encounter issues:

1. **Check the logs**: Enable debug mode for detailed logging
2. **Validate configuration**: Ensure your MCP configuration is correct
3. **Test manually**: Test the server outside of Cursor
4. **Community support**: Join the Mantra Discord for help
5. **GitHub Issues**: Report bugs on the project repository

---

**Built with ❤️ for the Mantra ecosystem and Cursor IDE** 