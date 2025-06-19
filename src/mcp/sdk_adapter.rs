//! MCP-to-SDK Adapter
//!
//! This module provides a wrapper layer that adapts the Mantra DEX SDK for use in MCP contexts.
//! It handles async runtime integration, error mapping, connection pooling, and provides
//! an MCP-friendly interface to the underlying SDK functionality.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::{RwLock, Semaphore};
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

/// MCP SDK Adapter for managing DEX client operations with connection pooling
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
    pub async fn get_active_wallet(&self) -> McpResult<Option<MantraWallet>> {
        // For now, return None - this will be implemented when wallet management is added
        // TODO: Implement proper wallet management and retrieval
        Ok(None)
    }

    /// Get the currently active wallet info
    pub async fn get_active_wallet_info(&self) -> McpResult<Option<WalletInfo>> {
        // For now, return None - this will be implemented when wallet management is added
        // TODO: Implement proper wallet management and retrieval
        Ok(None)
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

    // =========================================================================
    // SDK Operation Wrappers
    // =========================================================================

    pub async fn get_pool(&self, pool_id: &str) -> McpResult<Value> {
        // Placeholder for get_pool implementation
        Ok(serde_json::json!({ "pool_id": pool_id }))
    }

    pub async fn get_pools(&self, _args: Value) -> McpResult<Value> {
        // Placeholder for get_pools implementation
        Ok(serde_json::json!({ "pools": [] }))
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
        _pool_id: u64,
        _operation: Option<String>,
        _include_recommendations: bool,
    ) -> McpResult<Value> {
        // Placeholder
        Ok(serde_json::json!({}))
    }

    pub async fn provide_liquidity(&self, args: Value) -> McpResult<Value> {
        // Placeholder for provide_liquidity implementation
        info!(?args, "SDK Adapter: Providing liquidity");
        // In a real implementation, this would:
        // 1. Parse args
        // 2. Get active wallet
        // 3. Get a client with the wallet
        // 4. Call client.provide_liquidity
        // 5. Return the result
        Ok(serde_json::json!({
            "status": "success",
            "message": "Liquidity provided (simulation)",
            "tx_hash": "SIMULATED_TX_HASH"
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
        info!(?args, "SDK Adapter: Withdrawing liquidity");
        Ok(serde_json::json!({
            "status": "success",
            "message": "Liquidity withdrawn (simulation)",
            "tx_hash": "SIMULATED_WITHDRAW_TX_HASH"
        }))
    }

    pub async fn get_liquidity_positions(&self, _args: Value) -> McpResult<Value> {
        info!("SDK Adapter: Getting liquidity positions");
        Ok(serde_json::json!({
            "positions": [
                {
                    "pool_id": "1",
                    "lp_token_denom": "mantra/pool/1",
                    "amount": "1000000"
                }
            ]
        }))
    }

    pub async fn execute_swap(&self, args: Value) -> McpResult<Value> {
        info!(?args, "SDK Adapter: Executing swap");
        // Placeholder implementation for swap execution
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": "0x1234567890abcdef",
            "swap_details": {
                "pool_id": args.get("pool_id").unwrap_or(&Value::String("1".to_string())),
                "offer_asset": args.get("offer_asset").unwrap_or(&Value::Null),
                "ask_asset_denom": args.get("ask_asset_denom").unwrap_or(&Value::String("uusdc".to_string())),
                "return_amount": "4950",
                "swap_fee": "50"
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    pub async fn create_pool(&self, args: Value) -> McpResult<Value> {
        info!(?args, "SDK Adapter: Creating pool");
        // Placeholder implementation for pool creation
        Ok(serde_json::json!({
            "status": "success",
            "transaction_hash": "0xabcdef1234567890",
            "pool_details": {
                "pool_id": "42",
                "pool_type": args.get("pool_type").unwrap_or(&Value::String("constant_product".to_string())),
                "assets": args.get("assets").unwrap_or(&Value::Array(vec![])),
                "fees": args.get("fees").unwrap_or(&Value::Null),
                "creation_fee": "88000000" // 88 OM in uom
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
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
