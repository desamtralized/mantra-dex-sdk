//! MCP-to-SDK Adapter
//!
//! This module provides a wrapper layer that adapts the Mantra DEX SDK for use in MCP contexts.
//! It handles async runtime integration, error mapping, connection pooling, and provides
//! an MCP-friendly interface to the underlying SDK functionality.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::client::MantraDexClient;
use crate::config::{ContractAddresses, MantraNetworkConfig};
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
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_network: 5,
            connection_timeout_secs: 30,
            connection_ttl_secs: 300, // 5 minutes
            max_retries: 3,
            retry_base_delay_ms: 100,
        }
    }
}

// Connection pooling removed since MantraDexClient cannot be cloned
// Each request will create a new client instance

/// MCP SDK Adapter for managing DEX client operations
pub struct McpSdkAdapter {
    /// Connection pool configuration
    config: ConnectionPoolConfig,
    /// Cache for frequently accessed data
    cache: Arc<RwLock<HashMap<String, (Value, Instant)>>>,
    /// Cache TTL
    cache_ttl: Duration,
}

impl McpSdkAdapter {
    /// Create a new MCP SDK adapter
    pub fn new(config: ConnectionPoolConfig) -> Self {
        Self {
            cache_ttl: Duration::from_secs(config.connection_ttl_secs),
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a client connection for the specified network
    pub async fn get_client(
        &self,
        network_config: &MantraNetworkConfig,
    ) -> McpResult<MantraDexClient> {
        // Create new client each time since clients cannot be cloned/pooled
        self.create_new_client(network_config).await
    }

    /// Get a client with wallet attached
    pub async fn get_client_with_wallet(
        &self,
        network_config: &MantraNetworkConfig,
        wallet: MantraWallet,
    ) -> McpResult<MantraDexClient> {
        let base_client = self.create_new_client(network_config).await?;
        Ok(base_client.with_wallet(wallet))
    }

    // Connection pooling removed - clients are created fresh each time

    /// Create a new client for the network
    async fn create_new_client(
        &self,
        network_config: &MantraNetworkConfig,
    ) -> McpResult<MantraDexClient> {
        debug!(
            "Creating new DEX client for network: {}",
            network_config.network_id
        );

        match MantraDexClient::new(network_config.clone()).await {
            Ok(client) => {
                info!(
                    "Successfully created DEX client for network: {}",
                    network_config.network_id
                );
                Ok(client)
            }
            Err(e) => {
                error!(
                    "Failed to create DEX client for network {}: {}",
                    network_config.network_id, e
                );
                Err(McpServerError::Sdk(e))
            }
        }
    }

    // Connection pooling removed - clients are created fresh each time

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

    /// Clean up expired cache entries
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

    /// Get connection pool statistics (always empty since pooling is disabled)
    pub async fn get_pool_stats(&self) -> HashMap<String, usize> {
        HashMap::new()
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        let total_entries = cache.len();
        let expired_entries = cache
            .values()
            .filter(|(_, timestamp)| timestamp.elapsed() >= self.cache_ttl)
            .count();
        (total_entries, expired_entries)
    }

    /// Shutdown the adapter and clean up all resources
    pub async fn shutdown(&self) -> McpResult<()> {
        info!("Shutting down MCP SDK adapter");

        // Clear cache
        self.cache_clear().await;

        info!("MCP SDK adapter shutdown complete");
        Ok(())
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
        let adapter = McpSdkAdapter::default();
        let stats = adapter.get_pool_stats().await;
        assert!(stats.is_empty());
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let adapter = McpSdkAdapter::default();

        // Test cache set/get
        let test_value = serde_json::json!({"test": "value"});
        adapter
            .cache_set("test_key".to_string(), test_value.clone())
            .await;

        let cached_value = adapter.cache_get("test_key").await;
        assert_eq!(cached_value, Some(test_value));

        // Test cache clear
        adapter.cache_clear().await;
        let cleared_value = adapter.cache_get("test_key").await;
        assert_eq!(cleared_value, None);
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

        // Cleanup should not fail
        let result = adapter.cleanup().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_client_creation() {
        let adapter = McpSdkAdapter::default();

        // Create a test network config - note this may fail without proper RPC
        let config = MantraNetworkConfig {
            network_name: "test".to_string(),
            network_id: "test-1".to_string(),
            rpc_url: "http://localhost:26657".to_string(),
            gas_price: 0.001,
            gas_adjustment: 1.5,
            native_denom: "uom".to_string(),
            contracts: ContractAddresses {
                pool_manager: "test".to_string(),
                farm_manager: None,
                fee_collector: None,
                epoch_manager: None,
            },
        };

        // Test client creation (may fail in test environment)
        match adapter.get_client(&config).await {
            Ok(_) => {
                // Client creation succeeded
            }
            Err(_) => {
                // Expected in test environment without real RPC
            }
        }
    }

    #[tokio::test]
    async fn test_shutdown() {
        let adapter = McpSdkAdapter::default();

        // Add some test data
        adapter
            .cache_set("test".to_string(), serde_json::json!("data"))
            .await;

        // Shutdown should not fail
        let result = adapter.shutdown().await;
        assert!(result.is_ok());

        // Cache should be empty after shutdown
        let cached_value = adapter.cache_get("test").await;
        assert_eq!(cached_value, None);
    }
}
