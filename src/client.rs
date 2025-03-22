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

/// Mantra DEX client for interacting with the network
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
        println!("Last block height: {:?}", height.block.header.height);
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
        serde_json::from_slice::<R>(&resp.data.as_slice()).map_err(Into::into)
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
        println!("Getting last block height");
        let height = self.get_last_block_height().await?;
        let wallet = self.wallet()?;
        let rpc_client = self.rpc_client.lock().await;

        println!("Last block height: {:?}", height);
        let tx_body = Body::new(msgs, String::new(), 0u32);
        println!("Tx body: {:?}", tx_body);

        // Get account info for signing
        println!("Getting account info for {}", wallet.address().unwrap());
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

        println!("Account info: {:?}", account_info);

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
        println!("Account type_url: {}", account_any.type_url);

        // Decode the BaseAccount from the Any object's value
        let base_account = BaseAccount::decode(account_any.value.as_slice())
            .map_err(|e| Error::Rpc(format!("Failed to decode BaseAccount: {}", e)))?;

        println!("Base account: {:?}", base_account);

        let account_number = base_account.account_number;
        println!("Account number: {:?}", account_number);
        let sequence = base_account.sequence;
        println!("Sequence: {:?}", sequence);
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
        println!("Tx raw: {:?}", tx_raw);
        // Broadcast the transaction
        let response = rpc_client
            .broadcast_tx_commit(tx_raw.to_bytes().unwrap())
            .await
            .map_err(|e| Error::Rpc(format!("Failed to broadcast transaction: {}", e)))?;
        println!("Response: {:?}", response);
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

            println!("Transaction result: {:?}", tx_result);

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
    pub async fn swap(
        &self,
        pool_id: &str,
        offer_asset: Coin,
        ask_asset_denom: &str,
        max_spread: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        let msg = pool_manager::ExecuteMsg::Swap {
            pool_identifier: pool_id.to_string(),
            belief_price: None,
            receiver: None,
            ask_asset_denom: ask_asset_denom.to_string(),
            max_spread: max_spread.map(|d| {
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
    pub async fn provide_liquidity(
        &self,
        pool_id: &str,
        assets: Vec<Coin>,
        slippage_tolerance: Option<Decimal>,
    ) -> Result<TxResponse, Error> {
        let msg = pool_manager::ExecuteMsg::ProvideLiquidity {
            pool_identifier: pool_id.to_string(),
            max_spread: None,
            receiver: None,
            unlocking_duration: None,
            lock_position_identifier: None,
            slippage_tolerance: slippage_tolerance.map(|d| {
                // Convert the Decimal to the version used by mantra_dex_std
                let decimal_str = d.to_string();
                cosmwasm_std::Decimal::from_str(&decimal_str).unwrap_or_default()
            }),
        };

        let coins = assets
            .into_iter()
            .map(|a| Coin {
                denom: a.denom,
                amount: a.amount,
            })
            .collect();

        let pool_manager_address = self.config.contracts.pool_manager.clone();
        self.execute(&pool_manager_address, &msg, coins).await
    }

    /// Withdraw liquidity from a pool
    pub async fn withdraw_liquidity(
        &self,
        pool_id: &str,
        lp_amount: Uint128,
    ) -> Result<TxResponse, Error> {
        let pool = self.get_pool(pool_id).await?;
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

    /// Execute multiple swap operations
    pub async fn execute_swap_operations(
        &self,
        operations: Vec<SwapOperation>,
        amount: Uint128,
    ) -> Result<TxResponse, Error> {
        let first_op = operations
            .first()
            .ok_or_else(|| Error::Other("Swap operations list cannot be empty".to_string()))?;

        let msg = pool_manager::ExecuteMsg::Swap {
            ask_asset_denom: first_op.get_target_asset_info().clone(),
            belief_price: None,
            max_spread: None,
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
}
