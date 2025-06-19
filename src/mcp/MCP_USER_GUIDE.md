# Mantra DEX MCP Server - User Guide

This comprehensive guide covers all functionality provided by the Mantra DEX MCP Server. Whether you're an AI agent, developer, or power user, this guide will help you make the most of the MCP server's capabilities.

## 📋 Table of Contents

1. [Getting Started](#getting-started)
2. [Wallet Management](#wallet-management)
3. [Network Operations](#network-operations)
4. [Pool Operations](#pool-operations)
5. [Trading Operations](#trading-operations)
6. [Transaction Monitoring](#transaction-monitoring)
7. [Analytics & Reporting](#analytics--reporting)
8. [Resource Access](#resource-access)
9. [Advanced Features](#advanced-features)
10. [Troubleshooting](#troubleshooting)

## 🚀 Getting Started

### Prerequisites

Before using the MCP server, ensure you have:

1. **Built the server**: `cargo build --release --bin mcp-server --features mcp`
2. **Network access**: Internet connection for blockchain operations
3. **Wallet funds**: Testnet tokens for testing (get from faucet)

### First Steps

1. **Start the server**:
   ```bash
   ./target/release/mcp-server --transport stdio --network testnet --debug
   ```

2. **List available tools**:
   ```json
   {"jsonrpc": "2.0", "id": 1, "method": "tools/list"}
   ```

3. **Generate your first wallet**:
   ```json
   {
     "jsonrpc": "2.0", 
     "id": 1, 
     "method": "tools/call", 
     "params": {
       "name": "generate_wallet", 
       "arguments": {}
     }
   }
   ```

## 🔑 Wallet Management

### Generating New Wallets

**Tool**: `generate_wallet`

Creates a new HD wallet with a BIP39 mnemonic phrase.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "generate_wallet",
    "arguments": {}
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "wallet_address": "mantra1abc123...",
    "public_key": "02a1b2c3...",
    "mnemonic": "abandon ability able about above absent absorb abstract absurd abuse access accident",
    "account_index": 0,
    "message": "Wallet generated successfully"
  }
}
```

### Importing Existing Wallets

**Tool**: `import_wallet`

Import a wallet from an existing mnemonic phrase.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "import_wallet",
    "arguments": {
      "mnemonic": "your twelve word mnemonic phrase here example test case security backup crypto",
      "account_index": 0
    }
  }
}
```

**Parameters**:
- `mnemonic` (required): 12 or 24-word BIP39 mnemonic phrase
- `account_index` (optional): Account derivation index (default: 0)

### Getting Wallet Information

**Tool**: `get_wallet_info`

Get information about the currently active wallet.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_wallet_info",
    "arguments": {}
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "wallet_address": "mantra1abc123...",
    "public_key": "02a1b2c3...",
    "account_index": 0,
    "derivation_path": "m/44'/118'/0'/0/0",
    "network": "testnet"
  }
}
```

### Checking Wallet Balance

**Tool**: `get_wallet_balance`

Query the balance of all tokens in the active wallet.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_wallet_balance",
    "arguments": {}
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "wallet_address": "mantra1abc123...",
    "balances": [
      {
        "denom": "uom",
        "amount": "1000000000",
        "symbol": "OM",
        "decimals": 6,
        "formatted_amount": "1000.0 OM"
      },
      {
        "denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
        "amount": "500000000",
        "symbol": "aUSDY",
        "decimals": 6,
        "formatted_amount": "500.0 aUSDY"
      }
    ],
    "total_count": 2
  }
}
```

## 🌐 Network Operations

### Network Status

**Tool**: `get_network_status`

Get current network status and connectivity information.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_network_status",
    "arguments": {}
  }
}
```

### Block Height

**Tool**: `get_block_height`

Get the latest block height from the blockchain.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_block_height",
    "arguments": {}
  }
}
```

### Contract Addresses

**Tool**: `get_contract_addresses`

Get all contract addresses for the current network.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_contract_addresses",
    "arguments": {}
  }
}
```

## 🏊‍♂️ Pool Operations

### Listing Pools

**Tool**: `get_pools`

List all available liquidity pools with optional filtering.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_pools",
    "arguments": {
      "limit": 10,
      "start_after": null
    }
  }
}
```

**Parameters**:
- `limit` (optional): Maximum number of pools to return (default: 10)
- `start_after` (optional): Pool ID to start after for pagination

### Getting Pool Details

**Tool**: `get_pool`

Get detailed information about a specific pool.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_pool",
    "arguments": {
      "pool_id": "1"
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "pool": {
      "pool_id": "1",
      "pool_type": "ConstantProduct",
      "assets": [
        {
          "denom": "uom",
          "amount": "1000000000"
        },
        {
          "denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
          "amount": "500000000"
        }
      ],
      "features": {
        "swaps_enabled": true,
        "deposits_enabled": true,
        "withdrawals_enabled": true
      },
      "fees": {
        "swap_fee": "0.003",
        "protocol_fee": "0.001"
      }
    }
  }
}
```

### Validating Pool Status

**Tool**: `validate_pool_status`

Check if a pool is available for operations.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "validate_pool_status",
    "arguments": {
      "pool_id": "1"
    }
  }
}
```

### Creating Pools (Admin Only)

**Tool**: `create_pool`

Create a new liquidity pool (requires admin privileges).

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "create_pool",
    "arguments": {
      "pool_type": "ConstantProduct",
      "assets": [
        {
          "denom": "uom",
          "amount": "1000000000"
        },
        {
          "denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
          "amount": "500000000"
        }
      ],
      "swap_fee": "0.003",
      "protocol_fee": "0.001"
    }
  }
}
```

## 💱 Trading Operations

### Simulating Swaps

**Tool**: `simulate_swap`

Preview swap outcomes without executing the transaction.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "simulate_swap",
    "arguments": {
      "pool_id": "1",
      "offer_asset_denom": "uom",
      "offer_asset_amount": "1000000",
      "ask_asset_denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY"
    }
  }
}
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "success": true,
    "simulation": {
      "return_amount": "995000",
      "spread_amount": "2500",
      "commission_amount": "3000",
      "price_impact": "0.0025",
      "effective_price": "0.995",
      "fees": {
        "swap_fee": "3000",
        "protocol_fee": "1000"
      }
    }
  }
}
```

### Executing Swaps

**Tool**: `execute_swap`

Execute a token swap transaction.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "execute_swap",
    "arguments": {
      "pool_id": "1",
      "offer_asset_denom": "uom",
      "offer_asset_amount": "1000000",
      "ask_asset_denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
      "max_slippage": "0.05"
    }
  }
}
```

**Parameters**:
- `pool_id` (required): Pool to execute swap in
- `offer_asset_denom` (required): Asset to swap from
- `offer_asset_amount` (required): Amount to swap (in micro units)
- `ask_asset_denom` (required): Asset to swap to
- `max_slippage` (optional): Maximum slippage tolerance (0.05 = 5%)

### Providing Liquidity

**Tool**: `provide_liquidity`

Add liquidity to a pool and receive LP tokens.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "provide_liquidity",
    "arguments": {
      "pool_id": "1",
      "assets": [
        {
          "denom": "uom",
          "amount": "1000000"
        },
        {
          "denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
          "amount": "500000"
        }
      ],
      "max_slippage": "0.02"
    }
  }
}
```

### Withdrawing Liquidity

**Tool**: `withdraw_liquidity`

Remove liquidity from a pool by burning LP tokens.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "withdraw_liquidity",
    "arguments": {
      "pool_id": "1",
      "lp_token_amount": "100000"
    }
  }
}
```

## 📊 Transaction Monitoring

### Monitoring Swap Transactions

**Tool**: `monitor_swap_transaction`

Monitor the status of a swap transaction in real-time.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "monitor_swap_transaction",
    "arguments": {
      "tx_hash": "ABC123DEF456...",
      "min_confirmations": 1,
      "timeout_secs": 300,
      "monitor_events": true
    }
  }
}
```

### Checking Monitor Status

**Tool**: `get_transaction_monitor_status`

Check the status of a transaction monitor.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_transaction_monitor_status",
    "arguments": {
      "monitor_id": "monitor-uuid-123"
    }
  }
}
```

### Validating Swap Results

**Tool**: `validate_swap_result`

Validate that a completed swap meets expected criteria.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "validate_swap_result",
    "arguments": {
      "tx_hash": "ABC123DEF456...",
      "expected_pool_id": "1",
      "expected_offer_asset": {
        "denom": "uom",
        "amount": "1000000"
      },
      "expected_ask_asset_denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
      "slippage_tolerance": 0.05,
      "validate_events": true
    }
  }
}
```

## 📈 Analytics & Reporting

### Swap History

**Tool**: `get_swap_history`

Query trading history for the active wallet.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_swap_history",
    "arguments": {
      "limit": 50,
      "offset": 0,
      "status_filter": "all",
      "sort_by": "timestamp",
      "sort_order": "desc"
    }
  }
}
```

### Trading Statistics

**Tool**: `get_swap_statistics`

Get comprehensive trading statistics for a time period.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_swap_statistics",
    "arguments": {
      "time_period": "30d",
      "include_pool_breakdown": true,
      "include_asset_breakdown": true
    }
  }
}
```

### Trading Reports

**Tool**: `generate_trading_report`

Generate detailed trading performance reports.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "generate_trading_report",
    "arguments": {
      "period": "7d",
      "report_type": "detailed",
      "include_charts": false,
      "export_format": "json"
    }
  }
}
```

### LP Token Management

**Tool**: `get_lp_token_balance`

Query LP token balances for specific pools.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_lp_token_balance",
    "arguments": {
      "pool_id": "1"
    }
  }
}
```

**Tool**: `estimate_lp_withdrawal_amounts`

Estimate amounts received when withdrawing LP tokens.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "estimate_lp_withdrawal_amounts",
    "arguments": {
      "pool_id": "1",
      "lp_token_amount": "100000"
    }
  }
}
```

### Impermanent Loss Calculation

**Tool**: `calculate_impermanent_loss`

Calculate impermanent loss for liquidity positions.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "calculate_impermanent_loss",
    "arguments": {
      "pool_id": "1",
      "initial_deposit": {
        "asset_a_amount": "1000000",
        "asset_b_amount": "500000",
        "timestamp": "2024-01-01T00:00:00Z"
      },
      "comparison_method": "current_prices"
    }
  }
}
```

## 📡 Resource Access

The MCP server provides resources for accessing cached and historical data.

### Trading History Resource

Access trading history through the `trades://history` resource:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": {
    "uri": "trades://history"
  }
}
```

### Pending Trades Resource

Access pending transactions through the `trades://pending` resource:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": {
    "uri": "trades://pending"
  }
}
```

### Liquidity Positions Resource

Access current liquidity positions through the `liquidity://positions` resource:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "resources/read",
  "params": {
    "uri": "liquidity://positions"
  }
}
```

## 🔧 Advanced Features

### Parameter Validation

**Tool**: `validate_swap_parameters`

Validate swap parameters before execution.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "validate_swap_parameters",
    "arguments": {
      "pool_id": "1",
      "offer_denom": "uom",
      "offer_amount": "1000000",
      "ask_asset_denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
      "max_slippage": "0.05",
      "simulate_before_validation": true
    }
  }
}
```

### Swap Execution Summary

**Tool**: `get_swap_execution_summary`

Get detailed execution summary for completed swaps.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_swap_execution_summary",
    "arguments": {
      "tx_hash": "ABC123DEF456...",
      "include_pool_analysis": true,
      "include_fee_breakdown": true,
      "include_slippage_analysis": true
    }
  }
}
```

### Performance Analysis

**Tool**: `analyze_swap_performance`

Analyze swap performance and get optimization recommendations.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "analyze_swap_performance",
    "arguments": {
      "time_period": "30d",
      "include_gas_analysis": true,
      "include_slippage_analysis": true,
      "include_timing_analysis": true
    }
  }
}
```

## 🚨 Troubleshooting

### Common Issues

#### 1. Wallet Not Configured
**Error**: `Wallet not configured`
**Solution**: Generate or import a wallet first:
```json
{"name": "generate_wallet", "arguments": {}}
```

#### 2. Insufficient Balance
**Error**: `Insufficient balance for transaction`
**Solution**: Check balance and ensure you have enough tokens:
```json
{"name": "get_wallet_balance", "arguments": {}}
```

#### 3. Pool Not Found
**Error**: `Pool not found: 999`
**Solution**: List available pools and use valid pool ID:
```json
{"name": "get_pools", "arguments": {"limit": 10}}
```

#### 4. Slippage Exceeded
**Error**: `Slippage tolerance exceeded`
**Solution**: Increase slippage tolerance or reduce trade size:
```json
{
  "name": "execute_swap",
  "arguments": {
    "max_slippage": "0.10"
  }
}
```

#### 5. Network Connection Error
**Error**: `Network connection error`
**Solution**: Check network connectivity and RPC endpoint:
```json
{"name": "get_network_status", "arguments": {}}
```

### Debug Mode

Enable debug mode for detailed logging:
```bash
./target/release/mcp --transport stdio --network testnet --debug --log-format pretty
```

### Validation Tools

Use built-in validation tools to check parameters:

```json
{
  "name": "validate_swap_parameters",
  "arguments": {
    "pool_id": "1",
    "offer_denom": "uom",
    "offer_amount": "1000000",
    "ask_asset_denom": "factory/mantra1x5nk33zpglp4ge6q9a8xx3zceqf4g8nvaggjmc/aUSDY",
    "simulate_before_validation": true
  }
}
```

### Error Recovery

Many operations support automatic retry with exponential backoff. Check error responses for retry recommendations:

```json
{
  "error": {
    "code": 1003,
    "message": "Network connection error",
    "data": {
      "retry_after_secs": 30,
      "max_retries": 3,
      "recoverable": true
    }
  }
}
```

## 📞 Support

For additional help:

1. **Check logs**: Enable debug mode for detailed logging
2. **Validate inputs**: Use validation tools before executing operations
3. **Network status**: Check network connectivity with `get_network_status`
4. **Community**: Join the Mantra Discord for community support

---

This user guide covers all major functionality of the Mantra DEX MCP Server. For the latest updates and additional features, check the project repository and documentation. 