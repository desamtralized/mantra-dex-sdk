# AI Coder Prompt: Implement Mantra DEX SDK MCP Server

## Context & Mission

You are tasked with implementing a comprehensive **Model Context Protocol (MCP) Server** that exposes all functionality of the **Mantra DEX SDK** for the Mantra blockchain. This server will enable AI agents like Claude to interact with the Mantra DEX through a standardized protocol.

The Mantra DEX SDK is a Rust library that provides:
- Wallet management and transaction signing
- DEX operations (swaps, liquidity provision, rewards)
- Pool management and administration
- Multi-hop swap routing
- Blockchain querying and transaction execution

## Architecture Overview

```
AI Agent (Claude) ↔ MCP Client ↔ MCP Server ↔ Mantra DEX SDK ↔ Mantra Blockchain
```

Your MCP server will be the bridge between MCP clients and the Mantra DEX SDK, translating MCP protocol messages into SDK method calls.

## Reference Documentation

**CRITICAL**: Study the Model Context Protocol documentation at https://modelcontextprotocol.io/tutorials/building-mcp-with-llms for complete MCP implementation guidance.

### Key MCP Concepts:
- **Resources**: Read-only data sources (wallet info, pool data, network status)
- **Tools**: Actions that can be executed (swap, provide liquidity, claim rewards)
- **Prompts**: Template-based interactions (not heavily used in this project)

## Existing SDK Analysis

### Core SDK Components (Located in `src/`):

#### 1. MantraDexClient (`src/client.rs` - 1180 lines)
**Async Methods to Expose:**
```rust
// Network & Account
pub async fn new(config: MantraNetworkConfig) -> Result<Self, Error>
pub async fn get_last_block_height(&self) -> Result<u64, Error>
pub async fn get_balances(&self) -> Result<Vec<Coin>, Error>

// Pool Operations
pub async fn get_pool(&self, pool_id: &str) -> Result<PoolInfoResponse, Error>
pub async fn get_pools(&self, limit: Option<u32>) -> Result<Vec<PoolInfoResponse>, Error>
pub async fn validate_pool_status(&self, pool_id: &str) -> Result<(), Error>

// Trading Operations
pub async fn simulate_swap(pool_id: &str, offer_asset: Coin, ask_asset_denom: &str) -> Result<SimulationResponse, Error>
pub async fn swap(pool_id: &str, offer_asset: Coin, ask_asset_denom: &str, max_slippage: Option<Decimal>) -> Result<TxResponse, Error>
pub async fn execute_swap_operations(operations: Vec<SwapOperation>, amount: Uint128) -> Result<TxResponse, Error>

// Liquidity Operations  
pub async fn provide_liquidity(pool_id: &str, assets: Vec<Coin>, liquidity_max_slippage: Option<Decimal>, swap_max_slippage: Option<Decimal>) -> Result<TxResponse, Error>
pub async fn withdraw_liquidity(pool_id: &str, lp_amount: Uint128) -> Result<TxResponse, Error>

// Rewards Operations
pub async fn claim_rewards(&self, until_epoch: Option<u64>) -> Result<TxResponse, Error>
pub async fn query_rewards(address: &str, until_epoch: Option<u64>) -> Result<serde_json::Value, Error>
pub async fn get_current_epoch(&self) -> Result<u64, Error>

// Admin Operations
pub async fn create_pool(asset_denoms: Vec<String>, asset_decimals: Vec<u8>, pool_fees: PoolFee, pool_type: PoolType, pool_identifier: Option<String>) -> Result<TxResponse, Error>
pub async fn update_pool_features(pool_identifier: &str, withdrawals_enabled: Option<bool>, deposits_enabled: Option<bool>, swaps_enabled: Option<bool>) -> Result<TxResponse, Error>
```

**Sync Methods to Expose:**
```rust
pub fn with_wallet(mut self, wallet: MantraWallet) -> Self
pub fn get_pool_status(&self, pool: &PoolInfoResponse) -> PoolStatus
pub fn validate_pool_fees(&self, pool_fees: &PoolFee) -> Result<(), Error>
pub fn create_validated_pool_fees(protocol_fee: Decimal, swap_fee: Decimal, burn_fee: Option<Decimal>, extra_fees: Option<Vec<Decimal>>) -> Result<PoolFee, Error>
```

#### 2. MantraWallet (`src/wallet/mod.rs` - 199 lines)
**Methods to Expose:**
```rust
pub fn from_mnemonic(mnemonic: &str, account_index: u32) -> Result<Self, Error>
pub fn generate() -> Result<(Self, String), Error>
pub fn address(&self) -> Result<AccountId, Error>
pub fn public_key(&self) -> PublicKey
pub fn info(&self) -> WalletInfo
pub fn create_fee(&self, amount: u64, gas_limit: u64, denom: &str) -> Result<Fee, Error>
pub fn create_default_fee(&self, gas_limit: u64) -> Result<Fee, Error>
```

#### 3. Network Configuration (`src/config.rs`)
**Structures to Expose:**
```rust
pub struct MantraNetworkConfig {
    pub network_name: String,
    pub network_id: String, 
    pub rpc_url: String,
    pub gas_price: f64,
    pub gas_adjustment: f64,
    pub native_denom: String,
    pub contracts: ContractAddresses,
}
```

## Implementation Requirements

### 1. Project Structure
Create the following file structure in `src/mcp/`:

```
src/mcp/
├── mod.rs                 # Module exports
├── server.rs             # Main MCP server implementation
├── sdk_adapter.rs        # SDK integration layer
├── config.rs             # MCP server configuration
├── error.rs              # Error handling and mapping
├── tools/
│   ├── mod.rs
│   ├── wallet.rs         # Wallet management tools
│   ├── network.rs        # Network operation tools
│   ├── pools.rs          # Pool management tools
│   ├── trading.rs        # Trading operation tools
│   ├── rewards.rs        # Rewards management tools
│   └── validation.rs     # Validation tools
├── resources/
│   ├── mod.rs
│   ├── wallet.rs         # Wallet resources
│   ├── network.rs        # Network resources
│   ├── pools.rs          # Pool resources
│   └── contracts.rs      # Contract resources
└── utils/
    ├── mod.rs
    ├── validation.rs     # Input validation utilities
    ├── caching.rs        # Caching utilities
    └── formatting.rs     # Response formatting utilities
```

### 2. Required Dependencies
Add to `Cargo.toml`:

```toml
[dependencies]
# Official Rust MCP SDK
rmcp = { git = "https://github.com/modelcontextprotocol/rust-sdk", branch = "main", features = ["server"] }

# Core dependencies
serde = { version = "1.0.219", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.45", features = ["full"] }

# Async/HTTP support (if needed)
reqwest = { version = "0.12", features = ["json"] }

# Logging and tracing
tracing = "0.1"
tracing-subscriber = "0.3"

# Validation and utilities
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"  # For error handling
```

### 3. Core Implementation Guidelines

#### MCP Server Structure (`src/mcp/server.rs`)
Create the main MCP server that:
- Uses the official `rmcp` crate with stdio transport
- Maintains state for SDK client, wallet, and network configuration
- Implements tool dispatch mechanism for all SDK operations
- Handles server lifecycle (startup, tool execution, shutdown)
- Provides error handling and logging

#### SDK Adapter Layer (`src/mcp/sdk_adapter.rs`)
Create an adapter that:
- Wraps the MantraDexClient for MCP compatibility
- Converts between JSON arguments and SDK types
- Handles all async SDK method calls
- Provides consistent error handling
- Maps SDK responses back to JSON format

### 4. Resource Implementations

#### Wallet Resources (`src/mcp/resources/wallet.rs`)
Implement functions for:
- Getting wallet information (address, public key)
- Retrieving wallet token balances
- Listing saved wallets from storage
- Wallet metadata and configuration

#### Pool Resources (`src/mcp/resources/pools.rs`)
Implement functions for:
- Listing all available pools with optional limits
- Getting detailed pool information by ID
- Pool status and availability checking
- Pool feature states (swaps, deposits, withdrawals)

### 5. Tool Implementations

#### Wallet Tools (`src/mcp/tools/wallet.rs`)
Implement tools for:
- Generating new wallets with mnemonic phrases
- Importing wallets from existing mnemonics
- Getting wallet information and balances
- Wallet management operations (save, load, list)

#### Trading Tools (`src/mcp/tools/trading.rs`)
Implement tools for:
- Simulating swaps to preview outcomes
- Executing single-hop token swaps
- Multi-hop swap operations
- Liquidity provision and withdrawal
- Transaction validation and execution

### 6. Error Handling (`src/mcp/error.rs`)
Create comprehensive error types that:
- Map SDK errors to MCP-compatible errors
- Handle validation errors for input parameters
- Provide serialization/deserialization error handling
- Include resource and tool not found errors
- Support permission and authentication errors
- Use thiserror for clean error definitions

### 7. Input Validation (`src/mcp/utils/validation.rs`)
Create validation functions for:
- Parsing coin amounts and denominations from JSON
- Validating pool IDs and addresses
- Checking slippage tolerances and percentages
- Validating mnemonic phrases
- Parameter range checking and format validation
- Type conversion with proper error handling

## Security Requirements

### 1. Private Key Safety
- **NEVER** include private keys in any MCP responses
- **NEVER** log private keys or mnemonics
- Always use wallet info/public data for responses
- Implement secure wallet storage patterns

### 2. Input Validation
Implement comprehensive validation for:
- All tool arguments (pool IDs, coin amounts, addresses)
- Parameter ranges and format checking
- Asset denominations and amounts
- Slippage tolerances and percentages
- Required vs optional fields

### 3. Rate Limiting
Implement rate limiting to:
- Track requests per client/session
- Enforce reasonable request limits
- Prevent abuse and DoS attacks
- Use time-window based limiting
- Provide clear error messages when limits exceeded

## Complete Tool & Resource Mapping

### Resources (Read-Only Data)
Map these URI patterns to SDK functions:
- `wallet://` resources - wallet info, balance, saved wallets
- `network://` resources - network config, status, block height
- `contracts://` resources - contract addresses and configurations
- `pools://` resources - pool lists, details, and status
- `rewards://` resources - pending rewards and history
- `epochs://` resources - epoch information and timing

### Tools (Actions)
Map these tool names to SDK operations:
- **Wallet Management**: generate_wallet, import_wallet, wallet operations
- **Network Operations**: switch_network, get_block_height, network status
- **Pool Operations**: get_pool, get_pools, pool validation, pool creation
- **Trading Operations**: simulate_swap, execute_swap, multihop swaps, liquidity
- **Rewards Operations**: query_rewards, claim_rewards, epoch management
- **Admin Operations**: pool feature management, enable/disable operations
- **Validation Operations**: fee validation, parameter checking

## Testing Strategy

### 1. Unit Tests
Create comprehensive unit tests for each tool and resource:
- Test each wallet operation (generate, import, info)
- Test swap operations (simulate, execute)
- Test pool operations (get, list, validate)
- Test rewards operations (query, claim)
- Test error handling for invalid inputs
- Mock blockchain interactions for consistent testing

### 2. Integration Tests
Test with actual MCP clients:
- Test MCP server startup and initialization
- Test resource access through MCP protocol
- Test tool execution via MCP client
- Test error propagation through MCP protocol
- Test with real blockchain connections (testnet)

## Implementation Priority

### Phase 1 (Critical - Week 1)
1. Set up basic MCP server framework
2. Implement core wallet operations (generate, import, info)
3. Add basic pool query operations
4. Create error handling system

### Phase 2 (High Priority - Week 2)  
1. Implement swap simulation and execution
2. Add liquidity operations
3. Create network configuration resources
4. Add input validation

### Phase 3 (Medium Priority - Week 3)
1. Implement rewards operations
2. Add admin pool management tools
3. Create caching layer
4. Add comprehensive testing

### Phase 4 (Enhancement - Week 4)
1. Performance optimization
2. Advanced analytics
3. Multi-hop swap optimization
4. Documentation and examples

## Completion Criteria

✅ **Functional Requirements:**
- All SDK public methods accessible via MCP tools
- All major data structures available as resources
- Proper error handling and validation
- Security best practices implemented

✅ **Quality Requirements:**
- >95% test coverage
- Response times <5 seconds
- Comprehensive input validation  
- No private key exposure

✅ **Integration Requirements:**
- Compatible with Claude MCP client
- Works with both mainnet and testnet
- Handles concurrent requests properly
- Supports all MCP protocol features

## Final Notes

- Follow the existing project's coding style and patterns
- Use the same error handling patterns as the main SDK
- Implement comprehensive logging for debugging
- Create detailed documentation for each tool and resource
- Test thoroughly with real blockchain connections
- Ensure all responses are properly formatted JSON

This implementation will create a production-ready MCP server that fully exposes the Mantra DEX SDK functionality to AI agents and other MCP clients, enabling sophisticated blockchain interactions through a standardized protocol.

Start with Phase 1 and work systematically through each component. Focus on security, reliability, and comprehensive error handling throughout the implementation.

**Remember**: Study the MCP documentation at https://modelcontextprotocol.io/tutorials/building-mcp-with-llms before beginning implementation to ensure proper protocol compliance. 