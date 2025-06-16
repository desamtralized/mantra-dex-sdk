# Mantra DEX SDK Model Context Protocol (MCP) Server - Product Requirements Document

## Overview

This document outlines the requirements for building a Model Context Protocol (MCP) server that exposes the complete functionality of the Mantra DEX SDK, enabling AI agents and other clients to interact with the Mantra blockchain DEX through a standardized protocol.

## Product Vision

Create a comprehensive MCP server that provides programmatic access to all Mantra DEX operations, allowing AI agents to:
- Manage wallets and execute blockchain transactions
- Perform DEX operations (swap, liquidity, rewards)
- Query blockchain state and pool information
- Administer pool configurations and features

## Target Users

- **AI Agents/LLMs**: Primary consumers requiring blockchain interaction capabilities
- **Developers**: Building applications that need Mantra DEX integration
- **Automated Trading Systems**: Requiring programmatic DEX access
- **DeFi Tools**: Portfolio management and analytics platforms

## Core Requirements

### 1. Wallet Management Resources

#### 1.1 Wallet Operations
- **Resource**: `wallet://create` - Generate new HD wallets with mnemonic phrases
- **Resource**: `wallet://import` - Import wallets from mnemonic phrases
- **Resource**: `wallet://info` - Get wallet address and public key information
- **Resource**: `wallet://balance` - Query wallet token balances

#### 1.2 Wallet Storage
- **Resource**: `wallet://save` - Persist wallet data securely
- **Resource**: `wallet://load` - Load saved wallet configurations
- **Resource**: `wallet://list` - List available saved wallets

### 2. Network Configuration Resources

#### 2.1 Network Management
- **Resource**: `network://config` - Current network configuration
- **Resource**: `network://switch` - Switch between networks (mainnet/testnet)
- **Resource**: `network://status` - Network health and block height

#### 2.2 Contract Information
- **Resource**: `contracts://addresses` - Contract addresses for current network
- **Resource**: `contracts://info` - Contract metadata and versions

### 3. Pool Management Tools

#### 3.1 Pool Query Tools
- **Tool**: `get_pool` - Retrieve specific pool information by ID
- **Tool**: `get_pools` - List all available pools with optional limits
- **Tool**: `validate_pool_status` - Check if pool is available for operations
- **Tool**: `get_pool_status` - Get detailed pool status information

#### 3.2 Pool Administration Tools (Admin)
- **Tool**: `create_pool` - Create new liquidity pools
- **Tool**: `update_pool_features` - Enable/disable pool features
- **Tool**: `enable_pool_operations` - Enable all pool operations
- **Tool**: `disable_pool_operations` - Disable all pool operations
- **Tool**: `update_global_features` - Update global pool settings

### 4. Trading Tools

#### 4.1 Swap Operations
- **Tool**: `simulate_swap` - Preview swap outcomes without execution
- **Tool**: `execute_swap` - Perform single-hop token swaps
- **Tool**: `execute_multihop_swap` - Execute complex multi-hop swaps

#### 4.2 Liquidity Operations
- **Tool**: `provide_liquidity` - Add liquidity to pools
- **Tool**: `provide_liquidity_unchecked` - Add liquidity without validation
- **Tool**: `withdraw_liquidity` - Remove liquidity from pools

### 5. Rewards Management Tools

#### 5.1 Rewards Query Tools
- **Tool**: `query_rewards` - Get pending rewards for address
- **Tool**: `query_all_rewards` - Get all rewards for address
- **Tool**: `query_rewards_until_epoch` - Get rewards up to specific epoch

#### 5.2 Rewards Operations
- **Tool**: `claim_rewards` - Claim rewards with optional epoch limit
- **Tool**: `claim_all_rewards` - Claim all available rewards
- **Tool**: `get_current_epoch` - Get current reward epoch

### 6. Validation and Fee Tools

#### 6.1 Validation Tools
- **Tool**: `validate_pool_fees` - Validate pool fee structures
- **Tool**: `validate_epoch` - Validate epoch numbers
- **Tool**: `create_validated_pool_fees` - Create validated fee objects

#### 6.2 Fee Management Tools  
- **Tool**: `create_fee` - Create transaction fee objects
- **Tool**: `create_default_fee` - Create fees using network defaults
- **Tool**: `estimate_gas` - Estimate gas requirements for operations

### 7. Blockchain Query Tools

#### 7.1 General Queries
- **Tool**: `get_block_height` - Get latest block height
- **Tool**: `query_contract` - Execute smart contract queries
- **Tool**: `get_account_info` - Get account details and sequence

## Technical Requirements

### 1. Protocol Compliance
- **MCP Version**: Implement MCP specification v1.0+
- **Transport**: Support both stdio and HTTP transports
- **Serialization**: JSON-RPC 2.0 for all communications
- **Error Handling**: Comprehensive error responses with proper codes

### 2. Security Requirements
- **Private Key Security**: Never expose private keys in responses
- **Input Validation**: Validate all inputs before blockchain operations
- **Rate Limiting**: Implement reasonable request rate limits
- **Access Control**: Support authentication/authorization mechanisms

### 3. Performance Requirements
- **Response Time**: < 5 seconds for blockchain queries
- **Concurrency**: Support multiple concurrent requests
- **Caching**: Cache frequently accessed data (pools, network info)
- **Resource Management**: Efficient memory and connection management

### 4. Integration Requirements
- **SDK Integration**: Use existing MantraDexClient and MantraWallet
- **Async Operations**: Support async/await patterns throughout
- **Error Propagation**: Map SDK errors to MCP error responses
- **Configuration**: Support environment-based configuration

## Data Models

### 1. Resource Schemas
```json
{
  "wallet": {
    "address": "string",
    "public_key": "string", 
    "balance": [{"denom": "string", "amount": "string"}]
  },
  "pool": {
    "pool_id": "string",
    "assets": ["string"],
    "status": "Available|Disabled",
    "features": {
      "swaps_enabled": "boolean",
      "deposits_enabled": "boolean", 
      "withdrawals_enabled": "boolean"
    }
  }
}
```

### 2. Tool Argument Schemas
```json
{
  "swap_args": {
    "pool_id": "string",
    "offer_asset": {"denom": "string", "amount": "string"},
    "ask_asset_denom": "string",
    "max_slippage": "optional<decimal>"
  },
  "liquidity_args": {
    "pool_id": "string", 
    "assets": [{"denom": "string", "amount": "string"}],
    "max_slippage": "optional<decimal>"
  }
}
```

## Success Metrics

### 1. Functional Metrics
- **API Coverage**: Core SDK methods exposed (wallet, pools, swaps, rewards)
- **Tool Success Rate**: >90% successful tool executions for MVP
- **Resource Availability**: Basic resource queries working

### 2. Performance Metrics
- **Average Response Time**: <10 seconds for blockchain operations
- **Basic Functionality**: Handle single-user operations reliably
- **Error Rate**: <10% of requests result in errors for MVP

### 3. Integration Metrics
- **Client Compatibility**: Works with Claude MCP client
- **SDK Version Support**: Compatible with current mantra-dex-std v3.0.0
- **Network Support**: Testnet functionality (mainnet optional for MVP)

## Implementation Phases (MVP Focus)

### Phase 1: Core Infrastructure (Week 1)
- MCP server framework setup
- Basic resource and tool registration
- SDK integration layer
- Configuration management

### Phase 2: Wallet Operations (Week 2) 
- Wallet management resources
- Basic wallet operations (generate, import, info, balance)
- Basic security implementation

### Phase 3: Pool & Trading Operations (Week 3)
- Pool query tools
- Basic swap operations (simulate, execute)
- Pool status validation

### Phase 4: Testing & Documentation (Week 4)
- Unit testing for core functionality
- Basic integration testing
- Essential documentation
- Claude MCP client compatibility testing

## Risk Mitigation

### 1. Technical Risks
- **Blockchain Connectivity**: Implement retry logic and fallback RPC endpoints
- **Transaction Failures**: Comprehensive error handling with clear messages
- **State Synchronization**: Cache invalidation strategies

### 2. Security Risks
- **Private Key Exposure**: Strict separation of wallet operations
- **Input Validation**: Comprehensive sanitization of all inputs
- **Access Control**: Authentication and authorization layers

### 3. Integration Risks
- **SDK Version Changes**: Version compatibility checking
- **MCP Specification Changes**: Modular design for easy updates
- **Client Compatibility**: Testing with multiple MCP clients

## Dependencies

### 1. External Dependencies
- **mantra-dex-std**: v3.0.0+ (DEX standard library)
- **mcp-sdk**: Latest MCP SDK for Rust
- **tokio**: Async runtime
- **serde**: Serialization support

### 2. Internal Dependencies
- **MantraDexClient**: Core SDK client
- **MantraWallet**: Wallet operations
- **MantraNetworkConfig**: Network configuration
- **Error types**: Comprehensive error handling

## Acceptance Criteria

### 1. Functional Acceptance
- [ ] All SDK public methods exposed through MCP tools/resources
- [ ] Wallet operations work end-to-end
- [ ] Pool operations execute successfully
- [ ] Rewards can be queried and claimed
- [ ] Multi-hop swaps function correctly

### 2. Quality Acceptance
- [ ] Comprehensive error handling with meaningful messages
- [ ] Input validation prevents invalid operations
- [ ] Performance meets specified metrics
- [ ] Security requirements fully implemented

### 3. Integration Acceptance
- [ ] Compatible with Claude and other MCP clients
- [ ] Works with both mainnet and testnet
- [ ] Handles network switching correctly
- [ ] Supports concurrent operations

This PRD serves as the definitive guide for implementing the Mantra DEX SDK MCP Server, ensuring all stakeholder requirements are met while maintaining high standards for security, performance, and usability. 