use std::str::FromStr;
use std::sync::Arc;

use base64::{engine::general_purpose, Engine};
use cosmos_sdk_proto::{
    cosmos::auth::v1beta1::{BaseAccount, QueryAccountRequest, QueryAccountResponse},
    cosmos::bank::v1beta1::{QueryAllBalancesRequest, QueryAllBalancesResponse},
    cosmwasm::wasm::v1::QuerySmartContractStateResponse,
};
use cosmrs::{
    proto::{
        cosmos::base::{abci::v1beta1::TxResponse, v1beta1::Coin as CosmosCoin},
        cosmwasm::wasm::v1::{MsgExecuteContract, QuerySmartContractStateRequest},
    },
    rpc::{Client as RpcClient, HttpClient},
    tendermint::{chain::Id, Hash},
    tx::{Body, MessageExt, SignDoc, SignerInfo},
    Any,
};
use cosmwasm_std::{Coin, Decimal, Uint128};
use mantra_dex_std::pool_manager::{
    self, PoolInfoResponse, PoolsResponse, SimulationResponse, SwapOperation,
};
use prost::Message;
use serde::de::DeserializeOwned;
use tokio::sync::Mutex;

use crate::config::MantraNetworkConfig;
use crate::error::Error;
use crate::wallet::MantraWallet;

/// Pool status enum for validation
#[derive(Debug, Clone, PartialEq)]
pub enum PoolStatus {
    /// Pool is available for all operations (swaps, deposits, withdrawals)
    Available,
    /// Pool is disabled and cannot be used for operations
    Disabled,
}

impl PoolStatus {
    /// Check if the pool is available for operations
    ///
    /// # Returns
    ///
    /// `true` if the pool status is `Available`, `false` otherwise
    pub fn is_available(&self) -> bool {
        matches!(self, PoolStatus::Available)
    }
}

/// Mantra DEX client for interacting with the network
///
/// This client provides methods to interact with the Mantra DEX v3.0.0,
/// including pool operations, swapping, liquidity provision, and rewards management.
pub struct MantraDexClient {
    /// RPC client for the Mantra network
    rpc_client: Arc<Mutex<HttpClient>>,
    /// Network configuration
    config: MantraNetworkConfig,
    /// Wallet for signing transactions
    wallet: Option<MantraWallet>,
}

impl MantraDexClient {
    /// Create a new client with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Network configuration containing RPC endpoints and contract addresses
    ///
    /// # Returns
    ///
    /// A new `MantraDexClient` instance ready for use
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC client cannot be created
    pub async fn new(config: MantraNetworkConfig) -> Result<Self, Error> {
        let rpc_client = HttpClient::new(config.rpc_url.as_str())
            .map_err(|e| Error::Rpc(format!("Failed to create RPC client: {}", e)))?;

        Ok(Self {
            rpc_client: Arc::new(Mutex::new(rpc_client)),
            config,
            wallet: None,
        })
    }

    /// Set the wallet for signing transactions
    ///
    /// # Arguments
    ///
    /// * `wallet` - The wallet to use for signing transactions
    ///
    /// # Returns
    ///
    /// The client instance with the wallet configured
    pub fn with_wallet(mut self, wallet: MantraWallet) -> Self {
        self.wallet = Some(wallet);
        self
    }

    /// Get the wallet if available
    pub fn wallet(&self) -> Result<&MantraWallet, Error> {
        self.wallet
            .as_ref()
            .ok_or_else(|| Error::Wallet("No wallet configured".to_string()))
    }

    /// Get last block height
    pub async fn get_last_block_height(&self) -> Result<u64, Error> {
        let rpc_client = self.rpc_client.lock().await;
        let height = rpc_client
            .latest_block()
            .await
            .map_err(|e| Error::Rpc(format!("Failed to get last block height: {}", e)))?;
        Ok(height.block.header.height.value() as u64)
    }

    /// Get the Wallet balances
    pub async fn get_balances(&self) -> Result<Vec<Coin>, Error> {
        let wallet = self.wallet()?;
        let address = wallet.address().unwrap().to_string();
        let rpc_client = self.rpc_client.lock().await;

        // Create a request to get all balances
        let request = QueryAllBalancesRequest {
            address,
            pagination: None,
            resolve_denom: false,
        };

        // Encode the request to protobuf
        let encoded_request = request.encode_to_vec();

        // Execute the query
        let response = rpc_client
            .abci_query(
                Some("/cosmos.bank.v1beta1.Query/AllBalances".to_string()),
                encoded_request,
                None,
                false,
            )
            .await
            .map_err(|e| Error::Rpc(format!("Failed to get balances: {}", e)))?;

        if !response.code.is_ok() {
            return Err(Error::Rpc(format!("Query failed: {}", response.log)));
        }

        // Decode the response
        let balances_response = QueryAllBalancesResponse::decode(response.value.as_slice())
            .map_err(|e| Error::Rpc(format!("Failed to decode balances response: {}", e)))?;

        // Convert from cosmos proto coins to cosmwasm coins
        let balances = balances_response
            .balances
            .into_iter()
            .map(|coin| Coin {
                denom: coin.denom,
                amount: Uint128::from_str(&coin.amount).unwrap_or_default(),
            })
            .collect();

        Ok(balances)
    }

    /// Get the network configuration
    pub fn config(&self) -> &MantraNetworkConfig {
        &self.config
    }

    /// Query a smart contract
    pub async fn query<Q: serde::Serialize + Clone, R: DeserializeOwned>(
        &self,
        contract_addr: &str,
        query_msg: &Q,
    ) -> Result<R, Error> {
        let rpc_client = self.rpc_client.lock().await;
        let query = QuerySmartContractStateRequest {
            address: contract_addr.to_string(),
            query_data: serde_json::to_vec(query_msg)?,
        };

        // Now that we're using the same Prost version as cosmos-sdk-proto,
        // we can use the Message trait directly
        let data = query.encode_to_vec();
        let result = rpc_client
            .abci_query(
                Some("/cosmwasm.wasm.v1.Query/SmartContractState".to_string()),
                data,
                None,
                false,
            )
            .await
            .map_err(|e| Error::Rpc(format!("ABCI query failed: {}", e)))?;

        if !result.code.is_ok() {
            return Err(Error::Contract(format!(
                "Contract query failed: {}",
                String::from_utf8_lossy(result.log.as_bytes())
            )));
        }
        let resp: QuerySmartContractStateResponse =
            QuerySmartContractStateResponse::decode(result.value.as_slice())
                .map_err(|e| Error::Rpc(format!("Failed to decode query response: {}", e)))?;
        serde_json::from_slice::<R>(resp.data.as_slice()).map_err(Into::into)
    }

    /// Execute a contract message
    pub async fn execute<T: serde::Serialize>(
        &self,
        contract_addr: &str,
        msg: &T,
        funds: Vec<Coin>,
    ) -> Result<TxResponse, Error> {
        let wallet = self.wallet()?;
        let sender = wallet.address().unwrap().to_string();

        let cosmos_coins = funds
            .iter()
            .map(|c| CosmosCoin {
                denom: c.denom.clone(),
                amount: c.amount.to_string(),
            })
            .collect();
        let execute_msg = MsgExecuteContract {
            sender: sender.clone(),
            contract: contract_addr.to_string(),
            msg: serde_json::to_vec(msg)?,
            funds: cosmos_coins,
        };
        println!("Execute message: {:?}", execute_msg);

        self.broadcast_tx(vec![Any {
            type_url: "/cosmwasm.wasm.v1.MsgExecuteContract".to_string(),
            value: execute_msg.to_bytes().unwrap(),
        }])
        .await
    }

    /// Broadcast a transaction to the network
    async fn broadcast_tx(&self, msgs: Vec<Any>) -> Result<TxResponse, Error> {
        let _height = self.get_last_block_height().await?;
        let wallet = self.wallet()?;
        let rpc_client = self.rpc_client.lock().await;

        let tx_body = Body::new(msgs, String::new(), 0u32);

        // Get account info for signing
        let addr = wallet.address().unwrap().to_string();

        // Create request using the proper protobuf type
        let request = QueryAccountRequest { address: addr };

        // Encode the request to protobuf
        let encoded_request = request.encode_to_vec();

        let account_info = rpc_client
            .abci_query(
                Some("/cosmos.auth.v1beta1.Query/Account".to_string()),
                encoded_request,
                None,
                false,
            )
            .await
            .map_err(|e| Error::Rpc(format!("Failed to get account info: {}", e)))?;

        if !account_info.code.is_ok() {
            return Err(Error::Rpc(format!(
                "Account query failed: {}",
                account_info.log
            )));
        }

        // Decode the response using the correct protobuf type
        let account_response = QueryAccountResponse::decode(account_info.value.as_slice())
            .map_err(|e| Error::Rpc(format!("Failed to decode account response: {}", e)))?;

        // Extract the account data - account.value contains a serialized BaseAccount
        let account_any = account_response.account.unwrap();

        // Decode the BaseAccount from the Any object's value
        let base_account = BaseAccount::decode(account_any.value.as_slice())
            .map_err(|e| Error::Rpc(format!("Failed to decode BaseAccount: {}", e)))?;

        let account_number = base_account.account_number;
        let sequence = base_account.sequence;
        // Create the fee
        let fee = wallet.create_default_fee(2_000_000)?;

        // Create signer info with sequence number
        let signer_info = SignerInfo::single_direct(Some(wallet.public_key()), sequence);

        // Create auth info with fee
        let auth_info = signer_info.auth_info(fee);

        let chain_id = Id::try_from(self.config.network_id.as_str())
            .map_err(|e| Error::Tx(format!("Invalid chain ID: {}", e)))?;

        let sign_doc = SignDoc::new(&tx_body, &auth_info, &chain_id, account_number)
            .map_err(|e| Error::Tx(format!("Failed to create sign doc: {}", e)))?;

        // Sign the transaction
        let tx_raw = sign_doc
            .sign(wallet.signing_key())
            .map_err(|e| Error::Tx(format!("Failed to sign transaction: {}", e)))?;
        // Broadcast the transaction
        let response = rpc_client
            .broadcast_tx_commit(tx_raw.to_bytes().unwrap())
            .await
            .map_err(|e| Error::Rpc(format!("Failed to broadcast transaction: {}", e)))?;
        // Get the transaction response
        let tx_response = if response.check_tx.code.is_err() {
            return Err(Error::Contract(format!(
                "Transaction check failed: {}",
                response.check_tx.log
            )));
        } else if response.tx_result.code.is_err() {
            return Err(Error::Contract(format!(
                "Transaction execution failed: {}",
                response.tx_result.log
            )));
        } else {
            // Query the tx
            let tx_result = rpc_client
                .tx(
                    Hash::try_from(response.hash.as_bytes().to_vec())
                        .map_err(|e| Error::Tx(format!("Invalid tx hash: {}", e)))?,
                    false,
                )
                .await
                .map_err(|e| Error::Rpc(format!("Failed to get transaction: {}", e)))?;

            // Transform the response to TxResponse
            TxResponse {
                height: tx_result.height.value() as i64,
                txhash: hex::encode(response.hash.as_bytes()),
                codespace: "".to_string(),
                code: 0,
                data: general_purpose::STANDARD.encode(tx_result.tx_result.data),
                raw_log: tx_result.tx_result.log.to_string(),
                logs: vec![],
                info: "".to_string(),
                gas_wanted: tx_result.tx_result.gas_wanted,
                gas_used: tx_result.tx_result.gas_used,
                tx: None,
                timestamp: "".to_string(),
                events: vec![],
            }
        };

        Ok(tx_response)
    }

    /// Get pool information by ID
    pub async fn get_pool(&self, pool_id: &str) -> Result<PoolInfoResponse, Error> {
        let query = pool_manager::QueryMsg::Pools {
            pool_identifier: Some(pool_id.to_string()),
            start_after: None,
            limit: None,
        };
        let pool_manager_address = self.config.contracts.pool_manager.clone();
        let response: PoolsResponse = self.query(&pool_manager_address, &query).await?;
        if response.pools.is_empty() {
            return Err(Error::Other(format!("Pool {} not found", pool_id)));
        }

        let pool = &response.pools[0];
        Ok(pool.clone())
    }

    /// Get list of pools
    pub async fn get_pools(&self, limit: Option<u32>) -> Result<Vec<PoolInfoResponse>, Error> {
        let query = pool_manager::QueryMsg::Pools {
            pool_identifier: None,
            start_after: None,
            limit,
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        let response: PoolsResponse = self.query(&pool_manager_address, &query).await?;

        Ok(response.pools)
    }

    /// Extract pool status from PoolInfoResponse
    pub fn get_pool_status(&self, pool: &PoolInfoResponse) -> PoolStatus {
        // Map the actual status from pool.pool_info.status to our PoolStatus enum
        // The status field in the mantra_dex_std::pool_manager::PoolInfo structure
        // contains information about the pool's operational state
        let status = &pool.pool_info.status;

        // If all operations are enabled, the pool is Available
        // In v3.0.0, we check if swaps, deposits, and withdrawals are all enabled
        if status.swaps_enabled && status.deposits_enabled && status.withdrawals_enabled {
            PoolStatus::Available
        } else {
            // If any operation is disabled, the pool is considered Disabled
            PoolStatus::Disabled
        }
    }

    /// Validate that a pool is available for operations
    pub async fn validate_pool_status(&self, pool_id: &str) -> Result<(), Error> {
        let pool = self.get_pool(pool_id).await?;
        let status = self.get_pool_status(&pool);

        if !status.is_available() {
            return Err(Error::Other(format!(
                "Pool {} is not available for operations (status: {:?})",
                pool_id, status
            )));
        }

        Ok(())
    }

    /// Simulate a swap to see the expected amount
    pub async fn simulate_swap(
        &self,
        pool_id: &str,
        offer_asset: Coin,
        ask_asset_denom: &str,
    ) -> Result<SimulationResponse, Error> {
        let query = pool_manager::QueryMsg::Simulation {
            pool_identifier: pool_id.to_string(),
            ask_asset_denom: ask_asset_denom.to_string(),
            offer_asset: offer_asset.clone(),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.query(&pool_manager_address, &query).await
    }

    /// Swap tokens
    /// Execute a swap operation on a pool
    ///
    /// **v3.0.0 Breaking Change**: The `max_spread` parameter has been renamed to `max_slippage`
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The identifier of the pool to swap in
    /// * `offer_asset` - The asset being offered for swap
    /// * `ask_asset_denom` - The denomination of the asset being requested
    /// * `max_slippage` - Optional maximum slippage tolerance (replaces `max_spread` from v2.x)
    ///
    /// # Returns
    ///
    /// Transaction response containing the swap result
    ///
    /// # Errors
    ///
    /// * Returns error if pool status validation fails (pool must be Available)
    /// * Returns error if the swap transaction fails
    /// * Returns error if no wallet is configured
    pub async fn swap(
        &self,
        pool_id: &str,
        offer_asset: Coin,
        ask_asset_denom: &str,
        max_slippage: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        // Validate pool status before executing swap
        self.validate_pool_status(pool_id).await?;

        let msg = pool_manager::ExecuteMsg::Swap {
            pool_identifier: pool_id.to_string(),
            belief_price: None,
            receiver: None,
            ask_asset_denom: ask_asset_denom.to_string(),
            max_slippage: max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
        };

        println!("Swap message: {:?}", msg);

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, vec![offer_asset])
            .await
    }

    /// Provide liquidity to a pool
    ///
    /// **v3.0.0 Breaking Changes**:
    /// - `slippage_tolerance` parameter renamed to `liquidity_max_slippage`
    /// - `max_spread` parameter renamed to `swap_max_slippage`
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The identifier of the pool to provide liquidity to
    /// * `assets` - Vector of assets to provide as liquidity
    /// * `liquidity_max_slippage` - Optional maximum slippage for liquidity provision (replaces `slippage_tolerance`)
    /// * `swap_max_slippage` - Optional maximum slippage for internal swaps (replaces `max_spread`)
    ///
    /// # Returns
    ///
    /// Transaction response containing the liquidity provision result
    ///
    /// # Errors
    ///
    /// * Returns error if pool status validation fails (pool must be Available)
    /// * Returns error if the liquidity provision transaction fails
    /// * Returns error if no wallet is configured
    pub async fn provide_liquidity(
        &self,
        pool_id: &str,
        assets: Vec<Coin>,
        liquidity_max_slippage: Option<Decimal>,
        swap_max_slippage: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        // Validate pool status before providing liquidity
        self.validate_pool_status(pool_id).await?;

        let msg = pool_manager::ExecuteMsg::ProvideLiquidity {
            pool_identifier: pool_id.to_string(),
            liquidity_max_slippage: liquidity_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            swap_max_slippage: swap_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            receiver: None,
            unlocking_duration: None,
            lock_position_identifier: None,
        };

        let mut coins: Vec<Coin> = assets
            .into_iter()
            .map(|a| Coin {
                denom: a.denom,
                amount: a.amount,
            })
            .collect();

        // Sort coins by denomination as required by Cosmos SDK
        coins.sort_by(|a, b| a.denom.cmp(&b.denom));

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, coins).await
    }

    /// Provide liquidity to a pool without status validation (for creating new pools)
    ///
    /// This method bypasses pool status validation and should only be used when creating new pools
    /// or in scenarios where pool status checking is not required.
    ///
    /// **v3.0.0 Breaking Changes**: Same parameter renames as `provide_liquidity`
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The identifier of the pool to provide liquidity to
    /// * `assets` - Vector of assets to provide as liquidity
    /// * `liquidity_max_slippage` - Optional maximum slippage for liquidity provision
    /// * `swap_max_slippage` - Optional maximum slippage for internal swaps
    ///
    /// # Returns
    ///
    /// Transaction response containing the liquidity provision result
    ///
    /// # Errors
    ///
    /// * Returns error if the liquidity provision transaction fails
    /// * Returns error if no wallet is configured
    pub async fn provide_liquidity_unchecked(
        &self,
        pool_id: &str,
        assets: Vec<Coin>,
        liquidity_max_slippage: Option<Decimal>,
        swap_max_slippage: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        let msg = pool_manager::ExecuteMsg::ProvideLiquidity {
            pool_identifier: pool_id.to_string(),
            liquidity_max_slippage: liquidity_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            swap_max_slippage: swap_max_slippage.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
            receiver: None,
            unlocking_duration: None,
            lock_position_identifier: None,
        };

        let mut coins: Vec<Coin> = assets
            .into_iter()
            .map(|a| Coin {
                denom: a.denom,
                amount: a.amount,
            })
            .collect();

        // Sort coins by denomination as required by Cosmos SDK
        coins.sort_by(|a, b| a.denom.cmp(&b.denom));

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, coins).await
    }

    /// Withdraw liquidity from a pool
    pub async fn withdraw_liquidity(
        &self,
        pool_id: &str,
        lp_amount: Uint128,
    ) -> Result<TxResponse, Error> {
        // Get pool info and validate status in one call
        let pool = self.get_pool(pool_id).await?;
        let status = self.get_pool_status(&pool);

        if !status.is_available() {
            return Err(Error::Other(format!(
                "Pool {} is not available for operations (status: {:?})",
                pool_id, status
            )));
        }

        let lp_denom = pool.pool_info.lp_denom;
        let msg = pool_manager::ExecuteMsg::WithdrawLiquidity {
            pool_identifier: pool_id.to_string(),
        };
        let funds = vec![Coin {
            denom: lp_denom,
            amount: lp_amount,
        }];
        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, funds).await
    }

    /// Create a new pool with the specified assets and configuration
    ///
    /// **v3.0.0 New Feature**: Enhanced fee validation ensures total fees ≤ 20%
    ///
    /// # Arguments
    ///
    /// * `asset_denoms` - Vector of asset denominations for the pool
    /// * `asset_decimals` - Vector of decimal places for each asset
    /// * `pool_fees` - Fee structure for the pool (validated against v3.0.0 requirements)
    /// * `pool_type` - Type of pool to create
    /// * `pool_identifier` - Optional custom identifier for the pool
    ///
    /// # Returns
    ///
    /// Transaction response containing the pool creation result
    ///
    /// # Errors
    ///
    /// * Returns `FeeValidation` error if pool fees exceed 20% total
    /// * Returns error if pool creation transaction fails
    /// * Returns error if no wallet is configured
    ///
    /// # Notes
    ///
    /// Pool creation requires a fee of 98 OM (98,000,000 uom)
    pub async fn create_pool(
        &self,
        asset_denoms: Vec<String>,
        asset_decimals: Vec<u8>,
        pool_fees: mantra_dex_std::fee::PoolFee,
        pool_type: mantra_dex_std::pool_manager::PoolType,
        pool_identifier: Option<String>,
    ) -> Result<TxResponse, Error> {
        // Validate pool fees before creating the pool (v3.0.0 requirement)
        self.validate_pool_fees(&pool_fees)?;

        let msg = pool_manager::ExecuteMsg::CreatePool {
            asset_denoms,
            asset_decimals,
            pool_fees,
            pool_type,
            pool_identifier,
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();

        // Pool creation requires a fee of 98 OM (98,000,000 uom)
        let pool_creation_fee = vec![Coin {
            denom: "uom".to_string(),
            amount: Uint128::new(98_000_000), // 98 OM
        }];

        self.execute(&pool_manager_address, &msg, pool_creation_fee)
            .await
    }

    /// Execute multiple swap operations
    pub async fn execute_swap_operations(
        &self,
        operations: Vec<SwapOperation>,
        amount: Uint128,
    ) -> Result<TxResponse, Error> {
        let first_op = operations
            .first()
            .ok_or_else(|| Error::Other("Swap operations list cannot be empty".to_string()))?;

        // Validate pool status for the first operation
        self.validate_pool_status(&first_op.get_pool_identifer())
            .await?;

        let msg = pool_manager::ExecuteMsg::Swap {
            ask_asset_denom: first_op.get_target_asset_info().clone(),
            belief_price: None,
            max_slippage: None,
            receiver: None,
            pool_identifier: first_op.get_pool_identifer().clone(),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(
            &pool_manager_address,
            &msg,
            vec![Coin {
                denom: first_op.get_input_asset_info().clone(),
                amount,
            }],
        )
        .await
    }

    // =========================
    // Farm Manager Functionality
    // =========================

    /// Claim rewards from farm manager with optional epoch parameter
    ///
    /// **v3.0.0 New Feature**: Enhanced claim functionality with epoch-based claiming
    ///
    /// # Arguments
    ///
    /// * `until_epoch` - Optional epoch limit for claiming rewards. If provided, only claims rewards up to that epoch
    ///
    /// # Returns
    ///
    /// Transaction response containing the claim result
    ///
    /// # Errors
    ///
    /// * Returns error if farm manager contract is not configured
    /// * Returns error if the claim transaction fails
    /// * Returns error if no wallet is configured
    ///
    /// # Backward Compatibility
    ///
    /// When `until_epoch` is `None`, behaves like the v2.x parameterless claim
    pub async fn claim_rewards(&self, until_epoch: Option<u64>) -> Result<TxResponse, Error> {
        let farm_manager_address =
            self.config.contracts.farm_manager.as_ref().ok_or_else(|| {
                Error::Other("Farm manager contract address not configured".to_string())
            })?;

        let msg = if let Some(epoch) = until_epoch {
            serde_json::json!({
                "claim": {
                    "until_epoch": epoch
                }
            })
        } else {
            // Backward compatibility: parameterless claim
            serde_json::json!({
                "claim": {}
            })
        };

        self.execute(farm_manager_address, &msg, vec![]).await
    }

    /// Claim rewards without epoch parameter (backward compatibility)
    ///
    /// This is a convenience method that calls `claim_rewards(None)` for backward compatibility
    /// with v2.x code that used parameterless claim methods.
    ///
    /// # Returns
    ///
    /// Transaction response containing the claim result
    pub async fn claim_rewards_all(&self) -> Result<TxResponse, Error> {
        self.claim_rewards(None).await
    }

    /// Claim rewards up to a specific epoch
    ///
    /// This is a convenience method that calls `claim_rewards(Some(until_epoch))`.
    ///
    /// # Arguments
    ///
    /// * `until_epoch` - The epoch limit for claiming rewards
    ///
    /// # Returns
    ///
    /// Transaction response containing the claim result
    pub async fn claim_rewards_until_epoch(&self, until_epoch: u64) -> Result<TxResponse, Error> {
        self.claim_rewards(Some(until_epoch)).await
    }

    /// Query rewards for an address with optional epoch parameter
    ///
    /// **v3.0.0 New Feature**: Enhanced rewards query with epoch range support
    ///
    /// # Arguments
    ///
    /// * `address` - The address to query rewards for
    /// * `until_epoch` - Optional epoch limit for querying rewards. If provided, only returns rewards up to that epoch
    ///
    /// # Returns
    ///
    /// JSON value containing the rewards information
    ///
    /// # Errors
    ///
    /// * Returns error if farm manager contract is not configured
    /// * Returns error if the query fails
    ///
    /// # Backward Compatibility
    ///
    /// When `until_epoch` is `None`, behaves like the v2.x parameterless rewards query
    pub async fn query_rewards(
        &self,
        address: &str,
        until_epoch: Option<u64>,
    ) -> Result<serde_json::Value, Error> {
        let farm_manager_address =
            self.config.contracts.farm_manager.as_ref().ok_or_else(|| {
                Error::Other("Farm manager contract address not configured".to_string())
            })?;

        let query = if let Some(epoch) = until_epoch {
            serde_json::json!({
                "rewards": {
                    "address": address,
                    "until_epoch": epoch
                }
            })
        } else {
            serde_json::json!({
                "rewards": {
                    "address": address
                }
            })
        };

        self.query(farm_manager_address, &query).await
    }

    /// Query all rewards for an address (backward compatibility)
    pub async fn query_all_rewards(&self, address: &str) -> Result<serde_json::Value, Error> {
        self.query_rewards(address, None).await
    }

    /// Query rewards for an address up to a specific epoch
    pub async fn query_rewards_until_epoch(
        &self,
        address: &str,
        until_epoch: u64,
    ) -> Result<serde_json::Value, Error> {
        self.query_rewards(address, Some(until_epoch)).await
    }

    /// Get current epoch from epoch manager contract
    pub async fn get_current_epoch(&self) -> Result<u64, Error> {
        let epoch_manager_address =
            self.config
                .contracts
                .epoch_manager
                .as_ref()
                .ok_or_else(|| {
                    Error::Other("Epoch manager contract address not configured".to_string())
                })?;

        let query = serde_json::json!({
            "current_epoch": {}
        });

        let response: serde_json::Value = self.query(epoch_manager_address, &query).await?;

        // Extract epoch number from response
        response
            .get("epoch")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| Error::Other("Failed to parse epoch from response".to_string()))
    }

    /// Validate epoch parameter for claim/query operations
    pub async fn validate_epoch(&self, epoch: u64) -> Result<(), Error> {
        let current_epoch = self.get_current_epoch().await?;

        if epoch > current_epoch {
            return Err(Error::Other(format!(
                "Cannot specify future epoch {}. Current epoch is {}",
                epoch, current_epoch
            )));
        }

        Ok(())
    }

    // =========================
    // Feature Toggle Functionality
    // =========================

    /// Update pool feature toggles with per-pool control
    /// This allows fine-tuned control over which operations are allowed for a specific pool
    /// Update feature toggles for a specific pool
    ///
    /// **v3.0.0 New Feature**: Per-pool feature toggles with pool_identifier targeting
    ///
    /// # Arguments
    ///
    /// * `pool_identifier` - The identifier of the pool to update features for
    /// * `withdrawals_enabled` - Optional flag to enable/disable withdrawals for this pool
    /// * `deposits_enabled` - Optional flag to enable/disable deposits for this pool  
    /// * `swaps_enabled` - Optional flag to enable/disable swaps for this pool
    ///
    /// # Returns
    ///
    /// Transaction response containing the feature update result
    ///
    /// # Errors
    ///
    /// * Returns error if the feature update transaction fails
    /// * Returns error if no wallet is configured
    ///
    /// # Notes
    ///
    /// In v3.0.0, all feature toggles must target specific pools via pool_identifier
    pub async fn update_pool_features(
        &self,
        pool_identifier: &str,
        withdrawals_enabled: Option<bool>,
        deposits_enabled: Option<bool>,
        swaps_enabled: Option<bool>,
    ) -> Result<TxResponse, Error> {
        let feature_toggle = mantra_dex_std::pool_manager::FeatureToggle {
            pool_identifier: pool_identifier.to_string(),
            withdrawals_enabled,
            deposits_enabled,
            swaps_enabled,
        };

        let msg = pool_manager::ExecuteMsg::UpdateConfig {
            fee_collector_addr: None,
            farm_manager_addr: None,
            pool_creation_fee: None,
            feature_toggle: Some(feature_toggle),
        };

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, vec![]).await
    }

    /// Enable withdrawals for a specific pool
    pub async fn enable_pool_withdrawals(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(true), None, None)
            .await
    }

    /// Disable withdrawals for a specific pool
    pub async fn disable_pool_withdrawals(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(false), None, None)
            .await
    }

    /// Enable deposits for a specific pool
    pub async fn enable_pool_deposits(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, Some(true), None)
            .await
    }

    /// Disable deposits for a specific pool
    pub async fn disable_pool_deposits(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, Some(false), None)
            .await
    }

    /// Enable swaps for a specific pool
    pub async fn enable_pool_swaps(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, None, Some(true))
            .await
    }

    /// Disable swaps for a specific pool
    pub async fn disable_pool_swaps(&self, pool_identifier: &str) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, None, None, Some(false))
            .await
    }

    /// Enable all operations for a specific pool
    pub async fn enable_all_pool_operations(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(true), Some(true), Some(true))
            .await
    }

    /// Disable all operations for a specific pool
    pub async fn disable_all_pool_operations(
        &self,
        pool_identifier: &str,
    ) -> Result<TxResponse, Error> {
        self.update_pool_features(pool_identifier, Some(false), Some(false), Some(false))
            .await
    }

    /// Update global feature toggles (backward compatibility)
    /// This method maintains compatibility with the v2.x approach but uses per-pool targeting
    /// The pool_identifier parameter allows targeting specific pools for global-style operations
    #[deprecated(
        since = "3.0.0",
        note = "Use update_pool_features for per-pool control"
    )]
    pub async fn update_global_features(
        &self,
        pool_identifier: &str, // Required in v3.0.0 - pools must be targeted specifically
        withdrawals_enabled: Option<bool>,
        deposits_enabled: Option<bool>,
        swaps_enabled: Option<bool>,
    ) -> Result<TxResponse, Error> {
        // Delegate to the new per-pool method
        self.update_pool_features(
            pool_identifier,
            withdrawals_enabled,
            deposits_enabled,
            swaps_enabled,
        )
        .await
    }

    // =========================
    // Fee Validation Functionality for v3.0.0
    // =========================

    /// Validate pool fee structure to ensure total fees don't exceed 20%
    /// This is a critical requirement for v3.0.0 compatibility
    /// Validate pool fees according to v3.0.0 requirements
    ///
    /// **v3.0.0 New Feature**: Enhanced fee validation ensures total fees ≤ 20%
    ///
    /// This method validates that the sum of all fees (protocol_fee + swap_fee + burn_fee + extra_fees)
    /// does not exceed 20% (0.2) as required by the v3.0.0 specification.
    ///
    /// # Arguments
    ///
    /// * `pool_fees` - The pool fee structure to validate
    ///
    /// # Returns
    ///
    /// `Ok(())` if fees are valid, otherwise returns a `FeeValidation` error
    ///
    /// # Errors
    ///
    /// * Returns `FeeValidation` error if total fees exceed 20%
    /// * Returns `FeeValidation` error if any individual fee is negative
    ///
    /// # Fee Structure
    ///
    /// The v3.0.0 fee structure includes:
    /// - `protocol_fee`: Fee for the protocol
    /// - `swap_fee`: Fee for swaps  
    /// - `burn_fee`: Optional fee that gets burned
    /// - `extra_fees`: Optional array of additional fees
    pub fn validate_pool_fees(
        &self,
        pool_fees: &mantra_dex_std::fee::PoolFee,
    ) -> Result<(), Error> {
        let protocol_fee = pool_fees.protocol_fee.share;
        let swap_fee = pool_fees.swap_fee.share;
        let burn_fee = pool_fees.burn_fee.share;

        // Calculate total from extra fees
        let extra_fees_total: cosmwasm_std::Decimal =
            pool_fees.extra_fees.iter().map(|fee| fee.share).sum();

        // Calculate total fees
        let total_fees = protocol_fee + swap_fee + burn_fee + extra_fees_total;

        // Maximum allowed total fees is 20% (0.2)
        let max_total_fees = cosmwasm_std::Decimal::percent(20);

        if total_fees > max_total_fees {
            return Err(Error::FeeValidation(format!(
                "Total fees ({}) exceed maximum allowed ({}). Protocol: {}, Swap: {}, Burn: {}, Extra: {}",
                total_fees,
                max_total_fees,
                protocol_fee,
                swap_fee,
                burn_fee,
                extra_fees_total
            )));
        }

        // Individual fee validation (each fee should be non-negative)
        // Note: cosmwasm_std::Decimal is always non-negative by design, so this validation
        // is primarily for completeness and future-proofing
        let zero = cosmwasm_std::Decimal::zero();
        if protocol_fee < zero || swap_fee < zero || burn_fee < zero {
            return Err(Error::FeeValidation(
                "Individual fees cannot be negative".to_string(),
            ));
        }

        // Validate extra fees
        for (i, fee) in pool_fees.extra_fees.iter().enumerate() {
            if fee.share < zero {
                return Err(Error::FeeValidation(format!(
                    "Extra fee {} cannot be negative: {}",
                    i, fee.share
                )));
            }
        }

        Ok(())
    }

    /// Create a validated PoolFee structure with automatic total fee checking
    /// Create a validated pool fee structure
    ///
    /// **v3.0.0 New Feature**: Helper method to create and validate pool fees in one step
    ///
    /// This method creates a `PoolFee` structure from the provided fee components and validates
    /// that the total fees do not exceed 20% as required by v3.0.0.
    ///
    /// # Arguments
    ///
    /// * `protocol_fee` - Protocol fee percentage
    /// * `swap_fee` - Swap fee percentage  
    /// * `burn_fee` - Optional burn fee percentage
    /// * `extra_fees` - Optional vector of additional fee percentages
    ///
    /// # Returns
    ///
    /// A validated `PoolFee` structure ready for use in pool creation
    ///
    /// # Errors
    ///
    /// * Returns `FeeValidation` error if total fees exceed 20%
    /// * Returns `FeeValidation` error if any individual fee is negative
    ///

    pub fn create_validated_pool_fees(
        &self,
        protocol_fee: cosmwasm_std::Decimal,
        swap_fee: cosmwasm_std::Decimal,
        burn_fee: Option<cosmwasm_std::Decimal>,
        extra_fees: Option<Vec<cosmwasm_std::Decimal>>,
    ) -> Result<mantra_dex_std::fee::PoolFee, Error> {
        let burn_fee = burn_fee.unwrap_or_else(cosmwasm_std::Decimal::zero);
        let extra_fees = extra_fees.unwrap_or_default();

        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee {
                share: protocol_fee,
            },
            swap_fee: mantra_dex_std::fee::Fee { share: swap_fee },
            burn_fee: mantra_dex_std::fee::Fee { share: burn_fee },
            extra_fees: extra_fees
                .into_iter()
                .map(|share| mantra_dex_std::fee::Fee { share })
                .collect(),
        };

        // Validate the created fee structure
        self.validate_pool_fees(&pool_fees)?;

        Ok(pool_fees)
    }
}
