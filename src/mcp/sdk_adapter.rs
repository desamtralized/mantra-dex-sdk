//! MCP-to-SDK Adapter
//!
//! This module provides a wrapper layer that adapts the Mantra DEX SDK for use in MCP contexts.
//! It handles async runtime integration, error mapping, connection pooling, and provides
//! an MCP-friendly interface to the underlying SDK functionality.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::str::FromStr;

use chrono;
use cosmwasm_std::{Coin, Decimal, Uint128};

use serde_json::Value;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::client::MantraDexClient;
use crate::config::MantraNetworkConfig;
use crate::wallet::{MantraWallet, WalletInfo};

use super::server::{McpResult, McpServerError};

/// Configuration for connection pooling
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections per network
    pub max_connections_per_network: usize,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Connection TTL in seconds
    pub connection_ttl_secs: u64,
    /// Maximum number of retries for failed operations
    pub max_retries: u32,
    /// Base delay for exponential backoff in milliseconds
    pub retry_base_delay_ms: u64,
    /// Maximum idle time before connection is considered stale in seconds
    pub max_idle_time_secs: u64,
    /// Health check interval in seconds
    pub health_check_interval_secs: u64,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_network: 5,
            connection_timeout_secs: 30,
            connection_ttl_secs: 300, // 5 minutes
            max_retries: 3,
            retry_base_delay_ms: 100,
            max_idle_time_secs: 60,         // 1 minute
            health_check_interval_secs: 30, // 30 seconds
        }
    }
}

/// Pooled connection wrapper with metadata
#[derive(Debug)]
struct PooledConnection {
    /// The actual DEX client
    client: MantraDexClient,
    /// When this connection was created
    created_at: Instant,
    /// When this connection was last used
    last_used: Instant,
    /// Whether this connection is currently healthy
    is_healthy: bool,
}

impl PooledConnection {
    /// Create a new pooled connection
    fn new(client: MantraDexClient) -> Self {
        let now = Instant::now();
        Self {
            client,
            created_at: now,
            last_used: now,
            is_healthy: true,
        }
    }

    /// Check if the connection is expired based on TTL
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }

    /// Check if the connection is idle based on max idle time
    fn is_idle(&self, max_idle: Duration) -> bool {
        self.last_used.elapsed() > max_idle
    }

    /// Update the last used timestamp
    fn mark_used(&mut self) {
        self.last_used = Instant::now();
    }

    /// Mark connection as healthy/unhealthy
    fn set_health(&mut self, healthy: bool) {
        self.is_healthy = healthy;
    }
}

/// Connection pool for a specific network
#[derive(Debug)]
struct NetworkConnectionPool {
    /// Available connections
    connections: Vec<PooledConnection>,
    /// Network configuration
    network_config: MantraNetworkConfig,
    /// Semaphore to limit concurrent connection creation
    creation_semaphore: Semaphore,
    /// Pool configuration
    config: ConnectionPoolConfig,
}

impl NetworkConnectionPool {
    /// Create a new network connection pool
    fn new(network_config: MantraNetworkConfig, config: ConnectionPoolConfig) -> Self {
        Self {
            connections: Vec::new(),
            creation_semaphore: Semaphore::new(config.max_connections_per_network),
            network_config,
            config,
        }
    }

    /// Get a connection from the pool or create a new one
    async fn get_connection(&mut self) -> McpResult<MantraDexClient> {
        // First, try to get a healthy, non-expired connection from the pool
        for (_index, pooled_conn) in self.connections.iter_mut().enumerate() {
            if pooled_conn.is_healthy
                && !pooled_conn.is_expired(Duration::from_secs(self.config.connection_ttl_secs))
                && !pooled_conn.is_idle(Duration::from_secs(self.config.max_idle_time_secs))
            {
                pooled_conn.mark_used();
                debug!(
                    "Reusing existing connection for network: {}",
                    self.network_config.network_id
                );
                // Since MantraDexClient can't be cloned, we need to create a new client
                // with the same configuration. This is a limitation of the current SDK design.
                return self.create_new_client().await;
            }
        }

        // Remove expired or unhealthy connections
        self.cleanup_expired_connections();

        // If we're at the connection limit, wait for a permit
        let _permit = self.creation_semaphore.acquire().await.map_err(|e| {
            McpServerError::Internal(format!("Failed to acquire connection permit: {}", e))
        })?;

        // Create a new client
        let client = self.create_new_client().await?;

        // Add to pool for tracking purposes (even though we can't reuse the exact instance)
        let pooled_conn = PooledConnection::new(
            MantraDexClient::new(self.network_config.clone())
                .await
                .map_err(|e| McpServerError::Sdk(e))?,
        );

        self.connections.push(pooled_conn);

        debug!(
            "Created new connection for network: {} (pool size: {})",
            self.network_config.network_id,
            self.connections.len()
        );

        Ok(client)
    }

    /// Create a new client for the network
    async fn create_new_client(&self) -> McpResult<MantraDexClient> {
        debug!(
            "Creating new DEX client for network: {}",
            self.network_config.network_id
        );

        match MantraDexClient::new(self.network_config.clone()).await {
            Ok(client) => {
                info!(
                    "Successfully created DEX client for network: {}",
                    self.network_config.network_id
                );
                Ok(client)
            }
            Err(e) => {
                error!(
                    "Failed to create DEX client for network {}: {}",
                    self.network_config.network_id, e
                );
                Err(McpServerError::Sdk(e))
            }
        }
    }

    /// Cleanup expired and unhealthy connections
    fn cleanup_expired_connections(&mut self) {
        let ttl = Duration::from_secs(self.config.connection_ttl_secs);
        let max_idle = Duration::from_secs(self.config.max_idle_time_secs);

        let initial_count = self.connections.len();

        self.connections
            .retain(|conn| conn.is_healthy && !conn.is_expired(ttl) && !conn.is_idle(max_idle));

        let removed_count = initial_count - self.connections.len();
        if removed_count > 0 {
            debug!(
                "Cleaned up {} expired/unhealthy connections for network: {}",
                removed_count, self.network_config.network_id
            );
        }
    }

    /// Perform health checks on all connections
    async fn health_check(&mut self) {
        for pooled_conn in &mut self.connections {
            // Simple health check - try to get the latest block height
            match pooled_conn.client.get_last_block_height().await {
                Ok(_) => {
                    pooled_conn.set_health(true);
                }
                Err(e) => {
                    warn!(
                        "Health check failed for connection to network {}: {}",
                        self.network_config.network_id, e
                    );
                    pooled_conn.set_health(false);
                }
            }
        }
    }

    /// Get pool statistics
    fn get_stats(&self) -> (usize, usize, usize) {
        let total = self.connections.len();
        let healthy = self.connections.iter().filter(|c| c.is_healthy).count();
        let available_permits = self.creation_semaphore.available_permits();
        (total, healthy, available_permits)
    }
}

/// MCP SDK adapter for connection management and wallet state
#[derive(Debug)]
pub struct McpSdkAdapter {
    /// Connection pools per network
    connection_pools: Arc<RwLock<HashMap<String, NetworkConnectionPool>>>,
    /// Connection pool configuration
    config: ConnectionPoolConfig,
    /// Cache for frequently accessed data
    cache: Arc<RwLock<HashMap<String, (Value, Instant)>>>,
    /// Cache TTL
    cache_ttl: Duration,
    /// Health check task handle
    health_check_handle: Option<tokio::task::JoinHandle<()>>,
    /// Loaded wallets (address -> wallet info)
    wallets: Arc<RwLock<HashMap<String, WalletInfo>>>,
    /// Current active wallet address
    active_wallet: Arc<Mutex<Option<String>>>,
    /// Current active wallet instance (if available)
    active_wallet_instance: Arc<Mutex<Option<MantraWallet>>>,
}

impl McpSdkAdapter {
    /// Create a new MCP SDK adapter with connection pooling
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let adapter = Self {
            connection_pools: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(config.connection_ttl_secs),
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
            health_check_handle: None,
            wallets: Arc::new(RwLock::new(HashMap::new())),
            active_wallet: Arc::new(Mutex::new(None)),
            active_wallet_instance: Arc::new(Mutex::new(None)),
        };

        adapter
    }

    /// Start the background health check task
    pub async fn start_health_checks(&mut self) {
        let pools = Arc::clone(&self.connection_pools);
        let interval = Duration::from_secs(self.config.health_check_interval_secs);

        let handle = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                debug!("Running connection pool health checks");

                let mut pools_guard = pools.write().await;
                for (network_id, pool) in pools_guard.iter_mut() {
                    debug!("Health checking pool for network: {}", network_id);
                    pool.health_check().await;
                }

                debug!("Completed connection pool health checks");
            }
        });

        self.health_check_handle = Some(handle);
    }

    /// Stop the background health check task
    pub async fn stop_health_checks(&mut self) {
        if let Some(handle) = self.health_check_handle.take() {
            handle.abort();
            debug!("Stopped connection pool health checks");
        }
    }

    /// Get a client connection for the specified network
    pub async fn get_client(
        &self,
        network_config: &MantraNetworkConfig,
    ) -> McpResult<MantraDexClient> {
        let network_id = network_config.network_id.clone();

        // Get or create the network pool
        {
            let mut pools = self.connection_pools.write().await;
            if !pools.contains_key(&network_id) {
                debug!("Creating new connection pool for network: {}", network_id);
                pools.insert(
                    network_id.clone(),
                    NetworkConnectionPool::new(network_config.clone(), self.config.clone()),
                );
            }
        }

        // Get a connection from the pool
        let mut pools = self.connection_pools.write().await;
        let pool = pools.get_mut(&network_id).ok_or_else(|| {
            McpServerError::Internal(format!("Network pool not found: {}", network_id))
        })?;

        pool.get_connection().await
    }

    /// Get a client with wallet attached
    pub async fn get_client_with_wallet(
        &self,
        network_config: &MantraNetworkConfig,
        wallet: MantraWallet,
    ) -> McpResult<MantraDexClient> {
        let base_client = self.get_client(network_config).await?;
        Ok(base_client.with_wallet(wallet))
    }

    /// Execute with retry logic
    pub async fn execute_with_retry<F, T>(&self, operation: F) -> McpResult<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = McpResult<T>> + Send>>
            + Send
            + 'static,
        T: Send + 'static,
    {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        let delay = Duration::from_millis(
                            self.config.retry_base_delay_ms * (2_u64.pow(attempt)),
                        );
                        warn!(
                            "Operation failed (attempt {}), retrying in {:?}",
                            attempt + 1,
                            delay
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| McpServerError::Internal("Unknown retry error".to_string())))
    }

    /// Clean up expired cache entries and connection pools
    pub async fn cleanup(&self) -> McpResult<()> {
        // Clean cache
        {
            let mut cache = self.cache.write().await;
            let original_count = cache.len();

            cache.retain(|_, (_, timestamp)| timestamp.elapsed() < self.cache_ttl);

            let removed_count = original_count - cache.len();
            if removed_count > 0 {
                debug!("Cleaned {} expired cache entries", removed_count);
            }
        }

        // Clean connection pools
        {
            let mut pools = self.connection_pools.write().await;
            for (network_id, pool) in pools.iter_mut() {
                debug!("Cleaning connection pool for network: {}", network_id);
                pool.cleanup_expired_connections();
            }
        }

        Ok(())
    }

    /// Get cached value
    pub async fn cache_get(&self, key: &str) -> Option<Value> {
        let cache = self.cache.read().await;
        cache.get(key).and_then(|(value, timestamp)| {
            if timestamp.elapsed() < self.cache_ttl {
                Some(value.clone())
            } else {
                None
            }
        })
    }

    /// Set cached value
    pub async fn cache_set(&self, key: String, value: Value) {
        let mut cache = self.cache.write().await;
        cache.insert(key, (value, Instant::now()));
    }

    /// Clear all cached values
    pub async fn cache_clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Cleared all cache entries");
    }

    /// Get the currently active wallet
    /// 
    /// Note: Since MantraWallet doesn't implement Clone, this method
    /// should primarily be used by the server layer that manages the wallet lifecycle.
    /// For balance queries and other operations, use the methods that accept wallet parameters.
    pub async fn get_active_wallet(&self) -> McpResult<Option<MantraWallet>> {
        // For now, return None since we can't clone the wallet
        // The server layer should manage wallet access directly
        Ok(None)
    }

    /// Get the currently active wallet info
    pub async fn get_active_wallet_info(&self) -> McpResult<Option<WalletInfo>> {
        let active_address = self.active_wallet.lock().await.clone();
        if let Some(address) = active_address {
            let wallets = self.wallets.read().await;
            Ok(wallets.get(&address).cloned())
        } else {
            debug!("No active wallet set");
            Ok(None)
        }
    }

    /// Set the active wallet
    pub async fn set_active_wallet(
        &self,
        address: String,
        wallet_info: WalletInfo,
    ) -> McpResult<()> {
        // Store the wallet info and set as active
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);
        *self.active_wallet.lock().await = Some(address.clone());
        
        info!("Set active wallet: {}", address);
        Ok(())
    }

    /// Set the active wallet with the actual wallet instance
    pub async fn set_active_wallet_with_instance(
        &self,
        wallet: MantraWallet,
    ) -> McpResult<()> {
        let wallet_info = wallet.info();
        let address = wallet_info.address.clone();
        
        // Store the wallet info
        self.wallets
            .write()
            .await
            .insert(address.clone(), wallet_info);
        
        // Set as active
        *self.active_wallet.lock().await = Some(address.clone());
        
        // Store the wallet instance
        *self.active_wallet_instance.lock().await = Some(wallet);
        
        info!("Set active wallet with instance: {}", address);
        Ok(())
    }

    /// Add wallet validation methods
    pub async fn validate_wallet_exists(&self) -> McpResult<()> {
        if self.get_active_wallet_info().await?.is_none() {
            return Err(McpServerError::WalletNotConfigured);
        }
        Ok(())
    }

    /// Get wallet error handling with proper error messages
    pub async fn get_active_wallet_with_validation(&self) -> McpResult<MantraWallet> {
        match self.get_active_wallet().await? {
            Some(wallet) => Ok(wallet),
            None => Err(McpServerError::WalletNotConfigured),
        }
    }

    /// Get spendable balances for a specific address
    /// 
    /// # Arguments
    /// 
    /// * `network_config` - Network configuration for the query
    /// * `wallet_address` - Wallet address to query balances for
    /// 
    /// # Returns
    /// 
    /// JSON value containing balance information
    pub async fn get_balances_for_address_direct(
        &self,
        network_config: &MantraNetworkConfig,
        wallet_address: &str,
    ) -> McpResult<Value> {
        debug!("Getting balances for network: {}", network_config.network_id);
        info!("Querying balances for address: {}", wallet_address);

        // Get client and execute balance query
        let client = self.get_client(network_config).await?;
        
        // Query spendable balances using the SDK client
        let balances = client.get_balances_for_address(wallet_address).await
            .map_err(|e| McpServerError::Sdk(e))?;
        
        debug!("Retrieved {} balances for address {}", balances.len(), wallet_address);
        
        // Convert to JSON format
        let balance_json: Vec<Value> = balances
            .into_iter()
            .map(|coin| {
                serde_json::json!({
                    "denom": coin.denom,
                    "amount": coin.amount.to_string()
                })
            })
            .collect();

        let result = serde_json::json!({
            "address": wallet_address,
            "balances": balance_json,
            "total_tokens": balance_json.len(),
            "network": network_config.network_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        info!("Successfully retrieved balances for address: {}", wallet_address);
        Ok(result)
    }

    /// Get spendable balances for the active wallet
    /// 
    /// # Arguments
    /// 
    /// * `network_config` - Network configuration for the query
    /// * `wallet_address` - Optional specific wallet address, uses active wallet if None
    /// 
    /// # Returns
    /// 
    /// JSON value containing balance information
    pub async fn get_balances(
        &self,
        network_config: &MantraNetworkConfig,
        wallet_address: Option<String>,
    ) -> McpResult<Value> {
        // Get the wallet address to query
        let address = if let Some(addr) = wallet_address {
            addr
        } else {
            // Use active wallet address
            match self.get_active_wallet_info().await? {
                Some(wallet_info) => wallet_info.address,
                None => return Err(McpServerError::WalletNotConfigured),
            }
        };

        // Delegate to the direct address method
        self.get_balances_for_address_direct(network_config, &address).await
    }

    /// Get connection pool statistics
    pub async fn get_pool_stats(&self) -> HashMap<String, (usize, usize, usize)> {
        let pools = self.connection_pools.read().await;
        pools
            .iter()
            .map(|(network_id, pool)| (network_id.clone(), pool.get_stats()))
            .collect()
    }

    /// Get the cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let total = cache.len();
        let expired = cache
            .values()
            .filter(|(_, timestamp)| timestamp.elapsed() >= self.cache_ttl)
            .count();
        (total, total - expired)
    }

    /// Shutdown the adapter and cleanup resources
    pub async fn shutdown(&mut self) -> McpResult<()> {
        info!("Shutting down MCP SDK adapter");

        // Stop health checks
        self.stop_health_checks().await;

        // Clear caches
        self.cache_clear().await;

        // Clear connection pools
        {
            let mut pools = self.connection_pools.write().await;
            pools.clear();
        }

        info!("MCP SDK adapter shutdown complete");
        Ok(())
    }

    /// Get the default network configuration
    /// This is a temporary method until proper network configuration management is implemented
    async fn get_default_network_config(&self) -> McpResult<MantraNetworkConfig> {
        // For now, we'll use the testnet configuration
        // This should be replaced with proper configuration management
        use crate::config::NetworkConstants;
        
        let network_constants = NetworkConstants::load("mantra-dukong")
            .map_err(|e| McpServerError::Internal(format!("Failed to load network constants: {}", e)))?;
        
        Ok(MantraNetworkConfig::from_constants(&network_constants))
    }

    // =========================================================================
    // SDK Operation Wrappers
    // =========================================================================

    pub async fn get_pool(&self, pool_id: &str) -> McpResult<Value> {
        // Validate pool_id parameter
        if pool_id.is_empty() {
            return Err(McpServerError::InvalidArguments("pool_id cannot be empty".to_string()));
        }

        // Get client connection
        let client = self.get_client(&self.get_default_network_config().await?).await?;

        // Query pool information from blockchain
        let pool_info = client.get_pool(pool_id).await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Convert pool info to JSON format
        let pool_data = serde_json::json!({
            "pool_id": pool_info.pool_info.pool_identifier,
            "pool_type": match pool_info.pool_info.pool_type {
                mantra_dex_std::pool_manager::PoolType::ConstantProduct => "constant_product",
                mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "stable_swap",
            },
            "assets": pool_info.pool_info.assets.iter().map(|asset| {
                serde_json::json!({
                    "denom": asset.denom,
                    "amount": asset.amount.to_string()
                })
            }).collect::<Vec<_>>(),
            "status": {
                "swaps_enabled": pool_info.pool_info.status.swaps_enabled,
                "deposits_enabled": pool_info.pool_info.status.deposits_enabled,
                "withdrawals_enabled": pool_info.pool_info.status.withdrawals_enabled
            },
            "lp_token_denom": pool_info.pool_info.lp_denom,
            "total_share": pool_info.total_share.to_string()
        });

        Ok(pool_data)
    }

    pub async fn get_pools(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting pools with args: {:?}", args);
        
        // Parse optional parameters
        let limit = args.get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        
        let start_after = args.get("start_after")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Execute the pools query directly (without retry for now due to client not being Clone)
        let pools_result = client.get_pools(limit)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Convert pools to JSON format
        let pools_json: Vec<Value> = pools_result
            .into_iter()
            .map(|pool| {
                serde_json::json!({
                    "pool_id": pool.pool_info.pool_identifier,
                    "pool_type": match pool.pool_info.pool_type {
                        mantra_dex_std::pool_manager::PoolType::ConstantProduct => "constant_product",
                        mantra_dex_std::pool_manager::PoolType::StableSwap { .. } => "stable_swap",
                    },
                    "assets": pool.pool_info.assets.iter().map(|asset| {
                        serde_json::json!({
                            "denom": asset.denom,
                            "amount": asset.amount.to_string()
                        })
                    }).collect::<Vec<_>>(),
                    "lp_denom": pool.pool_info.lp_denom,
                    "status": {
                        "swaps_enabled": pool.pool_info.status.swaps_enabled,
                        "deposits_enabled": pool.pool_info.status.deposits_enabled,
                        "withdrawals_enabled": pool.pool_info.status.withdrawals_enabled
                    },
                    "total_share": pool.total_share.to_string()
                })
            })
            .collect();

        info!("Successfully retrieved {} pools", pools_json.len());

        Ok(serde_json::json!({
            "pools": pools_json,
            "count": pools_json.len(),
            "limit": limit,
            "start_after": start_after
        }))
    }

    pub async fn get_pool_status(
        &self,
        _pool_id: u64,
        _include_metrics: bool,
        _include_history: bool,
    ) -> McpResult<Value> {
        // Placeholder
        Ok(serde_json::json!({}))
    }

    pub async fn validate_pool_status(
        &self,
        pool_id: &str,
        operation: Option<String>,
        include_recommendations: bool,
    ) -> McpResult<Value> {
        debug!("SDK Adapter: Validating pool status for pool {} with operation {:?}", pool_id, operation);
        
        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get pool information
        let pool_result = client.get_pool(pool_id).await;
        
        let mut validation_result = serde_json::Map::new();
        validation_result.insert("pool_id".to_string(), serde_json::Value::String(pool_id.to_string()));
        validation_result.insert("validation_timestamp".to_string(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));

        match pool_result {
            Ok(pool_info) => {
                let status = &pool_info.pool_info.status;
                
                // Overall pool existence validation
                validation_result.insert("pool_exists".to_string(), serde_json::Value::Bool(true));
                validation_result.insert("pool_identifier".to_string(), serde_json::Value::String(pool_info.pool_info.pool_identifier.clone()));
                
                // Feature status validation
                let mut feature_status = serde_json::Map::new();
                feature_status.insert("swaps_enabled".to_string(), serde_json::Value::Bool(status.swaps_enabled));
                feature_status.insert("deposits_enabled".to_string(), serde_json::Value::Bool(status.deposits_enabled));
                feature_status.insert("withdrawals_enabled".to_string(), serde_json::Value::Bool(status.withdrawals_enabled));
                validation_result.insert("features".to_string(), serde_json::Value::Object(feature_status));

                // Operation-specific validation
                if let Some(op) = operation {
                    let operation_valid = match op.as_str() {
                        "swap" => status.swaps_enabled,
                        "deposit" | "provide_liquidity" => status.deposits_enabled,
                        "withdraw" | "withdraw_liquidity" => status.withdrawals_enabled,
                        _ => false,
                    };
                    
                    validation_result.insert("operation".to_string(), serde_json::Value::String(op.clone()));
                    validation_result.insert("operation_valid".to_string(), serde_json::Value::Bool(operation_valid));
                    
                    if !operation_valid {
                        validation_result.insert("operation_error".to_string(), 
                            serde_json::Value::String(format!("Operation '{}' is not enabled for this pool", op)));
                    }
                }

                // Overall status assessment
                let all_operations_enabled = status.swaps_enabled && status.deposits_enabled && status.withdrawals_enabled;
                let overall_status = if all_operations_enabled {
                    "fully_operational"
                } else if !status.swaps_enabled && !status.deposits_enabled && !status.withdrawals_enabled {
                    "disabled"
                } else {
                    "partially_operational"
                };
                
                validation_result.insert("overall_status".to_string(), serde_json::Value::String(overall_status.to_string()));
                validation_result.insert("is_operational".to_string(), serde_json::Value::Bool(all_operations_enabled));

                // Add recommendations if requested
                if include_recommendations {
                    let mut recommendations = Vec::new();
                    
                    if !status.swaps_enabled {
                        recommendations.push("Swaps are disabled - users cannot trade in this pool");
                    }
                    if !status.deposits_enabled {
                        recommendations.push("Deposits are disabled - users cannot provide liquidity");
                    }
                    if !status.withdrawals_enabled {
                        recommendations.push("Withdrawals are disabled - users cannot remove liquidity");
                    }
                    
                    if recommendations.is_empty() {
                        recommendations.push("Pool is fully operational - all operations are enabled");
                    }
                    
                    validation_result.insert("recommendations".to_string(), 
                        serde_json::Value::Array(recommendations.into_iter().map(|s| serde_json::Value::String(s.to_string())).collect()));
                }

                validation_result.insert("status".to_string(), serde_json::Value::String("success".to_string()));
            }
            Err(e) => {
                validation_result.insert("pool_exists".to_string(), serde_json::Value::Bool(false));
                validation_result.insert("error".to_string(), serde_json::Value::String(format!("Failed to get pool information: {}", e)));
                validation_result.insert("status".to_string(), serde_json::Value::String("error".to_string()));
                validation_result.insert("is_operational".to_string(), serde_json::Value::Bool(false));
                
                if include_recommendations {
                    validation_result.insert("recommendations".to_string(), 
                        serde_json::Value::Array(vec![serde_json::Value::String("Pool does not exist or is not accessible".to_string())]));
                }
            }
        }

        Ok(serde_json::Value::Object(validation_result))
    }

    pub async fn provide_liquidity(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Providing liquidity with args: {:?}", args);
        
        // Parse required parameters
        let pool_id = args.get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let assets_json = args.get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpServerError::InvalidArguments("assets array is required".to_string()))?;

        // Parse assets
        let mut assets = Vec::new();
        for asset_json in assets_json {
            let denom = asset_json.get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("asset.denom is required".to_string()))?;

            let amount_str = asset_json.get("amount")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("asset.amount is required".to_string()))?;

            let amount = Uint128::from_str(amount_str)
                .map_err(|e| McpServerError::InvalidArguments(format!("Invalid asset amount: {}", e)))?;

            assets.push(Coin {
                denom: denom.to_string(),
                amount,
            });
        }

        // Parse optional slippage parameters
        let liquidity_max_slippage = args.get("liquidity_max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        let swap_max_slippage = args.get("swap_max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Get active wallet (required for providing liquidity)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute provide liquidity directly (without retry for now due to client not being Clone)
        let liquidity_result = client.provide_liquidity(pool_id, assets, liquidity_max_slippage, swap_max_slippage)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!("Successfully provided liquidity to pool {} with tx hash: {}", pool_id, liquidity_result.txhash);

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": liquidity_result.txhash,
            "liquidity_details": {
                "pool_id": pool_id,
                "assets": assets_json,
                "liquidity_max_slippage": liquidity_max_slippage.map(|d| d.to_string()),
                "swap_max_slippage": swap_max_slippage.map(|d| d.to_string()),
                "gas_used": liquidity_result.gas_used,
                "gas_wanted": liquidity_result.gas_wanted
            },
            "block_height": liquidity_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": liquidity_result.events
        }))
    }

    pub async fn provide_liquidity_unchecked(&self, args: Value) -> McpResult<Value> {
        info!(?args, "SDK Adapter: Providing liquidity (unchecked)");
        Ok(serde_json::json!({
            "status": "success",
            "message": "Liquidity provided (unchecked, simulation)",
            "tx_hash": "SIMULATED_UNCHECKED_TX_HASH"
        }))
    }

    pub async fn withdraw_liquidity(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Withdrawing liquidity with args: {:?}", args);
        
        // Parse required parameters
        let pool_id = args.get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let amount_str = args.get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("amount is required".to_string()))?;

        let lp_amount = Uint128::from_str(amount_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid LP amount: {}", e)))?;

        // Get active wallet (required for withdrawing liquidity)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute withdraw liquidity directly (without retry for now due to client not being Clone)
        let withdraw_result = client.withdraw_liquidity(pool_id, lp_amount)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!("Successfully withdrew liquidity from pool {} with tx hash: {}", pool_id, withdraw_result.txhash);

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": withdraw_result.txhash,
            "withdrawal_details": {
                "pool_id": pool_id,
                "lp_amount": amount_str,
                "gas_used": withdraw_result.gas_used,
                "gas_wanted": withdraw_result.gas_wanted
            },
            "block_height": withdraw_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": withdraw_result.events
        }))
    }

    pub async fn get_liquidity_positions(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting liquidity positions with args: {:?}", args);
        
        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => {
                    wallet.address()
                        .map_err(|e| McpServerError::InvalidArguments(format!("Failed to get wallet address: {}", e)))?
                        .to_string()
                },
                None => {
                    return Err(McpServerError::InvalidArguments("No wallet configured and no wallet_address provided".to_string()));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet to find LP tokens
        let balances_result = client.get_balances_for_address(&wallet_address)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Filter for LP tokens (typically start with "factory/" and contain "lp" or are pool denoms)
        let mut lp_positions = Vec::new();
        
        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this looks like an LP token denom
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                // Try to extract pool identifier from the denom
                let pool_id = if let Some(last_part) = denom.split('/').last() {
                    if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    }
                } else {
                    denom
                };

                lp_positions.push(serde_json::json!({
                    "pool_id": pool_id,
                    "lp_token_denom": denom,
                    "balance": balance.amount.to_string(),
                    "wallet_address": wallet_address
                }));
            }
        }

        info!("Found {} LP positions for wallet {}", lp_positions.len(), wallet_address);

        Ok(serde_json::json!({
            "status": "success",
            "wallet_address": wallet_address,
            "positions": lp_positions,
            "total_positions": lp_positions.len(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn execute_swap(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Executing swap with args: {:?}", args);
        
        // Parse required parameters
        let pool_id = args.get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        let offer_asset = args.get("offer_asset")
            .ok_or_else(|| McpServerError::InvalidArguments("offer_asset is required".to_string()))?;

        let ask_asset_denom = args.get("ask_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("ask_asset_denom is required".to_string()))?;

        // Parse offer asset
        let offer_denom = offer_asset.get("denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("offer_asset.denom is required".to_string()))?;

        let offer_amount_str = offer_asset.get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("offer_asset.amount is required".to_string()))?;

        let offer_amount = Uint128::from_str(offer_amount_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid offer amount: {}", e)))?;

        let offer_coin = Coin {
            denom: offer_denom.to_string(),
            amount: offer_amount,
        };

        // Parse optional max_slippage
        let max_slippage = args.get("max_slippage")
            .and_then(|v| v.as_str())
            .and_then(|s| Decimal::from_str(s).ok());

        // Get active wallet (required for swaps)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute the swap directly (without retry for now due to client not being Clone)
        let swap_result = client.swap(pool_id, offer_coin, ask_asset_denom, max_slippage)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!("Successfully executed swap in pool {} with tx hash: {}", pool_id, swap_result.txhash);

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": swap_result.txhash,
            "swap_details": {
                "pool_id": pool_id,
                "offer_asset": {
                    "denom": offer_denom,
                    "amount": offer_amount_str
                },
                "ask_asset_denom": ask_asset_denom,
                "max_slippage": max_slippage.map(|d| d.to_string()),
                "gas_used": swap_result.gas_used,
                "gas_wanted": swap_result.gas_wanted
            },
            "block_height": swap_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": swap_result.events
        }))
    }

    pub async fn get_lp_token_balance(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting LP token balance with args: {:?}", args);
        
        // Parse required pool_id parameter
        let pool_id = args.get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => {
                    wallet.address()
                        .map_err(|e| McpServerError::InvalidArguments(format!("Failed to get wallet address: {}", e)))?
                        .to_string()
                },
                None => {
                    return Err(McpServerError::InvalidArguments("No wallet configured and no wallet_address provided".to_string()));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet
        let balances_result = client.get_balances_for_address(&wallet_address)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Look for LP token for this specific pool
        let mut lp_balance = None;
        let mut lp_denom = None;
        
        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this is an LP token for the specified pool
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                // Try to extract pool identifier from the denom
                if let Some(last_part) = denom.split('/').last() {
                    let extracted_pool_id = if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    };
                    
                    if extracted_pool_id == pool_id {
                        lp_balance = Some(balance.amount.to_string());
                        lp_denom = Some(denom.clone());
                        break;
                    }
                }
            }
        }

        let balance_amount = lp_balance.unwrap_or_else(|| "0".to_string());
        let token_denom = lp_denom.unwrap_or_else(|| format!("factory/mantra/lp_{}", pool_id));

        info!("LP token balance for pool {}: {} {}", pool_id, balance_amount, token_denom);

        Ok(serde_json::json!({
            "status": "success",
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_token_balance": {
                "denom": token_denom,
                "amount": balance_amount
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn get_all_lp_token_balances(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Getting all LP token balances with args: {:?}", args);
        
        // Parse optional parameters
        let include_zero_balances = args.get("include_zero_balances")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => {
                    wallet.address()
                        .map_err(|e| McpServerError::InvalidArguments(format!("Failed to get wallet address: {}", e)))?
                        .to_string()
                },
                None => {
                    return Err(McpServerError::InvalidArguments("No wallet configured and no wallet_address provided".to_string()));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get all balances for the wallet
        let balances_result = client.get_balances_for_address(&wallet_address)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Filter for LP tokens
        let mut lp_balances = Vec::new();
        
        for balance in &balances_result {
            let denom = &balance.denom;
            // Check if this looks like an LP token denom
            if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                let amount_str = balance.amount.to_string();
                
                // Skip zero balances if not requested
                if !include_zero_balances && balance.amount.is_zero() {
                    continue;
                }

                // Try to extract pool identifier from the denom
                let pool_id = if let Some(last_part) = denom.split('/').last() {
                    if last_part.starts_with("lp_") {
                        last_part.strip_prefix("lp_").unwrap_or(last_part)
                    } else {
                        last_part
                    }
                } else {
                    denom
                };

                lp_balances.push(serde_json::json!({
                    "pool_id": pool_id,
                    "lp_token_denom": denom,
                    "balance": amount_str,
                    "is_zero": balance.amount.is_zero()
                }));
            }
        }

        info!("Found {} LP token balances for wallet {}", lp_balances.len(), wallet_address);

        Ok(serde_json::json!({
            "status": "success",
            "wallet_address": wallet_address,
            "lp_balances": lp_balances,
            "total_positions": lp_balances.len(),
            "include_zero_balances": include_zero_balances,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn estimate_lp_withdrawal_amounts(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Estimating LP withdrawal amounts with args: {:?}", args);
        
        // Parse required pool_id parameter
        let pool_id = args.get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_id is required".to_string()))?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = args.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.get_active_wallet().await? {
                Some(wallet) => {
                    wallet.address()
                        .map_err(|e| McpServerError::InvalidArguments(format!("Failed to get wallet address: {}", e)))?
                        .to_string()
                },
                None => {
                    return Err(McpServerError::InvalidArguments("No wallet configured and no wallet_address provided".to_string()));
                }
            }
        };

        // Get network config and client
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client(&network_config).await?;

        // Get pool information
        let pool_info = client.get_pool(pool_id)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        // Get LP token amount to withdraw (use full balance if not provided)
        let lp_amount = if let Some(amount_str) = args.get("lp_token_amount").and_then(|v| v.as_str()) {
            Uint128::from_str(amount_str)
                .map_err(|e| McpServerError::InvalidArguments(format!("Invalid LP token amount: {}", e)))?
        } else {
            // Use full LP balance
            let balances_result = client.get_balances_for_address(&wallet_address)
                .await
                .map_err(|e| McpServerError::Sdk(e))?;

            let mut full_balance = Uint128::zero();
            for balance in &balances_result {
                let denom = &balance.denom;
                if denom.contains("factory/") && (denom.contains("lp") || denom.contains("pool")) {
                    if let Some(last_part) = denom.split('/').last() {
                        let extracted_pool_id = if last_part.starts_with("lp_") {
                            last_part.strip_prefix("lp_").unwrap_or(last_part)
                        } else {
                            last_part
                        };
                        
                        if extracted_pool_id == pool_id {
                            full_balance = balance.amount;
                            break;
                        }
                    }
                }
            }
            full_balance
        };

        if lp_amount.is_zero() {
            return Ok(serde_json::json!({
                "status": "success",
                "pool_id": pool_id,
                "wallet_address": wallet_address,
                "lp_amount": "0",
                "estimated_withdrawal": [],
                "total_share": pool_info.total_share.to_string(),
                "message": "No LP tokens to withdraw",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }));
        }

        // Calculate withdrawal amounts based on pool ratio
        let total_share = pool_info.total_share;
        let mut estimated_amounts = Vec::new();

        for asset in &pool_info.pool_info.assets {
            // Calculate proportional withdrawal amount
            // withdrawal_amount = (lp_amount / total_share) * asset_amount
            let withdrawal_amount = if !total_share.amount.is_zero() {
                asset.amount.multiply_ratio(lp_amount, total_share.amount)
            } else {
                Uint128::zero()
            };

            estimated_amounts.push(serde_json::json!({
                "denom": asset.denom,
                "amount": withdrawal_amount.to_string(),
                "pool_amount": asset.amount.to_string()
            }));
        }

        info!("Estimated withdrawal amounts for {} LP tokens from pool {}: {:?}", lp_amount, pool_id, estimated_amounts);

        Ok(serde_json::json!({
            "status": "success",
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_amount": lp_amount.to_string(),
            "estimated_withdrawal": estimated_amounts,
            "pool_info": {
                "total_share": total_share.to_string(),
                "assets": pool_info.pool_info.assets.iter().map(|asset| {
                    serde_json::json!({
                        "denom": asset.denom,
                        "amount": asset.amount.to_string()
                    })
                }).collect::<Vec<_>>()
            },
            "withdrawal_ratio": if !total_share.amount.is_zero() {
                format!("{:.6}", lp_amount.u128() as f64 / total_share.amount.u128() as f64)
            } else {
                "0.000000".to_string()
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn create_pool(&self, args: Value) -> McpResult<Value> {
        debug!("SDK Adapter: Creating pool with args: {:?}", args);
        
        // Parse required parameters
        let pool_type_str = args.get("pool_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("pool_type is required".to_string()))?;

        let assets_json = args.get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpServerError::InvalidArguments("assets array is required".to_string()))?;

        // Parse pool type
        let pool_type = match pool_type_str {
            "constant_product" => mantra_dex_std::pool_manager::PoolType::ConstantProduct,
            "stable_swap" => {
                // For stable swap, we need amplification parameter
                let amplification = args.get("amplification")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u64;
                mantra_dex_std::pool_manager::PoolType::StableSwap { amp: amplification }
            },
            _ => return Err(McpServerError::InvalidArguments("Invalid pool_type. Must be 'constant_product' or 'stable_swap'".to_string())),
        };

        // Parse assets - extract denominations and decimals
        let mut asset_denoms = Vec::new();
        let mut asset_decimals = Vec::new();

        for asset_json in assets_json {
            let denom = asset_json.get("denom")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpServerError::InvalidArguments("asset.denom is required".to_string()))?;

            let decimals = asset_json.get("decimals")
                .and_then(|v| v.as_u64())
                .unwrap_or(6) as u8; // Default to 6 decimals

            asset_denoms.push(denom.to_string());
            asset_decimals.push(decimals);
        }

        // Parse fees
        let fees_json = args.get("fees");
        let protocol_fee_str = fees_json
            .and_then(|f| f.get("protocol_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.01"); // Default 1%

        let swap_fee_str = fees_json
            .and_then(|f| f.get("swap_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.03"); // Default 3%

        let burn_fee_str = fees_json
            .and_then(|f| f.get("burn_fee"))
            .and_then(|v| v.as_str())
            .unwrap_or("0.0"); // Default 0%

        // Parse fee decimals
        let protocol_fee = Decimal::from_str(protocol_fee_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid protocol_fee: {}", e)))?;
        let swap_fee = Decimal::from_str(swap_fee_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid swap_fee: {}", e)))?;
        let burn_fee = Decimal::from_str(burn_fee_str)
            .map_err(|e| McpServerError::InvalidArguments(format!("Invalid burn_fee: {}", e)))?;

        // Create pool fees structure
        let pool_fees = mantra_dex_std::fee::PoolFee {
            protocol_fee: mantra_dex_std::fee::Fee { share: protocol_fee },
            swap_fee: mantra_dex_std::fee::Fee { share: swap_fee },
            burn_fee: mantra_dex_std::fee::Fee { share: burn_fee },
            extra_fees: vec![],
        };

        // Parse optional pool identifier
        let pool_identifier = args.get("pool_identifier")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Clone pool_identifier for response formatting
        let pool_identifier_for_response = pool_identifier.clone();

        // Get active wallet (required for pool creation)
        let wallet = self.get_active_wallet_with_validation().await?;

        // Get network config and client with wallet
        let network_config = self.get_default_network_config().await?;
        let client = self.get_client_with_wallet(&network_config, wallet).await?;

        // Execute pool creation directly (without retry for now due to client not being Clone)
        let create_result = client.create_pool(asset_denoms, asset_decimals, pool_fees, pool_type, pool_identifier)
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        info!("Successfully created pool with tx hash: {}", create_result.txhash);

        // Format the response
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": create_result.txhash,
            "pool_details": {
                "pool_type": pool_type_str,
                "assets": assets_json,
                "fees": {
                    "protocol_fee": protocol_fee_str,
                    "swap_fee": swap_fee_str,
                    "burn_fee": burn_fee_str
                },
                "pool_identifier": pool_identifier_for_response,
                "creation_fee": "88000000", // 88 OM in uom
                "gas_used": create_result.gas_used,
                "gas_wanted": create_result.gas_wanted
            },
            "block_height": create_result.height,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "events": create_result.events
        }))
    }
}

impl Default for McpSdkAdapter {
    fn default() -> Self {
        Self::new(ConnectionPoolConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let config = ConnectionPoolConfig::default();
        let adapter = McpSdkAdapter::new(config);

        // Verify initial state
        let pool_stats = adapter.get_pool_stats().await;
        assert!(pool_stats.is_empty());
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let adapter = McpSdkAdapter::default();

        // Test cache set and get
        let key = "test_key".to_string();
        let value = serde_json::json!({"test": "value"});

        adapter.cache_set(key.clone(), value.clone()).await;

        let retrieved = adapter.cache_get(&key).await;
        assert_eq!(retrieved, Some(value));

        // Test cache miss
        let missing = adapter.cache_get("nonexistent").await;
        assert_eq!(missing, None);

        // Test cache clear
        adapter.cache_clear().await;
        let after_clear = adapter.cache_get(&key).await;
        assert_eq!(after_clear, None);
    }

    #[tokio::test]
    async fn test_cleanup() {
        let adapter = McpSdkAdapter::default();

        // Add some cache entries
        adapter
            .cache_set("key1".to_string(), serde_json::json!("value1"))
            .await;
        adapter
            .cache_set("key2".to_string(), serde_json::json!("value2"))
            .await;

        // Cleanup should not remove non-expired entries
        adapter.cleanup().await.unwrap();

        let (total, _valid) = adapter.get_cache_stats().await;
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn test_connection_pool_config() {
        let config = ConnectionPoolConfig {
            max_connections_per_network: 10,
            connection_timeout_secs: 60,
            connection_ttl_secs: 600,
            max_retries: 5,
            retry_base_delay_ms: 200,
            max_idle_time_secs: 120,
            health_check_interval_secs: 45,
        };

        let adapter = McpSdkAdapter::new(config.clone());
        assert_eq!(adapter.config.max_connections_per_network, 10);
        assert_eq!(adapter.config.connection_timeout_secs, 60);
        assert_eq!(adapter.config.max_retries, 5);
    }

    #[tokio::test]
    async fn test_health_check_lifecycle() {
        let mut adapter = McpSdkAdapter::default();

        // Start health checks
        adapter.start_health_checks().await;
        assert!(adapter.health_check_handle.is_some());

        // Stop health checks
        adapter.stop_health_checks().await;
        assert!(adapter.health_check_handle.is_none());
    }

    #[tokio::test]
    async fn test_shutdown() {
        let mut adapter = McpSdkAdapter::default();

        // Add some data
        adapter
            .cache_set("test".to_string(), serde_json::json!("data"))
            .await;

        // Start health checks
        adapter.start_health_checks().await;

        // Shutdown should clean everything
        adapter.shutdown().await.unwrap();

        let (cache_total, _) = adapter.get_cache_stats().await;
        let pool_stats = adapter.get_pool_stats().await;

        assert_eq!(cache_total, 0);
        assert!(pool_stats.is_empty());
        assert!(adapter.health_check_handle.is_none());
    }
}
