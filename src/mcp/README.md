# MANTRA DEX MCP Server

A Model Context Protocol (MCP) server implementation that provides AI agents with comprehensive access to the MANTRA blockchain DEX. Built in Rust with full JSON-RPC 2.0 compliance and async operation support.

## Architecture Overview

### Core Components

The MCP server is organized into three main modules in `src/mcp/`:

- **`server.rs`** - Core MCP server with JSON-RPC 2.0 handling
- **`sdk_adapter.rs`** - Adapter layer between MCP protocol and SDK operations
- **`client_wrapper.rs`** - MCP client wrapper and request processing

### Design Philosophy

The server follows a clean separation of concerns:

1. **Protocol Layer** (`server.rs`) - Handles MCP protocol specifics, JSON-RPC, transport
2. **Adapter Layer** (`sdk_adapter.rs`) - Translates MCP calls to SDK operations
3. **Core SDK** (`../client.rs`) - Business logic and blockchain interactions

This architecture ensures the MCP server is a thin protocol adapter over the robust SDK core.

## Development Setup

### Prerequisites

```bash
# Rust toolchain (1.70+)
rustup update stable

# Build dependencies
cargo check --features mcp
```

### Build Commands

```bash
# Development build
cargo build --features mcp

# Release build
cargo build --release --features mcp --bin mcp-server

# Run tests
cargo test --features mcp
```

## Code Structure

### Tool Implementation

Tools are implemented in `sdk_adapter.rs` using the adapter pattern:

```rust
// Tool definition
fn create_tool_def(name: &str, description: &str) -> Tool {
    Tool {
        name: name.to_string(),
        description: Some(description.to_string()),
        input_schema: create_input_schema(),
    }
}

// Tool execution
async fn handle_tool_call(&self, name: &str, arguments: &Value) -> Result<Vec<TextContent>, McpError> {
    match name {
        "execute_swap" => self.execute_swap(arguments).await,
        "get_pools" => self.get_pools(arguments).await,
        // ... other tools
        _ => Err(McpError::InvalidRequest(format!("Unknown tool: {}", name))),
    }
}
```

### Resource Implementation

Resources provide read-only access to data:

```rust
// Resource definition
fn create_resource(uri: &str, name: &str, description: &str) -> Resource {
    Resource {
        uri: uri.to_string(),
        name: Some(name.to_string()),
        description: Some(description.to_string()),
        mime_type: Some("application/json".to_string()),
    }
}

// Resource reading
async fn read_resource(&self, uri: &str) -> Result<Vec<ResourceContents>, McpError> {
    match uri {
        "network://status" => self.get_network_status().await,
        "trades://history" => self.get_trade_history().await,
        // ... other resources
    }
}
```

### Error Handling

The server implements comprehensive error handling with specific error codes:

```rust
#[derive(Debug)]
pub enum McpError {
    InvalidRequest(String),           // -32600
    MethodNotFound(String),           // -32601
    InvalidParams(String),            // -32602
    InternalError(String),            // -32603
    WalletNotConfigured(String),      // -1001
    NetworkError(String),             // -1002
}
```

## Available Tools

### Wallet Management
- `get_active_wallet` - Get current wallet information
- `list_wallets` - List all available wallets
- `switch_wallet` - Switch to different wallet
- `add_wallet_from_mnemonic` - Import wallet from mnemonic
- `remove_wallet` - Remove wallet from collection
- `get_balances` - Get wallet token balances

### Pool Operations
- `get_pools` - List all liquidity pools
- `get_contract_addresses` - Get contract addresses
- `validate_network_connectivity` - Check network status

### Trading Operations
- `execute_swap` - Execute token swaps with slippage protection
- `provide_liquidity` - Add liquidity to pools
- `withdraw_liquidity` - Remove liquidity from pools
- `create_pool` - Create new pools (admin only)

### LP Token Management
- `get_lp_token_balance` - Get LP balance for specific pool
- `get_all_lp_token_balances` - Get all LP balances
- `estimate_lp_withdrawal_amounts` - Estimate withdrawal amounts

## Available Resources

### Network Information
- `network://status` - Network health and connectivity status
- `network://config` - Current network configuration
- `contracts://addresses` - Smart contract addresses

## Development Workflow

### Adding New Tools

1. **Define the tool** in `sdk_adapter.rs`:
```rust
fn create_my_tool() -> Tool {
    Tool {
        name: "my_new_tool".to_string(),
        description: Some("Description of what this tool does".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "param1": {"type": "string", "description": "Parameter description"}
            },
            "required": ["param1"]
        }),
    }
}
```

2. **Implement the handler**:
```rust
async fn handle_my_tool(&self, arguments: &Value) -> Result<Vec<TextContent>, McpError> {
    let param1 = arguments["param1"].as_str()
        .ok_or_else(|| McpError::InvalidParams("param1 is required".to_string()))?;

    // Use SDK client for actual operation
    let result = self.client.my_operation(param1).await
        .map_err(|e| McpError::InternalError(format!("Operation failed: {}", e)))?;

    Ok(vec![TextContent::text(serde_json::to_string_pretty(&result)?)])
}
```

3. **Register in tool list** and **add to handler match**:
```rust
// In list_tools()
tools.push(self.create_my_tool());

// In call_tool()
"my_new_tool" => self.handle_my_tool(arguments).await,
```

### Adding New Resources

1. **Define resource**:
```rust
fn create_my_resource() -> Resource {
    Resource {
        uri: "my://resource".to_string(),
        name: Some("My Resource".to_string()),
        description: Some("Resource description".to_string()),
        mime_type: Some("application/json".to_string()),
    }
}
```

2. **Implement reader**:
```rust
async fn read_my_resource(&self) -> Result<Vec<ResourceContents>, McpError> {
    let data = self.client.get_my_data().await?;

    Ok(vec![ResourceContents::text(
        serde_json::to_string_pretty(&data)?,
    )])
}
```

## Testing

### Unit Tests

```bash
# Test specific module
cargo test --features mcp sdk_adapter

# Test with output
cargo test --features mcp -- --nocapture

# Test specific function
cargo test --features mcp test_wallet_operations
```

### Integration Testing

```bash
# Test against testnet
MANTRA_NETWORK=testnet cargo test --features mcp,integration

# Test MCP protocol compliance
cargo test --features mcp test_mcp_protocol
```

### Manual Testing

Test tool execution:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_pools","arguments":{}}}' | \
  cargo run --features mcp --bin mcp-server -- --transport stdio --network testnet
```

Test resource reading:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"network://status"}}' | \
  cargo run --features mcp --bin mcp-server -- --transport stdio
```

## Configuration

### Environment Variables

```bash
# Network selection
export MANTRA_NETWORK=testnet  # or mainnet, mantra-dukong

# Custom RPC endpoint
export MANTRA_RPC_URL=https://rpc.testnet.mantrachain.io

# Wallet configuration (for automation/testing)
export WALLET_MNEMONIC="your twelve word mnemonic phrase here example test case development automation"

# Logging
export RUST_LOG=debug
export MCP_DEBUG=true
```

### Command Line Options

```bash
# Basic usage
cargo run --features mcp --bin mcp-server -- \
  --transport stdio \
  --network testnet \
  --debug

# Automated setup with pre-configured wallet
export WALLET_MNEMONIC="abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
export MANTRA_NETWORK=testnet
cargo run --features mcp --bin mcp-server -- \
  --transport stdio \
  --debug
```

## Transport Protocols

### STDIO Transport
Primary transport for AI integrations (Cursor, etc.):
```bash
# Start server
cargo run --features mcp --bin mcp-server -- --transport stdio

# Send JSON-RPC request via stdin
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | ./mcp-server
```

### HTTP Transport
For web integrations and debugging:
```bash
# Start HTTP server
cargo run --features mcp --bin mcp-server -- --transport http --port 8080

# Make HTTP request
curl -X POST http://localhost:8080 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

## Error Handling

All operations return structured errors with specific codes:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -1001,
    "message": "Wallet not configured",
    "data": {
      "error_type": "WalletNotConfigured",
      "suggestions": ["Use add_wallet_from_mnemonic to import a wallet"]
    }
  }
}
```

## Performance Considerations

### Async Operations
All blockchain operations are async to prevent blocking:

```rust
// Non-blocking pool queries
let pools = tokio::spawn(async move {
    client.get_pools(None, None).await
});

// Concurrent balance checks
let futures = wallets.iter().map(|w| client.get_balances(&w.address));
let balances = futures::future::join_all(futures).await;
```

### Caching Strategy
The server implements smart caching for expensive operations:

- Pool information cached for 5 minutes
- Balance queries cached for 30 seconds
- Network status cached for 60 seconds

## Security

### Private Key Management
- Private keys never exposed in MCP responses
- Mnemonics handled securely in memory
- Wallet encryption when persisted

### Environment Variable Security
When using `WALLET_MNEMONIC` for automation:
- **Development/Testing Only** - Never use in production environments
- **Secure Storage** - Store in secure environment variable systems (e.g., GitHub Secrets, HashiCorp Vault)
- **Process Isolation** - Ensure environment is not shared with untrusted processes
- **Cleanup** - Unset the variable after use: `unset WALLET_MNEMONIC`
- **CI/CD Security** - Use encrypted secrets in CI/CD pipelines, never commit to repositories

### Input Validation
All tool parameters validated before SDK calls:

```rust
fn validate_amount(amount: &str) -> Result<u128, McpError> {
    amount.parse::<u128>()
        .map_err(|_| McpError::InvalidParams("Invalid amount format".to_string()))
}
```

## Contributing

1. Implement new features in SDK core first (`../client.rs`)
2. Add MCP wrapper in `sdk_adapter.rs`
3. Add comprehensive tests
4. Update documentation
5. Ensure error handling follows existing patterns

## Debugging

Enable debug logging:
```bash
RUST_LOG=debug cargo run --features mcp --bin mcp-server -- --debug
```

Use JSON logging for structured output:
```bash
MCP_LOG_FORMAT=json cargo run --features mcp --bin mcp-server
```

## Integration with AI Tools

The server works with any MCP-compatible client. For Claude Code integration, add to your MCP settings:

```json
{
  "mcpServers": {
    "mantra-dex": {
      "command": "/path/to/mantra-dex-sdk/target/release/mcp-server",
        "args": ["--transport", "stdio", "--network", "mantra-dukong"],
        "env": {
          "RUST_LOG": "info",
          "MCP_LOG_LEVEL": "debug",
          "WALLET_MNEMONIC": "your twelve or twenty four words mnemonic"
        }
    }
  }
}
```
