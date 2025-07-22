use cosmwasm_std::{Coin, Decimal, Uint128};
use serde::{Deserialize, Serialize};

/// Skip Adapter types and message structures
/// Based on Skip Go CosmWasm contracts for cross-chain operations

/// Skip swap operation for routing through adapters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSwapOperation {
    /// Pool identifier for the swap
    pub pool: String,
    /// Input token denomination
    pub denom_in: String,
    /// Output token denomination
    pub denom_out: String,
    /// Interface type (optional, defaults to None for standard swaps)
    pub interface: Option<String>,
}

/// Route for Skip smart swaps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipRoute {
    /// Asset to offer for this route
    pub offer_asset: SkipAsset,
    /// Operations to perform for this route
    pub operations: Vec<SkipSwapOperation>,
}

/// Asset representation for Skip operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipAsset {
    /// Native cosmos coin
    Native(Coin),
    /// CW20 token (not supported yet)
    Cw20(Cw20Coin),
}

impl SkipAsset {
    /// Create a new native asset
    pub fn native(denom: &str, amount: impl Into<Uint128>) -> Self {
        Self::Native(Coin {
            denom: denom.to_string(),
            amount: amount.into(),
        })
    }

    /// Get the denomination of the asset
    pub fn denom(&self) -> String {
        match self {
            SkipAsset::Native(coin) => coin.denom.clone(),
            SkipAsset::Cw20(coin) => coin.address.clone(),
        }
    }

    /// Get the amount of the asset
    pub fn amount(&self) -> Uint128 {
        match self {
            SkipAsset::Native(coin) => coin.amount,
            SkipAsset::Cw20(coin) => coin.amount,
        }
    }
}

/// CW20 token representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cw20Coin {
    /// Contract address
    pub address: String,
    /// Amount
    pub amount: Uint128,
}

/// Skip entry point execute messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipEntryPointExecuteMsg {
    /// User swap through Skip protocol
    UserSwap {
        /// The swap to execute
        swap: SkipSwap,
        /// Minimum asset to receive
        min_asset: SkipAsset,
        /// Remaining asset info
        remaining_asset: SkipAsset,
        /// Affiliate addresses for fee sharing
        affiliates: Vec<SkipAffiliate>,
    },
    /// Swap and action combined
    SwapAndAction {
        /// The swap to execute
        swap: SkipSwap,
        /// Minimum asset to receive
        min_asset: SkipAsset,
        /// Remaining asset info  
        remaining_asset: SkipAsset,
        /// Action to execute after swap
        post_swap_action: SkipAction,
        /// Affiliate addresses for fee sharing
        affiliates: Vec<SkipAffiliate>,
    },
}

/// Skip swap types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipSwap {
    /// Swap exact amount in
    SwapExactAssetIn(SkipSwapExactAssetIn),
    /// Swap exact amount out
    SwapExactAssetOut(SkipSwapExactAssetOut),
}

/// Skip swap exact asset in
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSwapExactAssetIn {
    /// Swap venue name (e.g., "mantra-dex")
    pub swap_venue_name: String,
    /// Operations to perform
    pub operations: Vec<SkipSwapOperation>,
}

/// Skip swap exact asset out
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipSwapExactAssetOut {
    /// Swap venue name (e.g., "mantra-dex")
    pub swap_venue_name: String,
    /// Operations to perform
    pub operations: Vec<SkipSwapOperation>,
}

/// Skip affiliate for fee sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipAffiliate {
    /// Affiliate address
    pub address: String,
    /// Basis points (0-10000, where 10000 = 100%)
    pub basis_points_fee: String,
}

/// Skip action for post-swap execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipAction {
    /// Transfer action
    Transfer {
        /// Recipient address
        to_address: String,
    },
    /// IBC transfer action
    IbcTransfer {
        /// IBC info
        ibc_info: SkipIbcInfo,
        /// Fee swap (optional)
        fee_swap: Option<SkipFeeSwap>,
    },
}

/// Skip IBC info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipIbcInfo {
    /// Source channel
    pub source_channel: String,
    /// Receiver address
    pub receiver: String,
    /// Memo (optional)
    pub memo: Option<String>,
    /// Recover address (optional)
    pub recover_address: Option<String>,
}

/// Skip fee swap info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipFeeSwap {
    /// Fee swap venue name
    pub swap_venue_name: String,
    /// Operations for fee swap
    pub operations: Vec<SkipSwapOperation>,
}

/// Skip entry point query messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipEntryPointQueryMsg {
    /// Simulate a swap exact asset in
    SimulateSwapExactAssetIn {
        /// Asset to swap in
        asset_in: SkipAsset,
        /// Swap operations to perform
        swap_operations: Vec<SkipSwapOperation>,
    },
    /// Simulate a swap exact asset out
    SimulateSwapExactAssetOut {
        /// Asset to get out
        asset_out: SkipAsset,
        /// Swap operations to perform
        swap_operations: Vec<SkipSwapOperation>,
    },
    /// Simulate a smart swap exact asset in
    SimulateSmartSwapExactAssetIn {
        /// Asset to swap in
        asset_in: SkipAsset,
        /// Routes to consider
        routes: Vec<SkipRoute>,
    },
}

/// Simulate swap exact asset in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateSwapExactAssetInResponse {
    /// The asset out from the simulation
    pub asset_out: SkipAsset,
    /// Optional spot price
    pub spot_price: Option<Decimal>,
}

/// Simulate swap exact asset out response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateSwapExactAssetOutResponse {
    /// The asset in needed for the simulation
    pub asset_in: SkipAsset,
    /// Optional spot price
    pub spot_price: Option<Decimal>,
}

/// Simulate smart swap exact asset in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateSmartSwapExactAssetInResponse {
    /// The asset out from the simulation
    pub asset_out: SkipAsset,
    /// Optional spot price
    pub spot_price: Option<Decimal>,
}

/// Skip adapter instantiate message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipAdapterInstantiateMsg {
    /// Entry point contract address
    pub entry_point_contract_address: String,
    /// Mantra pool manager address
    pub mantra_pool_manager_address: String,
}