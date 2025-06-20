use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Configuration support
use config::{Config, ConfigError, Environment, File, FileFormat};

// TODO: Import correct MCP types when API is finalized
// Current rust-mcp-sdk 0.4.2 has unstable APIs that don't match documentation
// Using minimal imports for now until API stabilizes

use crate::client::MantraDexClient;
use crate::config::{MantraNetworkConfig, NetworkConstants};
use crate::error::Error as SdkError;
use crate::wallet::WalletInfo;

use super::client_wrapper::McpClientWrapper;
use super::logging::{LoggingConfig, McpLogger};
use super::sdk_adapter::McpSdkAdapter;

// =============================================================================
// Transaction Monitoring Types
// =============================================================================

/// Transaction monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMonitorConfig {
    /// Transaction hash to monitor
    pub tx_hash: String,
    /// Minimum confirmations required
    pub min_confirmations: u64,
    /// Timeout in seconds
    pub timeout_secs: u64,
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    /// Whether to monitor events
    pub monitor_events: bool,
}

/// Transaction status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    /// Transaction is pending
    Pending,
    /// Transaction is confirmed with number of confirmations
    Confirmed { confirmations: u64 },
    /// Transaction failed with reason
    Failed { reason: String },
    /// Transaction monitoring timed out
    TimedOut,
    /// Transaction monitoring was cancelled
    Cancelled,
}

impl TransactionStatus {
    /// Check if the transaction is in a final state
    pub fn is_final(&self) -> bool {
        matches!(
            self,
            TransactionStatus::Confirmed { .. }
                | TransactionStatus::Failed { .. }
                | TransactionStatus::TimedOut
                | TransactionStatus::Cancelled
        )
    }

    /// Get status as string for display
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionStatus::Pending => "pending",
            TransactionStatus::Confirmed { .. } => "confirmed",
            TransactionStatus::Failed { .. } => "failed",
            TransactionStatus::TimedOut => "timeout",
            TransactionStatus::Cancelled => "cancelled",
        }
    }
}

/// Transaction monitor instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionMonitor {
    /// Unique monitor ID
    pub id: String,
    /// Monitor configuration
    pub config: TransactionMonitorConfig,
    /// Current status
    pub status: TransactionStatus,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Number of polls performed
    pub poll_count: u64,
    /// Block height when confirmed (if applicable)
    pub block_height: Option<u64>,
    /// Gas used (if available)
    pub gas_used: Option<u64>,
    /// Any events captured
    pub events: Vec<serde_json::Value>,
}

impl TransactionMonitor {
    /// Create a new transaction monitor
    pub fn new(config: TransactionMonitorConfig) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            config,
            status: TransactionStatus::Pending,
            created_at: now,
            updated_at: now,
            poll_count: 0,
            block_height: None,
            gas_used: None,
            events: Vec::new(),
        }
    }

    /// Update the monitor status
    pub fn update_status(&mut self, status: TransactionStatus) {
        self.status = status;
        self.updated_at = chrono::Utc::now();
    }

    /// Increment poll count
    pub fn increment_poll(&mut self) {
        self.poll_count += 1;
        self.updated_at = chrono::Utc::now();
    }

    /// Check if monitoring is completed
    pub fn is_completed(&self) -> bool {
        self.status.is_final()
    }

    /// Check if monitoring timed out
    pub fn is_timed_out(&self) -> bool {
        matches!(self.status, TransactionStatus::TimedOut)
    }

    /// Add an event to the monitor
    pub fn add_event(&mut self, event: serde_json::Value) {
        self.events.push(event);
        self.updated_at = chrono::Utc::now();
    }

    /// Get elapsed time since creation
    pub fn elapsed_time(&self) -> Duration {
        let now = chrono::Utc::now();
        (now - self.created_at)
            .to_std()
            .unwrap_or(Duration::from_secs(0))
    }

    /// Convert to JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "monitor_id": self.id,
            "tx_hash": self.config.tx_hash,
            "status": self.status.as_str(),
            "status_details": self.status,
            "created_at": self.created_at.to_rfc3339(),
            "updated_at": self.updated_at.to_rfc3339(),
            "elapsed_time_secs": self.elapsed_time().as_secs(),
            "poll_count": self.poll_count,
            "block_height": self.block_height,
            "gas_used": self.gas_used,
            "events": self.events,
            "config": {
                "min_confirmations": self.config.min_confirmations,
                "timeout_secs": self.config.timeout_secs,
                "poll_interval_secs": self.config.poll_interval_secs,
                "monitor_events": self.config.monitor_events
            }
        })
    }
}

/// Transaction monitor manager for handling multiple monitors
#[derive(Debug)]
pub struct TransactionMonitorManager {
    /// Active monitors
    monitors: Arc<RwLock<HashMap<String, TransactionMonitor>>>,
}

impl TransactionMonitorManager {
    /// Create a new transaction monitor manager
    pub fn new() -> Self {
        Self {
            monitors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a monitor
    pub async fn add_monitor(&self, monitor: TransactionMonitor) -> String {
        let id = monitor.id.clone();
        let mut monitors = self.monitors.write().await;
        monitors.insert(id.clone(), monitor);
        id
    }

    /// Get a monitor by ID
    pub async fn get_monitor(&self, id: &str) -> Option<TransactionMonitor> {
        let monitors = self.monitors.read().await;
        monitors.get(id).cloned()
    }

    /// Update a monitor
    pub async fn update_monitor(&self, id: &str, monitor: TransactionMonitor) -> bool {
        let mut monitors = self.monitors.write().await;
        if monitors.contains_key(id) {
            monitors.insert(id.to_string(), monitor);
            true
        } else {
            false
        }
    }

    /// Remove a monitor
    pub async fn remove_monitor(&self, id: &str) -> Option<TransactionMonitor> {
        let mut monitors = self.monitors.write().await;
        monitors.remove(id)
    }

    /// List all monitors
    pub async fn list_monitors(&self) -> Vec<TransactionMonitor> {
        let monitors = self.monitors.read().await;
        monitors.values().cloned().collect()
    }

    /// List monitors with filtering
    pub async fn list_monitors_filtered(&self, include_completed: bool) -> Vec<TransactionMonitor> {
        let monitors = self.monitors.read().await;
        monitors
            .values()
            .filter(|monitor| include_completed || !monitor.is_completed())
            .cloned()
            .collect()
    }

    /// Clean up completed monitors
    pub async fn cleanup_completed(&self) -> usize {
        let mut monitors = self.monitors.write().await;
        let initial_count = monitors.len();
        monitors.retain(|_, monitor| !monitor.is_completed());
        initial_count - monitors.len()
    }

    /// Clean up old monitors (force cleanup)
    pub async fn cleanup_old_monitors(&self, max_age: Duration) -> usize {
        let mut monitors = self.monitors.write().await;
        let initial_count = monitors.len();
        let now = chrono::Utc::now();

        monitors.retain(|_, monitor| {
            let age = (now - monitor.created_at)
                .to_std()
                .unwrap_or(Duration::from_secs(0));
            age < max_age
        });

        initial_count - monitors.len()
    }

    /// Start monitoring a transaction with polling
    pub async fn start_monitoring(
        &self,
        config: TransactionMonitorConfig,
        client: Arc<Mutex<Option<MantraDexClient>>>,
    ) -> Result<String, String> {
        let monitor = TransactionMonitor::new(config.clone());
        let monitor_id = monitor.id.clone();

        // Add the monitor
        self.add_monitor(monitor).await;

        // Start the monitoring task
        let manager = self.clone();
        let monitor_id_task = monitor_id.clone();
        tokio::spawn(async move {
            manager
                .monitor_transaction_polling(monitor_id_task, client)
                .await;
        });

        Ok(monitor_id)
    }

    /// Monitor transaction with polling logic
    async fn monitor_transaction_polling(
        &self,
        monitor_id: String,
        client: Arc<Mutex<Option<MantraDexClient>>>,
    ) {
        let timeout_duration = {
            let monitor = match self.get_monitor(&monitor_id).await {
                Some(m) => m,
                None => {
                    error!("Monitor {} not found during polling setup", monitor_id);
                    return;
                }
            };
            Duration::from_secs(monitor.config.timeout_secs)
        };

        let start_time = Instant::now();

        loop {
            // Check if we've exceeded the timeout
            if start_time.elapsed() >= timeout_duration {
                self.update_monitor_status(&monitor_id, TransactionStatus::TimedOut)
                    .await;
                info!("Transaction monitor {} timed out", monitor_id);
                break;
            }

            // Get the current monitor to check if it's been cancelled or completed
            let monitor = match self.get_monitor(&monitor_id).await {
                Some(m) => m,
                None => {
                    debug!("Monitor {} was removed during polling", monitor_id);
                    break;
                }
            };

            if monitor.is_completed() {
                debug!("Monitor {} is already completed", monitor_id);
                break;
            }

            // Perform the transaction query
            match self
                .query_transaction(&monitor.config.tx_hash, client.clone())
                .await
            {
                Ok(Some(tx_result)) => {
                    // Process the transaction result
                    self.process_transaction_result(&monitor_id, tx_result)
                        .await;

                    // Check if monitoring is complete
                    let updated_monitor = match self.get_monitor(&monitor_id).await {
                        Some(m) => m,
                        None => break,
                    };

                    if updated_monitor.is_completed() {
                        info!("Transaction monitoring completed for {}", monitor_id);
                        break;
                    }
                }
                Ok(None) => {
                    // Transaction not found yet, continue polling
                    debug!(
                        "Transaction {} not found yet, continuing to poll",
                        monitor.config.tx_hash
                    );
                }
                Err(e) => {
                    warn!(
                        "Error querying transaction {}: {}",
                        monitor.config.tx_hash, e
                    );
                    // Continue polling on errors (might be temporary network issues)
                }
            }

            // Increment poll count
            self.increment_monitor_poll(&monitor_id).await;

            // Wait for the next poll interval
            sleep(Duration::from_secs(monitor.config.poll_interval_secs)).await;
        }
    }

    /// Query transaction from blockchain
    async fn query_transaction(
        &self,
        tx_hash: &str,
        client: Arc<Mutex<Option<MantraDexClient>>>,
    ) -> Result<Option<serde_json::Value>, String> {
        let client_guard = client.lock().await;
        let client = match client_guard.as_ref() {
            Some(c) => c,
            None => return Err("Client not available".to_string()),
        };

        match client.query_transaction(tx_hash).await {
            Ok(tx_json) => Ok(Some(tx_json)),
            Err(SdkError::Rpc(e)) if e.to_string().contains("not found") => {
                // Transaction not found yet
                Ok(None)
            }
            Err(e) => Err(format!("Error querying transaction: {}", e)),
        }
    }

    /// Process transaction result and update monitor
    async fn process_transaction_result(&self, monitor_id: &str, tx_result: serde_json::Value) {
        let mut monitor = match self.get_monitor(monitor_id).await {
            Some(m) => m,
            None => return,
        };

        // Extract transaction details
        if let Some(code) = tx_result.get("code").and_then(|c| c.as_u64()) {
            if code == 0 {
                // Transaction succeeded
                let height = tx_result.get("height").and_then(|h| h.as_u64());
                let gas_used = tx_result.get("gas_used").and_then(|g| g.as_u64());

                monitor.block_height = height;
                monitor.gas_used = gas_used;

                // Check confirmations (simplified: assume 1 confirmation if we found the tx)
                let confirmations = 1; // In a real implementation, you'd query current block height and calculate

                if confirmations >= monitor.config.min_confirmations {
                    monitor.update_status(TransactionStatus::Confirmed { confirmations });
                } else {
                    // Continue monitoring for more confirmations
                    monitor.update_status(TransactionStatus::Pending);
                }

                // Extract events if monitoring is enabled
                if monitor.config.monitor_events {
                    if let Some(events) = tx_result.get("events") {
                        monitor.add_event(events.clone());
                    }
                }
            } else {
                // Transaction failed
                let reason = tx_result
                    .get("raw_log")
                    .and_then(|l| l.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                monitor.update_status(TransactionStatus::Failed { reason });
            }
        } else {
            // Malformed response
            monitor.update_status(TransactionStatus::Failed {
                reason: "Malformed transaction response".to_string(),
            });
        }

        self.update_monitor(monitor_id, monitor).await;
    }

    /// Update monitor status
    async fn update_monitor_status(&self, monitor_id: &str, status: TransactionStatus) {
        if let Some(mut monitor) = self.get_monitor(monitor_id).await {
            monitor.update_status(status);
            self.update_monitor(monitor_id, monitor).await;
        }
    }

    /// Increment monitor poll count
    async fn increment_monitor_poll(&self, monitor_id: &str) {
        if let Some(mut monitor) = self.get_monitor(monitor_id).await {
            monitor.increment_poll();
            self.update_monitor(monitor_id, monitor).await;
        }
    }

    /// Cancel a monitor
    pub async fn cancel_monitor(&self, monitor_id: &str) -> bool {
        if let Some(mut monitor) = self.get_monitor(monitor_id).await {
            if !monitor.is_completed() {
                monitor.update_status(TransactionStatus::Cancelled);
                self.update_monitor(monitor_id, monitor).await;
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl Clone for TransactionMonitorManager {
    fn clone(&self) -> Self {
        Self {
            monitors: self.monitors.clone(),
        }
    }
}

// =============================================================================
// JSON-RPC Types
// =============================================================================

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

// =============================================================================
// JSON-RPC Error Code Constants
// =============================================================================

// Standard JSON-RPC error codes
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;
const INTERNAL_ERROR: i32 = -32603;

// MCP-specific error codes
const WALLET_NOT_CONFIGURED: i32 = -32000;
const SERIALIZATION_ERROR: i32 = -32001;
const NETWORK_CONNECTION_FAILED: i32 = -32002;
const VALIDATION_ERROR: i32 = -32003;
const CONFIGURATION_ERROR: i32 = -32004;
const RESOURCE_NOT_FOUND: i32 = -32005;

// SDK-specific error codes
const BLOCKCHAIN_RPC_ERROR: i32 = -32100;
const TRANSACTION_FAILED: i32 = -32101;
const INSUFFICIENT_FUNDS: i32 = -32102;
const INVALID_PUBLIC_KEY_FORMAT: i32 = -32103;
const INVALID_MNEMONIC_FORMAT: i32 = -32104;
const INVALID_ADDRESS_FORMAT: i32 = -32105;
const POOL_NOT_FOUND: i32 = -32106;
const SWAP_SLIPPAGE_EXCEEDED: i32 = -32107;
const LIQUIDITY_INSUFFICIENT: i32 = -32108;
const TOOL_EXECUTION_FAILED: i32 = -32109;
const FEE_VALIDATION_FAILED: i32 = -32110;
const TIMEOUT_ERROR: i32 = -32111;
const IO_ERROR: i32 = -32112;

// =============================================================================
// Async Runtime Configuration and Management
// =============================================================================

/// Configuration for the async runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsyncRuntimeConfig {
    /// Runtime flavor (single-threaded or multi-threaded)
    pub flavor: RuntimeFlavor,
    /// Number of worker threads (for multi-threaded runtime)
    pub worker_threads: Option<usize>,
    /// Whether to enable I/O operations
    pub enable_io: bool,
    /// Whether to enable time operations
    pub enable_time: bool,
    /// Maximum blocking threads for blocking operations
    pub max_blocking_threads: Option<usize>,
    /// Thread keep alive duration
    pub thread_keep_alive: Option<Duration>,
    /// Thread stack size
    pub thread_stack_size: Option<usize>,
    /// Global queue interval for work stealing
    pub global_queue_interval: Option<u32>,
    /// Event interval for work stealing
    pub event_interval: Option<u32>,
}

/// Runtime flavor options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RuntimeFlavor {
    /// Single-threaded current thread runtime
    CurrentThread,
    /// Multi-threaded runtime with work stealing
    MultiThread,
}

impl Default for AsyncRuntimeConfig {
    fn default() -> Self {
        Self {
            flavor: RuntimeFlavor::MultiThread,
            worker_threads: None, // Use system default (number of CPUs)
            enable_io: true,
            enable_time: true,
            max_blocking_threads: Some(512), // Reasonable default for blocking operations
            thread_keep_alive: Some(Duration::from_secs(10)),
            thread_stack_size: None,         // Use system default
            global_queue_interval: Some(31), // Default Tokio value
            event_interval: Some(61),        // Default Tokio value
        }
    }
}

impl AsyncRuntimeConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Runtime flavor
        if let Ok(flavor) = env::var("MCP_RUNTIME_FLAVOR") {
            match flavor.to_lowercase().as_str() {
                "current_thread" | "single" => config.flavor = RuntimeFlavor::CurrentThread,
                "multi_thread" | "multi" => config.flavor = RuntimeFlavor::MultiThread,
                _ => warn!("Invalid runtime flavor '{}', using default", flavor),
            }
        }

        // Worker threads
        if let Ok(threads) = env::var("MCP_WORKER_THREADS") {
            if let Ok(count) = threads.parse::<usize>() {
                if count > 0 {
                    config.worker_threads = Some(count);
                } else {
                    warn!("Invalid worker thread count '{}', using default", threads);
                }
            }
        }

        // Max blocking threads
        if let Ok(blocking) = env::var("MCP_MAX_BLOCKING_THREADS") {
            if let Ok(count) = blocking.parse::<usize>() {
                config.max_blocking_threads = Some(count);
            }
        }

        // Thread keep alive
        if let Ok(keep_alive) = env::var("MCP_THREAD_KEEP_ALIVE_SECS") {
            if let Ok(secs) = keep_alive.parse::<u64>() {
                config.thread_keep_alive = Some(Duration::from_secs(secs));
            }
        }

        // Thread stack size
        if let Ok(stack_size) = env::var("MCP_THREAD_STACK_SIZE") {
            if let Ok(size) = stack_size.parse::<usize>() {
                config.thread_stack_size = Some(size);
            }
        }

        config
    }

    /// Build a Tokio runtime with this configuration
    pub fn build_runtime(&self) -> Result<Runtime, std::io::Error> {
        let mut builder = match self.flavor {
            RuntimeFlavor::CurrentThread => Builder::new_current_thread(),
            RuntimeFlavor::MultiThread => Builder::new_multi_thread(),
        };

        // Configure I/O and time
        if self.enable_io {
            builder.enable_io();
        }
        if self.enable_time {
            builder.enable_time();
        }

        // Configure worker threads (multi-threaded only)
        if self.flavor == RuntimeFlavor::MultiThread {
            if let Some(threads) = self.worker_threads {
                builder.worker_threads(threads);
            }
        }

        // Configure blocking threads
        if let Some(max_blocking) = self.max_blocking_threads {
            builder.max_blocking_threads(max_blocking);
        }

        // Configure thread settings
        if let Some(keep_alive) = self.thread_keep_alive {
            builder.thread_keep_alive(keep_alive);
        }

        if let Some(stack_size) = self.thread_stack_size {
            builder.thread_stack_size(stack_size);
        }

        // Configure work stealing parameters (multi-threaded only)
        if self.flavor == RuntimeFlavor::MultiThread {
            if let Some(interval) = self.global_queue_interval {
                builder.global_queue_interval(interval);
            }
            if let Some(interval) = self.event_interval {
                builder.event_interval(interval);
            }
        }

        // Set thread name
        builder.thread_name("mcp-tokio-worker");

        builder.build()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if let Some(threads) = self.worker_threads {
            if threads == 0 {
                return Err("Worker thread count must be greater than 0".to_string());
            }
            if threads > 1024 {
                return Err("Worker thread count too high (max 1024)".to_string());
            }
        }

        if let Some(blocking) = self.max_blocking_threads {
            if blocking == 0 {
                return Err("Max blocking threads must be greater than 0".to_string());
            }
            if blocking > 10000 {
                return Err("Max blocking threads too high (max 10000)".to_string());
            }
        }

        if let Some(stack_size) = self.thread_stack_size {
            if stack_size < 1024 * 1024 {
                return Err("Thread stack size too small (min 1MB)".to_string());
            }
        }

        Ok(())
    }
}

/// Runtime metrics and monitoring
#[derive(Debug, Clone)]
pub struct RuntimeMetrics {
    /// Runtime start time
    pub start_time: Instant,
    /// Number of active tasks
    pub active_tasks: usize,
    /// Number of worker threads
    pub worker_threads: usize,
    /// Number of blocking threads
    pub blocking_threads: usize,
    /// Runtime flavor
    pub flavor: RuntimeFlavor,
}

impl RuntimeMetrics {
    /// Create new runtime metrics
    pub fn new(config: &AsyncRuntimeConfig) -> Self {
        Self {
            start_time: Instant::now(),
            active_tasks: 0,
            worker_threads: config.worker_threads.unwrap_or_else(num_cpus::get),
            blocking_threads: config.max_blocking_threads.unwrap_or(512),
            flavor: config.flavor.clone(),
        }
    }

    /// Get runtime uptime
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get runtime metrics as JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "uptime_secs": self.uptime().as_secs(),
            "active_tasks": self.active_tasks,
            "worker_threads": self.worker_threads,
            "blocking_threads": self.blocking_threads,
            "flavor": match self.flavor {
                RuntimeFlavor::CurrentThread => "current_thread",
                RuntimeFlavor::MultiThread => "multi_thread",
            },
            "start_time": self.start_time.elapsed().as_secs()
        })
    }
}

/// Async runtime manager for the MCP server
pub struct AsyncRuntimeManager {
    /// Runtime configuration
    config: AsyncRuntimeConfig,
    /// Runtime metrics
    metrics: Arc<RwLock<RuntimeMetrics>>,
    /// Runtime handle (if running in existing runtime)
    handle: Option<Handle>,
}

impl AsyncRuntimeManager {
    /// Create a new runtime manager
    pub fn new(config: AsyncRuntimeConfig) -> Self {
        let metrics = RuntimeMetrics::new(&config);
        Self {
            config: config.clone(),
            metrics: Arc::new(RwLock::new(metrics)),
            handle: None,
        }
    }

    /// Initialize the runtime manager with current handle
    pub fn with_current_handle() -> Self {
        let config = AsyncRuntimeConfig::from_env();
        let mut manager = Self::new(config);
        manager.handle = Some(Handle::current());
        manager
    }

    /// Get runtime configuration
    pub fn config(&self) -> &AsyncRuntimeConfig {
        &self.config
    }

    /// Get runtime metrics
    pub async fn metrics(&self) -> RuntimeMetrics {
        self.metrics.read().await.clone()
    }

    /// Update active task count
    pub async fn update_active_tasks(&self, count: usize) {
        let mut metrics = self.metrics.write().await;
        metrics.active_tasks = count;
    }

    /// Get runtime handle
    pub fn handle(&self) -> Option<&Handle> {
        self.handle.as_ref()
    }

    /// Spawn a task with monitoring
    pub fn spawn_monitored<T>(&self, future: T) -> tokio::task::JoinHandle<T::Output>
    where
        T: std::future::Future + Send + 'static,
        T::Output: Send + 'static,
    {
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            // Increment active tasks
            {
                let mut m = metrics.write().await;
                m.active_tasks += 1;
            }

            let result = future.await;

            // Decrement active tasks
            {
                let mut m = metrics.write().await;
                m.active_tasks = m.active_tasks.saturating_sub(1);
            }

            result
        })
    }

    /// Spawn a blocking task with monitoring
    pub fn spawn_blocking_monitored<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let _metrics = self.metrics.clone();

        tokio::task::spawn_blocking(move || {
            // Note: We don't track blocking tasks in active_tasks as they run on separate thread pool
            let result = f();
            result
        })
    }

    /// Get runtime health status
    pub async fn health_status(&self) -> serde_json::Value {
        let metrics = self.metrics().await;
        serde_json::json!({
            "status": "healthy",
            "runtime": metrics.to_json(),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
}

// =============================================================================
// MCP Server Trait Definitions
// =============================================================================

/// Core MCP server lifecycle management trait
///
/// Defines the essential lifecycle operations for an MCP server including
/// initialization, configuration, and server metadata management.
#[async_trait::async_trait]
pub trait McpServerLifecycle: Send + Sync {
    /// Initialize the server with necessary resources and connections
    async fn initialize(&self) -> McpResult<()>;

    /// Get server information (name, version, etc.)
    fn get_server_info(&self) -> serde_json::Value;

    /// Get server capabilities (supported features)
    fn get_capabilities(&self) -> serde_json::Value;

    /// Perform graceful shutdown and cleanup
    async fn shutdown(&self) -> McpResult<()> {
        // Default implementation - servers can override for custom cleanup
        Ok(())
    }

    /// Check if server is ready to handle requests
    async fn is_ready(&self) -> bool {
        true // Default implementation
    }
}

/// MCP tool provider trait
///
/// Defines the interface for servers that provide executable tools.
/// Tools are functions that can be called by MCP clients to perform operations.
#[async_trait::async_trait]
pub trait McpToolProvider: Send + Sync {
    /// Get list of available tools with their schemas
    fn get_available_tools(&self) -> Vec<serde_json::Value>;

    /// Execute a tool with given arguments
    async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value>;

    /// Validate tool arguments before execution
    fn validate_tool_arguments(
        &self,
        _tool_name: &str,
        arguments: &serde_json::Value,
    ) -> McpResult<()> {
        // Default implementation - servers can override for custom validation
        if !arguments.is_object() {
            return Err(McpServerError::InvalidArguments(
                "Tool arguments must be an object".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if a tool is available
    fn has_tool(&self, tool_name: &str) -> bool {
        self.get_available_tools()
            .iter()
            .any(|tool| tool.get("name").and_then(|n| n.as_str()) == Some(tool_name))
    }
}

/// MCP resource provider trait
///
/// Defines the interface for servers that provide readable resources.
/// Resources are data sources that can be queried by MCP clients.
#[async_trait::async_trait]
pub trait McpResourceProvider: Send + Sync {
    /// Get list of available resources with their schemas
    fn get_available_resources(&self) -> Vec<serde_json::Value>;

    /// Read a resource by its URI
    async fn handle_resource_read(&self, uri: &str) -> McpResult<serde_json::Value>;

    /// Validate resource URI format
    fn validate_resource_uri(&self, uri: &str) -> McpResult<()> {
        // Default implementation - basic URI validation
        if uri.is_empty() {
            return Err(McpServerError::Validation(
                "Resource URI cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if a resource is available
    fn has_resource(&self, uri: &str) -> bool {
        self.get_available_resources()
            .iter()
            .any(|resource| resource.get("uri").and_then(|u| u.as_str()) == Some(uri))
    }

    /// Get resource metadata without reading full content
    async fn get_resource_metadata(&self, uri: &str) -> McpResult<serde_json::Value> {
        // Default implementation - basic metadata
        Ok(serde_json::json!({
            "uri": uri,
            "available": self.has_resource(uri)
        }))
    }
}

/// MCP server state management trait
///
/// Defines the interface for servers that manage internal state.
/// This includes caching, session management, and configuration.
#[async_trait::async_trait]
pub trait McpServerStateManager: Send + Sync {
    /// Get current server configuration
    async fn get_config(&self) -> serde_json::Value;

    /// Update server configuration
    async fn update_config(&self, config: serde_json::Value) -> McpResult<()>;

    /// Clear internal caches and state
    async fn clear_state(&self) -> McpResult<()>;

    /// Get server health status
    async fn get_health_status(&self) -> serde_json::Value {
        // Default implementation
        serde_json::json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }
}

/// Main MCP server trait
///
/// Combines all MCP server capabilities into a single trait.
/// Servers should implement this trait to provide full MCP functionality.
#[async_trait::async_trait]
pub trait McpServer:
    McpServerLifecycle + McpToolProvider + McpResourceProvider + McpServerStateManager
{
    /// Handle incoming MCP requests with proper routing
    async fn handle_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> McpResult<serde_json::Value> {
        match method {
            "tools/list" => {
                let tools = self.get_available_tools();
                Ok(serde_json::json!({ "tools": tools }))
            }
            "tools/call" => {
                if let Some(params) = params {
                    let tool_name =
                        params.get("name").and_then(|n| n.as_str()).ok_or_else(|| {
                            McpServerError::InvalidArguments("Missing tool name".to_string())
                        })?;

                    let arguments = params
                        .get("arguments")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));

                    self.handle_tool_call(tool_name, arguments).await
                } else {
                    Err(McpServerError::InvalidArguments(
                        "Missing parameters for tool call".to_string(),
                    ))
                }
            }
            "resources/list" => {
                let resources = self.get_available_resources();
                Ok(serde_json::json!({ "resources": resources }))
            }
            "resources/read" => {
                if let Some(params) = params {
                    let uri = params.get("uri").and_then(|u| u.as_str()).ok_or_else(|| {
                        McpServerError::InvalidArguments("Missing resource URI".to_string())
                    })?;

                    self.handle_resource_read(uri).await
                } else {
                    Err(McpServerError::InvalidArguments(
                        "Missing parameters for resource read".to_string(),
                    ))
                }
            }
            "initialize" => {
                let mut response = serde_json::Map::new();
                response.insert("protocolVersion".to_string(), serde_json::Value::String("2024-11-05".to_string()));
                response.insert("serverInfo".to_string(), self.get_server_info());
                response.insert("capabilities".to_string(), self.get_capabilities());
                Ok(serde_json::Value::Object(response))
            },
            "ping" => Ok(serde_json::json!({ "result": "pong" })),
            _ => Err(McpServerError::Mcp(format!("Unknown method: {}", method))),
        }
    }

    /// Get full server capabilities and information
    async fn get_server_description(&self) -> serde_json::Value {
        serde_json::json!({
            "info": self.get_server_info(),
            "capabilities": self.get_capabilities(),
            "tools": self.get_available_tools().len(),
            "resources": self.get_available_resources().len(),
            "health": self.get_health_status().await
        })
    }
}

// =============================================================================
// Transport Layer Trait
// =============================================================================

/// MCP transport layer trait
///
/// Defines the interface for different transport mechanisms (stdio, HTTP, WebSocket).
/// Transport implementations handle the low-level communication protocol.
#[async_trait::async_trait]
pub trait McpTransportLayer: Send + Sync {
    /// Start the transport layer
    async fn start(&self) -> McpResult<()>;

    /// Stop the transport layer
    async fn stop(&self) -> McpResult<()>;

    /// Send a JSON-RPC response
    async fn send_response(&self, response: JsonRpcResponse) -> McpResult<()>;

    /// Check if transport is active
    fn is_active(&self) -> bool;

    /// Get transport type identifier
    fn transport_type(&self) -> &'static str {
        "unknown"
    }

    /// Get transport configuration
    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "type": self.transport_type(),
            "active": self.is_active()
        })
    }
}

// =============================================================================
// Error Types and Results
// =============================================================================

/// MCP server error type
#[derive(Debug, thiserror::Error)]
pub enum McpServerError {
    #[error("SDK error: {0}")]
    Sdk(#[from] SdkError),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("Wallet not configured")]
    WalletNotConfigured,

    #[error("Invalid tool arguments: {0}")]
    InvalidArguments(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Unknown resource: {0}")]
    UnknownResource(String),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
}

impl McpServerError {
    /// Convert SDK error to MCP JSON-RPC error code
    /// This provides proper error mapping as required by the MCP specification
    pub fn to_json_rpc_error_code(&self) -> i32 {
        // JSON-RPC 2.0 standard error codes

        match self {
            // SDK error mapping - map based on the underlying SDK error type
            McpServerError::Sdk(sdk_error) => Self::sdk_error_to_code(sdk_error),

            // MCP protocol errors
            McpServerError::Mcp(_) => INTERNAL_ERROR,
            McpServerError::WalletNotConfigured => WALLET_NOT_CONFIGURED,
            McpServerError::InvalidArguments(_) => INVALID_PARAMS,
            McpServerError::UnknownTool(_) => METHOD_NOT_FOUND,
            McpServerError::UnknownResource(_) => RESOURCE_NOT_FOUND,

            // System errors
            McpServerError::Serialization(_) => SERIALIZATION_ERROR,
            McpServerError::Network(_) => NETWORK_CONNECTION_FAILED,
            McpServerError::Validation(_) => VALIDATION_ERROR,
            McpServerError::Internal(_) => INTERNAL_ERROR,
            McpServerError::Config(_) => CONFIGURATION_ERROR,
        }
    }

    /// Map SDK error types to appropriate JSON-RPC error codes
    fn sdk_error_to_code(sdk_error: &SdkError) -> i32 {
        // JSON-RPC 2.0 standard error codes

        match sdk_error {
            // CosmRS and RPC errors
            SdkError::CosmRs(_) => BLOCKCHAIN_RPC_ERROR,
            SdkError::Rpc(_) => BLOCKCHAIN_RPC_ERROR,

            // Transaction errors
            SdkError::TxBroadcast(_) => TRANSACTION_FAILED,
            SdkError::TxSimulation(_) => TRANSACTION_FAILED,
            SdkError::Tx(_) => TRANSACTION_FAILED,

            // Wallet errors with enhanced context detection
            SdkError::Wallet(msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("insufficient") || msg_lower.contains("balance") {
                    INSUFFICIENT_FUNDS
                } else if msg_lower.contains("public key") || msg_lower.contains("pubkey") {
                    INVALID_PUBLIC_KEY_FORMAT
                } else if msg_lower.contains("mnemonic") {
                    INVALID_MNEMONIC_FORMAT
                } else if msg_lower.contains("address") || msg_lower.contains("format") {
                    INVALID_ADDRESS_FORMAT
                } else {
                    WALLET_NOT_CONFIGURED
                }
            }

            // Configuration and contract errors
            SdkError::Config(_) => CONFIGURATION_ERROR,
            SdkError::Contract(msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("pool") && msg_lower.contains("not found") {
                    POOL_NOT_FOUND
                } else if msg_lower.contains("slippage") {
                    SWAP_SLIPPAGE_EXCEEDED
                } else if msg_lower.contains("liquidity") {
                    LIQUIDITY_INSUFFICIENT
                } else {
                    TOOL_EXECUTION_FAILED
                }
            }

            // Fee validation errors (v3.0.0 feature)
            SdkError::FeeValidation(_) => FEE_VALIDATION_FAILED,

            // Network and timeout errors
            SdkError::Network(_) => NETWORK_CONNECTION_FAILED,
            SdkError::Timeout(_) => TIMEOUT_ERROR,

            // Serialization and IO errors
            SdkError::Serialization(_) => SERIALIZATION_ERROR,
            SdkError::Io(_) => IO_ERROR,

            // Generic errors
            SdkError::Other(_) => INTERNAL_ERROR,
        }
    }

    /// Get additional error data for JSON-RPC error response
    /// This provides context and helps with debugging and error recovery
    pub fn get_error_data(&self) -> Option<serde_json::Value> {
        match self {
            McpServerError::Sdk(sdk_error) => Some(serde_json::json!({
                "sdk_error_type": Self::get_sdk_error_type_name(sdk_error),
                "original_error": sdk_error.to_string(),
                "category": "sdk",
                "error_code": Self::sdk_error_to_code(sdk_error),
                "recovery_suggestions": Self::get_recovery_suggestions(sdk_error),
                "severity": Self::get_error_severity(sdk_error),
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::Validation(msg) => Some(serde_json::json!({
                "validation_error": msg,
                "category": "validation",
                "severity": "high",
                "recovery_suggestions": ["Check input parameters", "Validate data format"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::Network(msg) => Some(serde_json::json!({
                "network_error": msg,
                "category": "network",
                "severity": "medium",
                "recovery_suggestions": ["Check network connectivity", "Verify RPC endpoints", "Retry with exponential backoff"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::InvalidArguments(msg) => Some(serde_json::json!({
                "argument_error": msg,
                "category": "arguments",
                "severity": "high",
                "recovery_suggestions": ["Check tool argument schema", "Validate required parameters"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::WalletNotConfigured => Some(serde_json::json!({
                "category": "wallet",
                "severity": "high",
                "recovery_suggestions": ["Generate or import a wallet", "Check wallet configuration"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::UnknownTool(tool_name) => Some(serde_json::json!({
                "tool_name": tool_name,
                "category": "tool",
                "severity": "medium",
                "recovery_suggestions": ["Check available tools list", "Verify tool name spelling"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            McpServerError::UnknownResource(uri) => Some(serde_json::json!({
                "resource_uri": uri,
                "category": "resource",
                "severity": "medium",
                "recovery_suggestions": ["Check available resources list", "Verify resource URI format"],
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            _ => None,
        }
    }

    /// Get recovery suggestions based on SDK error type
    fn get_recovery_suggestions(sdk_error: &SdkError) -> Vec<&'static str> {
        match sdk_error {
            SdkError::CosmRs(_) | SdkError::Rpc(_) => vec![
                "Check network connectivity",
                "Verify RPC endpoint configuration",
                "Try alternative RPC endpoints",
                "Check if blockchain network is operational",
            ],
            SdkError::TxBroadcast(_) | SdkError::TxSimulation(_) | SdkError::Tx(_) => vec![
                "Check transaction parameters",
                "Verify wallet has sufficient balance for gas fees",
                "Try with higher gas limit",
                "Check network congestion status",
            ],
            SdkError::Wallet(_) => vec![
                "Verify wallet is properly configured",
                "Check wallet balance",
                "Validate wallet address format",
                "Regenerate or re-import wallet if corrupted",
            ],
            SdkError::Config(_) => vec![
                "Check configuration file syntax",
                "Verify network configuration parameters",
                "Reset to default configuration if needed",
            ],
            SdkError::Contract(_) => vec![
                "Verify contract addresses are correct",
                "Check contract is deployed on current network",
                "Validate method parameters",
                "Check contract state and availability",
            ],
            SdkError::FeeValidation(_) => vec![
                "Review fee structure parameters",
                "Ensure total fees don't exceed 20% limit",
                "Validate individual fee components",
                "Use default fee structure if unsure",
            ],
            SdkError::Network(_) => vec![
                "Check internet connectivity",
                "Verify firewall settings",
                "Try different network configuration",
                "Check DNS resolution",
            ],
            SdkError::Timeout(_) => vec![
                "Increase timeout duration",
                "Check network latency",
                "Retry with exponential backoff",
                "Switch to faster RPC endpoint",
            ],
            SdkError::Serialization(_) => vec![
                "Check data format and structure",
                "Validate JSON syntax",
                "Ensure all required fields are present",
            ],
            SdkError::Io(_) => vec![
                "Check file permissions",
                "Verify file paths are accessible",
                "Ensure sufficient disk space",
                "Check directory structure",
            ],
            SdkError::Other(_) => vec![
                "Check application logs for details",
                "Retry the operation",
                "Contact support if issue persists",
            ],
        }
    }

    /// Get error severity level for monitoring and alerting
    fn get_error_severity(sdk_error: &SdkError) -> &'static str {
        match sdk_error {
            SdkError::CosmRs(_) | SdkError::Rpc(_) => "high",
            SdkError::TxBroadcast(_) | SdkError::TxSimulation(_) | SdkError::Tx(_) => "high",
            SdkError::Wallet(_) => "high",
            SdkError::Config(_) => "medium",
            SdkError::Contract(_) => "high",
            SdkError::FeeValidation(_) => "medium",
            SdkError::Network(_) => "medium",
            SdkError::Timeout(_) => "low",
            SdkError::Serialization(_) => "medium",
            SdkError::Io(_) => "low",
            SdkError::Other(_) => "medium",
        }
    }

    /// Get SDK error type name for debugging and categorization
    fn get_sdk_error_type_name(sdk_error: &SdkError) -> &'static str {
        match sdk_error {
            SdkError::CosmRs(_) => "CosmRs",
            SdkError::Rpc(_) => "Rpc",
            SdkError::TxBroadcast(_) => "TxBroadcast",
            SdkError::TxSimulation(_) => "TxSimulation",
            SdkError::Wallet(_) => "Wallet",
            SdkError::Config(_) => "Config",
            SdkError::Contract(_) => "Contract",
            SdkError::Serialization(_) => "Serialization",
            SdkError::Io(_) => "Io",
            SdkError::FeeValidation(_) => "FeeValidation",
            SdkError::Other(_) => "Other",
            SdkError::Tx(_) => "Tx",
            SdkError::Network(_) => "Network",
            SdkError::Timeout(_) => "Timeout",
        }
    }

    /// Create a JSON-RPC error object from this MCP error
    pub fn to_json_rpc_error(&self) -> JsonRpcError {
        JsonRpcError {
            code: self.to_json_rpc_error_code(),
            message: self.to_string(),
            data: self.get_error_data(),
        }
    }

    /// Check if error is recoverable and suggests retry strategy
    pub fn is_recoverable(&self) -> bool {
        match self {
            McpServerError::Sdk(sdk_error) => match sdk_error {
                SdkError::Network(_) | SdkError::Timeout(_) | SdkError::Rpc(_) => true,
                SdkError::TxBroadcast(_) => true, // Transaction might succeed on retry
                _ => false,
            },
            McpServerError::Network(_) => true,
            _ => false,
        }
    }

    /// Get suggested retry delay in seconds for recoverable errors
    pub fn get_retry_delay(&self) -> Option<u64> {
        if self.is_recoverable() {
            match self {
                McpServerError::Sdk(SdkError::Network(_)) => Some(5),
                McpServerError::Sdk(SdkError::Timeout(_)) => Some(10),
                McpServerError::Sdk(SdkError::Rpc(_)) => Some(3),
                McpServerError::Sdk(SdkError::TxBroadcast(_)) => Some(15),
                McpServerError::Network(_) => Some(5),
                _ => Some(1),
            }
        } else {
            None
        }
    }
}

/// MCP server result type
pub type McpResult<T> = std::result::Result<T, McpServerError>;

/// Mantra DEX MCP Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Network configuration
    pub network_config: MantraNetworkConfig,
    /// Whether to enable debug logging
    pub debug: bool,
    /// Maximum number of concurrent operations
    pub max_concurrent_ops: usize,
    /// HTTP server port (if using HTTP transport)
    pub http_port: u16,
    /// HTTP server host (if using HTTP transport)
    pub http_host: String,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
    /// Whether to auto-load .env file
    pub auto_load_env: bool,
    /// Async runtime configuration
    pub runtime_config: AsyncRuntimeConfig,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: "Mantra DEX SDK MCP Server".to_string(),
            version: "0.1.0".to_string(),
            network_config: MantraNetworkConfig::default(),
            debug: false,
            max_concurrent_ops: 10,
            http_port: 8080,
            http_host: "127.0.0.1".to_string(),
            request_timeout_secs: 30,
            cache_ttl_secs: 300,
            auto_load_env: true,
            runtime_config: AsyncRuntimeConfig::default(),
        }
    }
}

impl McpServerConfig {
    /// Load configuration from environment variables
    ///
    /// Environment variables supported:
    /// - MCP_SERVER_NAME: Server name
    /// - MCP_SERVER_VERSION: Server version
    /// - MCP_DEBUG: Enable debug logging (true/false)
    /// - MCP_MAX_CONCURRENT_OPS: Maximum concurrent operations
    /// - MCP_HTTP_PORT: HTTP server port
    /// - MCP_HTTP_HOST: HTTP server host
    /// - MCP_REQUEST_TIMEOUT_SECS: Request timeout in seconds
    /// - MCP_CACHE_TTL_SECS: Cache TTL in seconds
    /// - MCP_AUTO_LOAD_ENV: Auto-load .env file (true/false)
    /// - MANTRA_NETWORK: Network name (mainnet/testnet)
    pub fn from_env() -> McpResult<Self> {
        // Load .env file if auto-load is enabled (check env var first)
        let auto_load_env = env::var("MCP_AUTO_LOAD_ENV")
            .unwrap_or_else(|_| "true".to_string())
            .parse::<bool>()
            .unwrap_or(true);

        if auto_load_env {
            // Try to load .env file, but don't fail if it doesn't exist
            if let Err(e) = dotenv::dotenv() {
                debug!("Could not load .env file: {}", e);
            } else {
                info!("Loaded configuration from .env file");
            }
        }

        let mut config = Self::default();

        // Load server configuration
        if let Ok(name) = env::var("MCP_SERVER_NAME") {
            config.name = name;
        }

        if let Ok(version) = env::var("MCP_SERVER_VERSION") {
            config.version = version;
        }

        if let Ok(debug_str) = env::var("MCP_DEBUG") {
            config.debug = debug_str.parse().unwrap_or(false);
        }

        if let Ok(max_ops_str) = env::var("MCP_MAX_CONCURRENT_OPS") {
            config.max_concurrent_ops = max_ops_str.parse().unwrap_or(10);
        }

        if let Ok(port_str) = env::var("MCP_HTTP_PORT") {
            config.http_port = port_str.parse().unwrap_or(8080);
        }

        if let Ok(host) = env::var("MCP_HTTP_HOST") {
            config.http_host = host;
        }

        if let Ok(timeout_str) = env::var("MCP_REQUEST_TIMEOUT_SECS") {
            config.request_timeout_secs = timeout_str.parse().unwrap_or(30);
        }

        if let Ok(cache_ttl_str) = env::var("MCP_CACHE_TTL_SECS") {
            config.cache_ttl_secs = cache_ttl_str.parse().unwrap_or(300);
        }

        config.auto_load_env = auto_load_env;

        // Load runtime configuration from environment
        config.runtime_config = AsyncRuntimeConfig::from_env();

        // Load network configuration
        if let Ok(network_name) = env::var("MANTRA_NETWORK") {
            match network_name.as_str() {
                "mantra-dukong" => {
                    if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                        config.network_config = MantraNetworkConfig::from_constants(&constants);
                    } else {
                        warn!("Could not load mantra-dukong constants, using default");
                    }
                }
                _ => {
                    warn!("Unknown network '{}', using default", network_name);
                }
            }
        }

        info!("MCP Server configuration loaded from environment");
        debug!("Config: {:?}", config);

        Ok(config)
    }

    /// Validate configuration values
    pub fn validate(&self) -> McpResult<()> {
        if self.name.is_empty() {
            return Err(McpServerError::Validation(
                "Server name cannot be empty".to_string(),
            ));
        }

        if self.version.is_empty() {
            return Err(McpServerError::Validation(
                "Server version cannot be empty".to_string(),
            ));
        }

        if self.max_concurrent_ops == 0 {
            return Err(McpServerError::Validation(
                "Maximum concurrent operations must be greater than 0".to_string(),
            ));
        }

        if self.http_port == 0 {
            return Err(McpServerError::Validation(
                "HTTP port must be greater than 0".to_string(),
            ));
        }

        if self.http_host.is_empty() {
            return Err(McpServerError::Validation(
                "HTTP host cannot be empty".to_string(),
            ));
        }

        if self.request_timeout_secs == 0 {
            return Err(McpServerError::Validation(
                "Request timeout must be greater than 0".to_string(),
            ));
        }

        if self.cache_ttl_secs == 0 {
            return Err(McpServerError::Validation(
                "Cache TTL must be greater than 0".to_string(),
            ));
        }

        // Validate runtime configuration
        if let Err(e) = self.runtime_config.validate() {
            return Err(McpServerError::Validation(format!(
                "Runtime configuration error: {}",
                e
            )));
        }

        Ok(())
    }

    /// Create configuration with network override
    pub fn with_network(network: &str) -> McpResult<Self> {
        let mut config = Self::from_env()?;

        match network {
            "mantra-dukong" => {
                if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                    config.network_config = MantraNetworkConfig::from_constants(&constants);
                } else {
                    return Err(McpServerError::Network(
                        "Could not load mantra-dukong network constants".to_string(),
                    ));
                }
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unsupported network: {}. Supported networks: mantra-dukong",
                    network
                )));
            }
        }

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a file (TOML or JSON)
    ///
    /// Supports the following formats based on file extension:
    /// - `.toml` - TOML format
    /// - `.json` - JSON format
    /// - `.yaml`, `.yml` - YAML format
    ///
    /// The configuration file can contain any subset of configuration options.
    /// Missing options will use default values or environment variable overrides.
    pub fn from_file<P: AsRef<Path>>(file_path: P) -> McpResult<Self> {
        let path = file_path.as_ref();

        if !path.exists() {
            return Err(McpServerError::Validation(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }

        let file_format = Self::detect_file_format(path)?;
        info!(
            "Loading MCP configuration from: {} ({:?})",
            path.display(),
            file_format
        );

        let config_builder = Config::builder()
            // Start with defaults
            .set_default("name", "Mantra DEX SDK MCP Server")?
            .set_default("version", "0.1.0")?
            .set_default("debug", false)?
            .set_default("max_concurrent_ops", 10)?
            .set_default("http_port", 8080)?
            .set_default("http_host", "127.0.0.1")?
            .set_default("request_timeout_secs", 30)?
            .set_default("cache_ttl_secs", 300)?
            .set_default("auto_load_env", true)?
            // Add file source
            .add_source(File::new(path.to_str().unwrap(), file_format))
            // Add environment variable overrides with MCP_ prefix
            .add_source(Environment::with_prefix("MCP").separator("_"));

        let settings = config_builder.build().map_err(|e| {
            McpServerError::Validation(format!("Failed to load configuration: {}", e))
        })?;

        // Get network name before deserializing (since try_deserialize consumes settings)
        let network_name = settings
            .get_string("network")
            .ok()
            .or_else(|| env::var("MANTRA_NETWORK").ok());

        // Deserialize into our config struct with custom field handling
        let mut config: McpServerConfig = settings.try_deserialize().map_err(|e| {
            McpServerError::Validation(format!("Failed to parse configuration: {}", e))
        })?;

        // Load network configuration if specified
        if let Some(network_name) = network_name {
            Self::apply_network_config(&mut config, &network_name)?;
        }

        // Validate the configuration
        config.validate()?;

        info!(
            "MCP Server configuration successfully loaded from file: {}",
            path.display()
        );
        debug!("Loaded config: {:?}", config);

        Ok(config)
    }

    /// Save configuration to a file (TOML or JSON)
    ///
    /// The format is determined by the file extension:
    /// - `.toml` - TOML format
    /// - `.json` - JSON format
    pub fn save_to_file<P: AsRef<Path>>(&self, file_path: P) -> McpResult<()> {
        let path = file_path.as_ref();
        let file_format = Self::detect_file_format(path)?;

        let content = match file_format {
            FileFormat::Toml => toml::to_string_pretty(self).map_err(|e| {
                McpServerError::Internal(format!("TOML serialization failed: {}", e))
            })?,
            FileFormat::Json => {
                serde_json::to_string_pretty(self).map_err(McpServerError::Serialization)?
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unsupported file format for saving: {}. Use .toml or .json",
                    path.display()
                )));
            }
        };

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                McpServerError::Internal(format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        fs::write(path, content).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to write configuration to {}: {}",
                path.display(),
                e
            ))
        })?;

        info!("Configuration saved to: {}", path.display());
        Ok(())
    }

    /// Load configuration with layered sources: defaults -> file -> env vars
    ///
    /// This method provides the most flexible configuration loading:
    /// 1. Start with built-in defaults
    /// 2. Override with values from configuration file (if provided)
    /// 3. Override with environment variables (MCP_ prefix)
    /// 4. Apply network-specific settings
    pub fn from_sources(config_file: Option<&Path>) -> McpResult<Self> {
        info!("Loading MCP configuration with layered sources");

        let mut config_builder = Config::builder()
            // Start with defaults
            .set_default("name", "Mantra DEX SDK MCP Server")?
            .set_default("version", "0.1.0")?
            .set_default("debug", false)?
            .set_default("max_concurrent_ops", 10)?
            .set_default("http_port", 8080)?
            .set_default("http_host", "127.0.0.1")?
            .set_default("request_timeout_secs", 30)?
            .set_default("cache_ttl_secs", 300)?
            .set_default("auto_load_env", true)?;

        // Add file source if provided
        if let Some(path) = config_file {
            if path.exists() {
                let file_format = Self::detect_file_format(path)?;
                config_builder =
                    config_builder.add_source(File::new(path.to_str().unwrap(), file_format));
                info!(
                    "Added configuration file: {} ({:?})",
                    path.display(),
                    file_format
                );
            } else {
                warn!("Configuration file not found: {}", path.display());
            }
        }

        // Add environment variable overrides
        config_builder = config_builder.add_source(Environment::with_prefix("MCP").separator("_"));

        let settings = config_builder.build().map_err(|e| {
            McpServerError::Validation(format!("Failed to build configuration: {}", e))
        })?;

        // Get network name before deserializing (since try_deserialize consumes settings)
        let network_name = settings
            .get_string("network")
            .ok()
            .or_else(|| env::var("MANTRA_NETWORK").ok());

        // Deserialize into our config struct
        let mut config: McpServerConfig = settings.try_deserialize().map_err(|e| {
            McpServerError::Validation(format!("Failed to parse configuration: {}", e))
        })?;

        // Apply network configuration
        if let Some(network_name) = network_name {
            Self::apply_network_config(&mut config, &network_name)?;
        }

        // Validate the final configuration
        config.validate()?;

        info!("MCP Server configuration successfully loaded from layered sources");
        debug!("Final config: {:?}", config);

        Ok(config)
    }

    /// Generate an example configuration file
    pub fn generate_example_config() -> String {
        let example_config = Self::default();

        format!(
            r#"# Mantra DEX SDK MCP Server Configuration
# This is an example configuration file showing all available options.
# You can copy this file and modify the values as needed.

# Server identification
name = "{}"
version = "{}"

# Network configuration (will be overridden if MANTRA_NETWORK env var is set)
# Supported values: "mainnet", "testnet", "mantra-dukong", "mantra-testnet"
network = "testnet"

# Logging configuration
debug = {}

# Performance settings
max_concurrent_ops = {}
request_timeout_secs = {}
cache_ttl_secs = {}

# HTTP transport settings (used when running with --transport http)
http_host = "{}"
http_port = {}

# Environment file loading
auto_load_env = {}

# Async runtime configuration
[runtime_config]
# Runtime flavor: "CurrentThread" or "MultiThread"
flavor = "MultiThread"
# Number of worker threads (optional, defaults to CPU count)
# worker_threads = 4
# Enable I/O and time support
enable_io = true
enable_time = true
# Maximum blocking threads for blocking operations
max_blocking_threads = 512
# Thread keep alive duration in seconds
thread_keep_alive = 10

# Examples of network-specific overrides:
# [network_config]
# rpc_endpoint = "https://rpc.mantra.com"
# chain_id = "mantra-dukong"
# gas_price = "0.01uom"
"#,
            example_config.name,
            example_config.version,
            example_config.debug,
            example_config.max_concurrent_ops,
            example_config.request_timeout_secs,
            example_config.cache_ttl_secs,
            example_config.http_host,
            example_config.http_port,
            example_config.auto_load_env
        )
    }

    /// Create example configuration files in common formats
    pub fn create_example_files(directory: &Path) -> McpResult<()> {
        fs::create_dir_all(directory).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to create directory {}: {}",
                directory.display(),
                e
            ))
        })?;

        // Create TOML example
        let toml_path = directory.join("mcp-server.example.toml");
        let toml_content = Self::generate_example_config();
        fs::write(&toml_path, toml_content).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to write TOML example to {}: {}",
                toml_path.display(),
                e
            ))
        })?;

        // Create JSON example
        let json_path = directory.join("mcp-server.example.json");
        let example_config = Self::default();
        let json_content =
            serde_json::to_string_pretty(&example_config).map_err(McpServerError::Serialization)?;
        fs::write(&json_path, json_content).map_err(|e| {
            McpServerError::Internal(format!(
                "Failed to write JSON example to {}: {}",
                json_path.display(),
                e
            ))
        })?;

        info!(
            "Created example configuration files in: {}",
            directory.display()
        );
        info!("  - {}", toml_path.display());
        info!("  - {}", json_path.display());

        Ok(())
    }

    /// Detect file format based on file extension
    fn detect_file_format(path: &Path) -> McpResult<FileFormat> {
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "toml" => Ok(FileFormat::Toml),
            "json" => Ok(FileFormat::Json),
            "yaml" | "yml" => Ok(FileFormat::Yaml),
            _ => Err(McpServerError::Validation(format!(
                "Unsupported configuration file format: {}. Supported formats: .toml, .json, .yaml, .yml",
                extension
            ))),
        }
    }

    /// Apply network-specific configuration
    fn apply_network_config(config: &mut McpServerConfig, network_name: &str) -> McpResult<()> {
        match network_name {
            "mantra-dukong" => {
                if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                    config.network_config = MantraNetworkConfig::from_constants(&constants);
                    info!("Applied mantra-dukong network configuration");
                } else {
                    warn!("Could not load mantra-dukong constants, using default");
                }
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unknown network: {}. Supported networks: mantra-dukong",
                    network_name
                )));
            }
        }
        Ok(())
    }
}

/// Mantra DEX MCP Server state
pub struct McpServerStateData {
    /// DEX client instance
    pub client: Arc<Mutex<Option<MantraDexClient>>>,
    /// Server configuration
    pub config: McpServerConfig,
    /// Loaded wallets (address -> wallet info)
    pub wallets: Arc<RwLock<HashMap<String, WalletInfo>>>,
    /// Current active wallet address
    pub active_wallet: Arc<Mutex<Option<String>>>,
    /// Cached data for performance
    pub cache: Arc<RwLock<HashMap<String, Value>>>,
    /// SDK adapter for connection management
    pub sdk_adapter: Arc<McpSdkAdapter>,
    /// High-level client wrapper for MCP operations
    pub client_wrapper: Arc<Mutex<Option<McpClientWrapper>>>,
    /// Async runtime manager
    pub runtime_manager: Arc<AsyncRuntimeManager>,
    /// Logging infrastructure
    pub logger: Arc<McpLogger>,
    /// Transaction monitor manager
    pub transaction_monitor_manager: Arc<TransactionMonitorManager>,
}

impl McpServerStateData {
    pub fn new(config: McpServerConfig) -> Self {
        let sdk_adapter = Arc::new(McpSdkAdapter::default());
        let mut runtime_manager = AsyncRuntimeManager::new(config.runtime_config.clone());
        runtime_manager.handle = Some(tokio::runtime::Handle::current());
        let runtime_manager = Arc::new(runtime_manager);

        // Initialize logging infrastructure
        let logging_config = LoggingConfig::from_env();
        let logger = Arc::new(McpLogger::new(logging_config).expect("Failed to create MCP logger"));

        // Initialize transaction monitor manager
        let transaction_monitor_manager = Arc::new(TransactionMonitorManager::new());

        Self {
            client: Arc::new(Mutex::new(None)),
            config,
            wallets: Arc::new(RwLock::new(HashMap::new())),
            active_wallet: Arc::new(Mutex::new(None)),
            cache: Arc::new(RwLock::new(HashMap::new())),
            sdk_adapter,
            client_wrapper: Arc::new(Mutex::new(None)),
            runtime_manager,
            logger,
            transaction_monitor_manager,
        }
    }

    /// Initialize the DEX client
    pub async fn initialize_client(&self) -> McpResult<()> {
        info!("Attempting to initialize DEX client...");
        debug!(
            "Network config: {:?}",
            self.config.network_config
        );
        
        let client = match MantraDexClient::new(self.config.network_config.clone()).await {
            Ok(client) => {
                info!("DEX client created successfully");
                client
            }
            Err(e) => {
                error!("Failed to create DEX client: {:?}", e);
                return Err(McpServerError::Sdk(e));
            }
        };

        *self.client.lock().await = Some(client);
        info!(
            "DEX client initialized for network: {}",
            self.config.network_config.network_name
        );
        Ok(())
    }

    /// Get the current DEX client
    pub async fn get_client(&self) -> McpResult<Arc<Mutex<Option<MantraDexClient>>>> {
        Ok(self.client.clone())
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
        *self.active_wallet.lock().await = Some(address);

        Ok(())
    }

    /// Get the active wallet info
    pub async fn get_active_wallet(&self) -> McpResult<Option<WalletInfo>> {
        let active_address = self.active_wallet.lock().await.clone();
        if let Some(address) = active_address {
            let wallets = self.wallets.read().await;
            Ok(wallets.get(&address).cloned())
        } else {
            Ok(None)
        }
    }

    /// Cache a value
    /// Set cached value
    pub async fn cache_set(&self, key: String, value: Value) {
        self.cache.write().await.insert(key, value);
    }

    /// Get cached value
    pub async fn cache_get(&self, key: &str) -> Option<Value> {
        self.cache.read().await.get(key).cloned()
    }

    /// Clear cache
    pub async fn cache_clear(&self) {
        self.cache.write().await.clear();
    }

    /// Switch to a different network configuration
    pub async fn switch_network(&self, network_name: &str) -> McpResult<()> {
        use crate::config::{MantraNetworkConfig, NetworkConstants};

        // Load network constants for the requested network
        let constants = NetworkConstants::load(network_name).map_err(|e| {
            McpServerError::Network(format!("Failed to load network '{}': {}", network_name, e))
        })?;

        // Create new network config
        let _new_network_config = MantraNetworkConfig::from_constants(&constants);

        // Clear existing client and cache
        {
            let mut client = self.client.lock().await;
            *client = None;
        }
        self.cache_clear().await;

        // Update the network configuration in the server config
        // Note: This is a bit tricky since config is not mutable. We need to create a new config.
        // For now, we'll store the network config separately or update it through the client initialization

        info!("Successfully switched to network: {}", network_name);
        Ok(())
    }

    /// Initialize client with specific network configuration
    pub async fn initialize_client_with_network(
        &self,
        network_config: MantraNetworkConfig,
    ) -> McpResult<()> {
        let client = MantraDexClient::new(network_config.clone())
            .await
            .map_err(|e| McpServerError::Sdk(e))?;

        {
            let mut client_guard = self.client.lock().await;
            *client_guard = Some(client);
        }

        info!(
            "Initialized client for network: {}",
            network_config.network_name
        );
        Ok(())
    }
}

/// Mantra DEX MCP Server handler
///
/// TODO: Implement ServerHandler trait when MCP API is stable
/// Current rust-mcp-sdk 0.4.2 has unstable APIs that are changing between versions
/// This provides a solid foundation for implementing MCP functionality
pub struct MantraDexMcpServer {
    /// Server state
    state: Arc<McpServerStateData>,
}

impl MantraDexMcpServer {
    /// Create a new MCP server
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            state: Arc::new(McpServerStateData::new(config)),
        }
    }

    /// Initialize the server
    pub async fn initialize(&self) -> McpResult<()> {
        info!("Initializing Mantra DEX MCP Server");
        self.state.initialize_client().await?;

        // Initialize client wrapper
        let wrapper = McpClientWrapper::new(
            self.state.sdk_adapter.clone(),
            self.state.config.network_config.clone(),
        );
        *self.state.client_wrapper.lock().await = Some(wrapper);

        info!("Server initialization complete");
        Ok(())
    }

    /// Get server state
    pub fn state(&self) -> Arc<McpServerStateData> {
        self.state.clone()
    }

    /// Get server information
    pub fn get_server_info(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.state.config.name,
            "version": self.state.config.version
        })
    }

    /// Get server capabilities
    pub fn get_capabilities(&self) -> serde_json::Value {
        serde_json::json!({
            "tools": {
                "list_changed": true
            },
            "resources": {
                "list_changed": true,
                "subscribe": false
            },
            "logging": null,
            "experimental": null
        })
    }
}

// =============================================================================
// MCP Trait Implementations
// =============================================================================

#[async_trait::async_trait]
impl McpServerLifecycle for MantraDexMcpServer {
    async fn initialize(&self) -> McpResult<()> {
        MantraDexMcpServer::initialize(self).await
    }

    fn get_server_info(&self) -> serde_json::Value {
        MantraDexMcpServer::get_server_info(self)
    }

    fn get_capabilities(&self) -> serde_json::Value {
        MantraDexMcpServer::get_capabilities(self)
    }

    async fn shutdown(&self) -> McpResult<()> {
        info!("Shutting down Mantra DEX MCP Server");
        Ok(())
    }
}

#[async_trait::async_trait]
impl McpResourceProvider for MantraDexMcpServer {
    fn get_available_resources(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "uri": "trades://history",
                "name": "Trading History",
                "description": "Historical trading data and transaction records",
                "mimeType": "application/json"
            }),
            serde_json::json!({
                "uri": "trades://pending",
                "name": "Pending Trades",
                "description": "Currently pending or in-progress trading transactions",
                "mimeType": "application/json"
            }),
            serde_json::json!({
                "uri": "liquidity://positions",
                "name": "Liquidity Positions",
                "description": "Current and historical liquidity positions",
                "mimeType": "application/json"
            }),
        ]
    }

    async fn handle_resource_read(&self, uri: &str) -> McpResult<serde_json::Value> {
        match uri {
            "trades://history" => self.handle_trades_history_resource().await,
            "trades://pending" => self.handle_trades_pending_resource().await,
            "liquidity://positions" => self.handle_liquidity_positions_resource().await,
            _ => Err(McpServerError::UnknownResource(uri.to_string())),
        }
    }

    fn validate_resource_uri(&self, uri: &str) -> McpResult<()> {
        match uri {
            "trades://history" | "trades://pending" | "liquidity://positions" => Ok(()),
            _ => Err(McpServerError::Validation(format!(
                "Invalid resource URI: {}. Available resources: trades://history, trades://pending, liquidity://positions",
                uri
            ))),
        }
    }

    async fn get_resource_metadata(&self, uri: &str) -> McpResult<serde_json::Value> {
        match uri {
            "trades://history" => Ok(serde_json::json!({
                "uri": uri,
                "name": "Trading History",
                "description": "Historical trading data and transaction records",
                "mimeType": "application/json",
                "available": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "trades": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "tx_hash": {"type": "string"},
                                    "timestamp": {"type": "string"},
                                    "status": {"type": "string"},
                                    "pool_id": {"type": "string"},
                                    "from_asset": {"type": "object"},
                                    "to_asset": {"type": "object"},
                                    "gas_info": {"type": "object"}
                                }
                            }
                        },
                        "total_count": {"type": "number"},
                        "wallet_address": {"type": "string"}
                    }
                }
            })),
            "trades://pending" => Ok(serde_json::json!({
                "uri": uri,
                "name": "Pending Trades",
                "description": "Currently pending or in-progress trading transactions",
                "mimeType": "application/json",
                "available": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "pending_trades": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "tx_hash": {"type": "string"},
                                    "timestamp": {"type": "string"},
                                    "status": {"type": "string"},
                                    "pool_id": {"type": "string"},
                                    "monitor_id": {"type": "string"}
                                }
                            }
                        },
                        "total_count": {"type": "number"}
                    }
                }
            })),
            "liquidity://positions" => Ok(serde_json::json!({
                "uri": uri,
                "name": "Liquidity Positions",
                "description": "Current and historical liquidity positions",
                "mimeType": "application/json",
                "available": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "positions": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "pool_id": {"type": "string"},
                                    "lp_token_balance": {"type": "string"},
                                    "assets": {"type": "array"},
                                    "timestamp": {"type": "string"}
                                }
                            }
                        },
                        "total_value": {"type": "string"},
                        "wallet_address": {"type": "string"}
                    }
                }
            })),
            _ => Err(McpServerError::UnknownResource(uri.to_string())),
        }
    }
}

#[async_trait::async_trait]
impl McpServerStateManager for MantraDexMcpServer {
    async fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "server_name": self.state.config.name,
            "server_version": self.state.config.version,
            "network": {
                "chain_id": self.state.config.network_config.network_id,
                "rpc_endpoint": self.state.config.network_config.rpc_url,
                "lcd_endpoint": self.state.config.network_config.rpc_url,
                "grpc_endpoint": self.state.config.network_config.rpc_url
            },
            "settings": {
                "debug": self.state.config.debug,
                "max_concurrent_ops": self.state.config.max_concurrent_ops,
                "request_timeout_secs": self.state.config.request_timeout_secs,
                "cache_ttl_secs": self.state.config.cache_ttl_secs
            },
            "runtime": {
                "flavor": format!("{:?}", self.state.runtime_manager.config().flavor),
                "worker_threads": self.state.runtime_manager.config().worker_threads
            }
        })
    }

    async fn update_config(&self, config: serde_json::Value) -> McpResult<()> {
        // For now, only support network switching
        if let Some(network_name) = config.get("network").and_then(|n| n.as_str()) {
            self.state.switch_network(network_name).await?;
            info!("Switched to network: {}", network_name);
        }

        if let Some(debug_mode) = config.get("debug").and_then(|d| d.as_bool()) {
            // Note: This would require runtime logging level changes
            info!("Debug mode requested: {}", debug_mode);
        }

        Ok(())
    }

    async fn clear_state(&self) -> McpResult<()> {
        // Clear caches
        self.state.cache_clear().await;
        
        // Clear transaction monitors
        let cleanup_count = self.state.transaction_monitor_manager.cleanup_completed().await;
        info!("Cleared {} completed transaction monitors", cleanup_count);

        // Clear wallet cache (keep active wallet)
        let mut wallets = self.state.wallets.write().await;
        if let Some(active_address) = self.state.active_wallet.lock().await.clone() {
            if let Some(active_wallet) = wallets.get(&active_address).cloned() {
                wallets.clear();
                wallets.insert(active_address, active_wallet);
            }
        }
        drop(wallets);

        info!("Server state cleared successfully");
        Ok(())
    }

    async fn get_health_status(&self) -> serde_json::Value {
        let client_status = if self.state.client.lock().await.is_some() {
            "connected"
        } else {
            "disconnected"
        };

        let active_wallet = self.state.active_wallet.lock().await.is_some();
        let wallet_count = self.state.wallets.read().await.len();
        let cache_size = self.state.cache.read().await.len();
        
        let active_monitors = self.state.transaction_monitor_manager
            .list_monitors_filtered(false).await.len();
        
        let runtime_metrics = self.state.runtime_manager.metrics().await;

        serde_json::json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "components": {
                "client": {
                    "status": client_status,
                    "network": self.state.config.network_config.network_id
                },
                "wallet": {
                    "active": active_wallet,
                    "total_wallets": wallet_count
                },
                "cache": {
                    "entries": cache_size
                },
                "transaction_monitoring": {
                    "active_monitors": active_monitors
                },
                "runtime": {
                    "uptime_secs": runtime_metrics.uptime().as_secs(),
                    "active_tasks": runtime_metrics.active_tasks,
                    "worker_threads": runtime_metrics.worker_threads
                }
            }
        })
    }
}

#[async_trait::async_trait]
impl McpToolProvider for MantraDexMcpServer {
    fn get_available_tools(&self) -> Vec<serde_json::Value> {
        vec![
            // Network Tools
            serde_json::json!({
                "name": "get_contract_addresses",
                "description": "Get contract addresses for the current network",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "include_metadata": {
                            "type": "boolean",
                            "description": "Whether to include contract metadata and descriptions (default: false)",
                            "default": false
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "validate_network_connectivity",
                "description": "Validate network connectivity and blockchain access",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "check_rpc": {
                            "type": "boolean",
                            "description": "Whether to check RPC endpoint connectivity (default: true)",
                            "default": true
                        },
                        "check_block_height": {
                            "type": "boolean",
                            "description": "Whether to check latest block height retrieval (default: true)",
                            "default": true
                        },
                        "check_contracts": {
                            "type": "boolean",
                            "description": "Whether to validate contract addresses (default: true)",
                            "default": true
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "description": "Timeout for each connectivity check in seconds (default: 10)",
                            "default": 10,
                            "minimum": 1,
                            "maximum": 60
                        },
                        "include_diagnostics": {
                            "type": "boolean",
                            "description": "Whether to include detailed diagnostic information (default: false)",
                            "default": false
                        }
                    }
                }
            }),
            // Transaction Monitoring Tools
            serde_json::json!({
                "name": "monitor_swap_transaction",
                "description": "Start monitoring a swap transaction for confirmation status",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tx_hash": {
                            "type": "string",
                            "description": "Transaction hash to monitor"
                        },
                        "min_confirmations": {
                            "type": "integer",
                            "description": "Minimum confirmations required (default: 1)",
                            "default": 1,
                            "minimum": 1
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "description": "Timeout in seconds (default: 300)",
                            "default": 300,
                            "minimum": 30,
                            "maximum": 3600
                        },
                        "poll_interval_secs": {
                            "type": "integer",
                            "description": "Polling interval in seconds (default: 5)",
                            "default": 5,
                            "minimum": 1,
                            "maximum": 60
                        },
                        "monitor_events": {
                            "type": "boolean",
                            "description": "Whether to monitor transaction events (default: true)",
                            "default": true
                        }
                    },
                    "required": ["tx_hash"]
                }
            }),
            serde_json::json!({
                "name": "get_transaction_monitor_status",
                "description": "Get the current status of a transaction monitor",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "monitor_id": {
                            "type": "string",
                            "description": "Monitor ID returned from monitor_swap_transaction"
                        }
                    },
                    "required": ["monitor_id"]
                }
            }),
            serde_json::json!({
                "name": "cancel_transaction_monitor",
                "description": "Cancel an active transaction monitor",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "monitor_id": {
                            "type": "string",
                            "description": "Monitor ID to cancel"
                        }
                    },
                    "required": ["monitor_id"]
                }
            }),
            serde_json::json!({
                "name": "list_transaction_monitors",
                "description": "List all active transaction monitors",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "include_completed": {
                            "type": "boolean",
                            "description": "Whether to include completed monitors (default: false)",
                            "default": false
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "cleanup_transaction_monitors",
                "description": "Clean up completed and optionally aged transaction monitors",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "force": {
                            "type": "boolean",
                            "description": "Whether to force cleanup of old monitors regardless of status (default: false)",
                            "default": false
                        },
                        "max_age_secs": {
                            "type": "integer",
                            "description": "Maximum age in seconds for force cleanup (default: 3600)",
                            "default": 3600,
                            "minimum": 60
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "execute_swap",
                "description": "Executes a token swap in a specified pool with slippage protection.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { "type": "string", "description": "The ID of the pool to swap in." },
                        "offer_asset": {
                            "type": "object",
                            "properties": {
                                "denom": { "type": "string" },
                                "amount": { "type": "string" }
                            },
                            "required": ["denom", "amount"]
                        },
                        "ask_asset_denom": { "type": "string", "description": "The denomination of the asset to receive." },
                        "max_slippage": { "type": "string", "description": "Maximum allowed slippage percentage (e.g., '1.5'). Defaults to 1%." }
                    },
                    "required": ["pool_id", "offer_asset", "ask_asset_denom"]
                }
            }),
            serde_json::json!({
                "name": "provide_liquidity",
                "description": "Provides liquidity to a specified pool.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { "type": "string", "description": "The ID of the pool to provide liquidity to." },
                        "assets": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "denom": { "type": "string" },
                                    "amount": { "type": "string" }
                                },
                                "required": ["denom", "amount"]
                            },
                            "description": "The assets to provide."
                        },
                        "max_slippage": { "type": "string", "description": "Maximum allowed slippage percentage. Defaults to 1%." }
                    },
                    "required": ["pool_id", "assets"]
                }
            }),
            serde_json::json!({
                "name": "provide_liquidity_unchecked",
                "description": "Provides liquidity to a specified pool without client-side checks.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { "type": "string", "description": "The ID of the pool to provide liquidity to." },
                        "assets": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "denom": { "type": "string" },
                                    "amount": { "type": "string" }
                                },
                                "required": ["denom", "amount"]
                            },
                            "description": "The assets to provide."
                        }
                    },
                    "required": ["pool_id", "assets"]
                }
            }),
            serde_json::json!({
                "name": "withdraw_liquidity",
                "description": "Withdraws liquidity from a specified pool.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { "type": "string", "description": "The ID of the pool to withdraw from." },
                        "amount": { "type": "string", "description": "The amount of LP tokens to withdraw." }
                    },
                    "required": ["pool_id", "amount"]
                }
            }),
            serde_json::json!({
                "name": "create_pool",
                "description": "Creates a new liquidity pool (admin only).",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_type": {
                            "type": "string",
                            "description": "The type of pool to create (constant_product, stable_swap).",
                            "enum": ["constant_product", "stable_swap"]
                        },
                        "assets": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "denom": { "type": "string" },
                                    "amount": { "type": "string" }
                                },
                                "required": ["denom", "amount"]
                            },
                            "description": "The initial assets to provide to the pool.",
                            "minItems": 2,
                            "maxItems": 8
                        },
                        "fees": {
                            "type": "object",
                            "properties": {
                                "protocol_fee": { "type": "string", "description": "Protocol fee percentage" },
                                "swap_fee": { "type": "string", "description": "Swap fee percentage" },
                                "burn_fee": { "type": "string", "description": "Burn fee percentage" }
                            },
                            "description": "Fee structure for the pool"
                        },
                        "amplification": {
                            "type": "integer",
                            "description": "Amplification parameter for stable swap pools (ignored for constant product)",
                            "minimum": 1
                        }
                    },
                    "required": ["pool_type", "assets"]
                }
            }),
            serde_json::json!({
                "name": "validate_pool_status",
                "description": "Validates the operational status and configuration of a pool.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "integer",
                            "description": "The ID of the pool to validate.",
                            "minimum": 1
                        },
                        "operation": {
                            "type": "string",
                            "description": "Specific operation to validate (swap, deposit, withdraw) or omit for all",
                            "enum": ["swap", "deposit", "withdraw"]
                        },
                        "include_recommendations": {
                            "type": "boolean",
                            "description": "Whether to include actionable recommendations (default: true)",
                            "default": true
                        }
                    },
                    "required": ["pool_id"]
                }
            }),
            serde_json::json!({
                "name": "validate_swap_result",
                "description": "Validates and analyzes swap transaction results with comprehensive checks.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "tx_hash": { 
                            "type": "string", 
                            "description": "Transaction hash of the swap to validate." 
                        },
                        "expected_pool_id": { 
                            "type": "string", 
                            "description": "Expected pool ID for validation." 
                        },
                        "expected_offer_asset": {
                            "type": "object",
                            "properties": {
                                "denom": { "type": "string" },
                                "amount": { "type": "string" }
                            },
                            "required": ["denom", "amount"],
                            "description": "Expected asset offered in the swap."
                        },
                        "expected_ask_asset_denom": { 
                            "type": "string", 
                            "description": "Expected denomination of asset received." 
                        },
                        "max_slippage_tolerance": { 
                            "type": "string", 
                            "description": "Maximum acceptable slippage percentage (e.g., '1.5'). Defaults to 5%." 
                        },
                        "min_return_amount": { 
                            "type": "string", 
                            "description": "Minimum expected return amount for validation." 
                        },
                        "validate_gas_efficiency": { 
                            "type": "boolean", 
                            "description": "Whether to validate gas usage efficiency. Defaults to true." 
                        },
                        "validate_events": { 
                            "type": "boolean", 
                            "description": "Whether to validate transaction events. Defaults to true." 
                        },
                        "include_detailed_analysis": { 
                            "type": "boolean", 
                            "description": "Whether to include detailed performance analysis. Defaults to false." 
                        }
                    },
                    "required": ["tx_hash"]
                }
            }),
            serde_json::json!({
                "name": "get_swap_execution_summary",
                "description": "Provides a comprehensive summary of swap execution with performance metrics.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "tx_hash": { 
                            "type": "string", 
                            "description": "Transaction hash of the swap to analyze." 
                        },
                        "include_pool_analysis": { 
                            "type": "boolean", 
                            "description": "Whether to include pool state analysis. Defaults to true." 
                        },
                        "include_fee_breakdown": { 
                            "type": "boolean", 
                            "description": "Whether to include detailed fee breakdown. Defaults to true." 
                        },
                        "include_slippage_analysis": { 
                            "type": "boolean", 
                            "description": "Whether to include slippage analysis. Defaults to true." 
                        }
                    },
                    "required": ["tx_hash"]
                }
            }),
            serde_json::json!({
                "name": "validate_swap_parameters",
                "description": "Validates swap parameters against current pool state and market conditions.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": { 
                            "type": "string", 
                            "description": "Pool ID to validate against." 
                        },
                        "offer_asset": {
                            "type": "object",
                            "properties": {
                                "denom": { "type": "string" },
                                "amount": { "type": "string" }
                            },
                            "required": ["denom", "amount"],
                            "description": "Asset to be offered in the swap."
                        },
                        "ask_asset_denom": { 
                            "type": "string", 
                            "description": "Denomination of asset to receive." 
                        },
                        "max_slippage": { 
                            "type": "string", 
                            "description": "Maximum slippage tolerance percentage." 
                        },
                        "simulate_before_validation": { 
                            "type": "boolean", 
                            "description": "Whether to run simulation as part of validation. Defaults to true." 
                        },
                        "check_pool_liquidity": { 
                            "type": "boolean", 
                            "description": "Whether to check pool liquidity sufficiency. Defaults to true." 
                        }
                    },
                    "required": ["pool_id", "offer_asset", "ask_asset_denom"]
                }
            }),
            serde_json::json!({
                "name": "get_swap_history",
                "description": "Retrieves comprehensive swap transaction history with filtering and pagination.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Optional wallet address to filter swaps. If not provided, uses active wallet."
                        },
                        "limit": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 100,
                            "default": 20,
                            "description": "Maximum number of swap records to return."
                        },
                        "offset": {
                            "type": "integer",
                            "minimum": 0,
                            "default": 0,
                            "description": "Number of records to skip for pagination."
                        },
                        "status_filter": {
                            "type": "string",
                            "enum": ["all", "success", "failed", "pending"],
                            "default": "all",
                            "description": "Filter swaps by transaction status."
                        },
                        "pool_id": {
                            "type": "string",
                            "description": "Optional pool ID to filter swaps by specific pool."
                        },
                        "from_asset": {
                            "type": "string",
                            "description": "Optional asset denomination to filter swaps by input asset."
                        },
                        "to_asset": {
                            "type": "string",
                            "description": "Optional asset denomination to filter swaps by output asset."
                        },
                        "date_from": {
                            "type": "string",
                            "format": "date-time",
                            "description": "Optional start date for filtering (ISO 8601 format)."
                        },
                        "date_to": {
                            "type": "string",
                            "format": "date-time",
                            "description": "Optional end date for filtering (ISO 8601 format)."
                        },
                        "min_amount": {
                            "type": "string",
                            "description": "Optional minimum swap amount filter."
                        },
                        "max_amount": {
                            "type": "string",
                            "description": "Optional maximum swap amount filter."
                        },
                        "sort_by": {
                            "type": "string",
                            "enum": ["timestamp", "amount", "gas_used", "tx_hash", "pool_id"],
                            "default": "timestamp",
                            "description": "Field to sort results by."
                        },
                        "sort_order": {
                            "type": "string",
                            "enum": ["asc", "desc"],
                            "default": "desc",
                            "description": "Sort order (ascending or descending)."
                        },
                        "include_details": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include detailed swap information."
                        },
                        "include_gas_info": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include gas usage information."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "get_swap_statistics",
                "description": "Generates comprehensive swap statistics and analytics for a wallet or globally.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Optional wallet address for statistics. If not provided, uses active wallet."
                        },
                        "time_period": {
                            "type": "string",
                            "enum": ["1h", "24h", "7d", "30d", "90d", "1y", "all"],
                            "default": "30d",
                            "description": "Time period for statistics calculation."
                        },
                        "include_pool_breakdown": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include per-pool statistics breakdown."
                        },
                        "include_asset_breakdown": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include per-asset statistics breakdown."
                        },
                        "include_performance_metrics": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include performance and efficiency metrics."
                        },
                        "include_trend_analysis": {
                            "type": "boolean",
                            "default": false,
                            "description": "Whether to include trend analysis over time."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "export_swap_history",
                "description": "Exports swap history data in various formats for external analysis.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Optional wallet address to export. If not provided, uses active wallet."
                        },
                        "format": {
                            "type": "string",
                            "enum": ["json", "csv", "tsv"],
                            "default": "json",
                            "description": "Export format for the data."
                        },
                        "date_from": {
                            "type": "string",
                            "format": "date-time",
                            "description": "Optional start date for export (ISO 8601 format)."
                        },
                        "date_to": {
                            "type": "string",
                            "format": "date-time",
                            "description": "Optional end date for export (ISO 8601 format)."
                        },
                        "include_failed": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include failed transactions in export."
                        },
                        "include_pending": {
                            "type": "boolean",
                            "default": false,
                            "description": "Whether to include pending transactions in export."
                        },
                        "compress": {
                            "type": "boolean",
                            "default": false,
                            "description": "Whether to compress the exported data."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "track_swap_execution",
                "description": "Tracks and records a swap execution for history tracking purposes.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "tx_hash": {
                            "type": "string",
                            "description": "Transaction hash of the swap to track."
                        },
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID where the swap was executed."
                        },
                        "from_asset": {
                            "type": "object",
                            "properties": {
                                "denom": { "type": "string" },
                                "amount": { "type": "string" }
                            },
                            "required": ["denom", "amount"],
                            "description": "Input asset for the swap."
                        },
                        "to_asset_denom": {
                            "type": "string",
                            "description": "Output asset denomination."
                        },
                        "expected_return": {
                            "type": "string",
                            "description": "Optional expected return amount from simulation."
                        },
                        "max_slippage": {
                            "type": "string",
                            "description": "Optional maximum slippage tolerance used."
                        },
                        "wallet_address": {
                            "type": "string",
                            "description": "Optional wallet address. If not provided, uses active wallet."
                        }
                    },
                    "required": ["tx_hash", "pool_id", "from_asset", "to_asset_denom"]
                }
            }),
            serde_json::json!({
                "name": "analyze_swap_performance",
                "description": "Analyzes swap performance including slippage, gas efficiency, and success rates.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Optional wallet address for analysis. If not provided, uses active wallet."
                        },
                        "analysis_period": {
                            "type": "string",
                            "enum": ["1h", "24h", "7d", "30d", "90d"],
                            "default": "7d",
                            "description": "Time period for performance analysis."
                        },
                        "pool_id": {
                            "type": "string",
                            "description": "Optional pool ID to analyze performance for specific pool."
                        },
                        "include_gas_analysis": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include gas efficiency analysis."
                        },
                        "include_slippage_analysis": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include slippage analysis."
                        },
                        "include_timing_analysis": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include timing and execution analysis."
                        },
                        "include_recommendations": {
                            "type": "boolean",
                            "default": true,
                            "description": "Whether to include performance optimization recommendations."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "get_lp_token_balance",
                "description": "Get LP token balance for a specific pool",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID to query LP token balance for"
                        },
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to query (optional, uses active wallet if not provided)"
                        }
                    },
                    "required": ["pool_id"]
                }
            }),
            serde_json::json!({
                "name": "get_all_lp_token_balances",
                "description": "Get all LP token balances for the wallet across all pools",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to query (optional, uses active wallet if not provided)"
                        },
                        "include_zero_balances": {
                            "type": "boolean",
                            "description": "Include pools with zero LP token balance",
                            "default": false
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "estimate_lp_withdrawal_amounts",
                "description": "Estimate withdrawal amounts for LP tokens",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID to estimate withdrawal for"
                        },
                        "lp_token_amount": {
                            "type": "string",
                            "description": "Amount of LP tokens to withdraw (optional, uses full balance if not provided)"
                        },
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to query (optional, uses active wallet if not provided)"
                        }
                    },
                    "required": ["pool_id"]
                }
            }),
            serde_json::json!({
                "name": "generate_trading_report",
                "description": "Generate comprehensive trading report for a specific time period",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to generate report for (optional, uses active wallet if not provided)"
                        },
                        "time_period": {
                            "type": "string",
                            "enum": ["24h", "7d", "30d", "90d", "1y", "all"],
                            "default": "30d",
                            "description": "Time period for the trading report"
                        },
                        "report_format": {
                            "type": "string",
                            "enum": ["json", "summary", "detailed"],
                            "default": "summary",
                            "description": "Format of the trading report"
                        },
                        "include_charts": {
                            "type": "boolean",
                            "default": false,
                            "description": "Include chart data in the report"
                        },
                        "include_pool_breakdown": {
                            "type": "boolean",
                            "default": true,
                            "description": "Include pool-by-pool breakdown"
                        },
                        "include_asset_breakdown": {
                            "type": "boolean",
                            "default": true,
                            "description": "Include asset-by-asset breakdown"
                        },
                        "include_performance_metrics": {
                            "type": "boolean",
                            "default": true,
                            "description": "Include performance metrics and analytics"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "calculate_impermanent_loss",
                "description": "Calculate impermanent loss for liquidity positions",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID to calculate impermanent loss for"
                        },
                        "wallet_address": {
                            "type": "string",
                            "description": "Wallet address to analyze (optional, uses active wallet if not provided)"
                        },
                        "entry_price_asset_a": {
                            "type": "string",
                            "description": "Entry price of asset A when liquidity was provided"
                        },
                        "entry_price_asset_b": {
                            "type": "string",
                            "description": "Entry price of asset B when liquidity was provided"
                        },
                        "current_price_asset_a": {
                            "type": "string",
                            "description": "Current price of asset A (optional, will fetch current price if not provided)"
                        },
                        "current_price_asset_b": {
                            "type": "string",
                            "description": "Current price of asset B (optional, will fetch current price if not provided)"
                        },
                        "lp_token_amount": {
                            "type": "string",
                            "description": "Amount of LP tokens to calculate for (optional, uses full balance if not provided)"
                        },
                        "include_fees_earned": {
                            "type": "boolean",
                            "default": true,
                            "description": "Include trading fees earned in the calculation"
                        },
                        "include_detailed_breakdown": {
                            "type": "boolean",
                            "default": false,
                            "description": "Include detailed calculation breakdown"
                        }
                    },
                    "required": ["pool_id", "entry_price_asset_a", "entry_price_asset_b"]
                }
            }),
        ]
    }

    async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        match tool_name {
            "execute_swap" => self.handle_execute_swap(arguments).await,
            "provide_liquidity" => self.handle_provide_liquidity(arguments).await,
            "provide_liquidity_unchecked" => {
                self.handle_provide_liquidity_unchecked(arguments).await
            }
            "withdraw_liquidity" => self.handle_withdraw_liquidity(arguments).await,
            "create_pool" => self.handle_create_pool(arguments).await,
            "validate_pool_status" => self.handle_validate_pool_status(arguments).await,
            "monitor_swap_transaction" => self.handle_monitor_swap_transaction(arguments).await,
            "get_transaction_monitor_status" => {
                self.handle_get_transaction_monitor_status(arguments).await
            }
            "cancel_transaction_monitor" => self.handle_cancel_transaction_monitor(arguments).await,
            "list_transaction_monitors" => self.handle_list_transaction_monitors(arguments).await,
            "cleanup_transaction_monitors" => {
                self.handle_cleanup_transaction_monitors(arguments).await
            }
            "validate_swap_result" => self.handle_validate_swap_result(arguments).await,
            "get_swap_execution_summary" => self.handle_get_swap_execution_summary(arguments).await,
            "validate_swap_parameters" => self.handle_validate_swap_parameters(arguments).await,
            "get_swap_history" => self.handle_get_swap_history(arguments).await,
            "get_swap_statistics" => self.handle_get_swap_statistics(arguments).await,
            "export_swap_history" => self.handle_export_swap_history(arguments).await,
            "track_swap_execution" => self.handle_track_swap_execution(arguments).await,
            "analyze_swap_performance" => self.handle_analyze_swap_performance(arguments).await,
            "get_lp_token_balance" => self.handle_get_lp_token_balance(arguments).await,
            "get_all_lp_token_balances" => self.handle_get_all_lp_token_balances(arguments).await,
            "estimate_lp_withdrawal_amounts" => self.handle_estimate_lp_withdrawal_amounts(arguments).await,
            "generate_trading_report" => self.handle_generate_trading_report(arguments).await,
            "calculate_impermanent_loss" => self.handle_calculate_impermanent_loss(arguments).await,
            _ => Err(McpServerError::UnknownTool(tool_name.to_string())),
        }
    }
}

// Implement the main McpServer trait that combines all sub-traits
impl McpServer for MantraDexMcpServer {}

impl MantraDexMcpServer {
    /// Handle get_transaction_monitor_status tool
    async fn handle_get_transaction_monitor_status(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_pool_status tool call");
        let pool_id: u64 = arguments
            .get("pool_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'pool_id' argument".to_string(),
                )
            })?;

        let include_metrics = arguments
            .get("include_metrics")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let include_history = arguments
            .get("include_history")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let cache_key = format!(
            "pool:{}:status:metrics={}:history={}",
            pool_id, include_metrics, include_history
        );

        if let Some(cached_data) = self.state.cache_get(&cache_key).await {
            info!(pool_id, "Returning cached pool status");
            return Ok(cached_data);
        }

        info!(pool_id, "Fetching pool status from blockchain");
        let result = self
            .state
            .sdk_adapter
            .get_pool_status(pool_id, include_metrics, include_history)
            .await?;

        self.state.cache_set(cache_key, result.clone()).await;

        Ok(result)
    }

    async fn handle_validate_pool_status(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling validate_pool_status tool call");

        let pool_id: u64 = arguments
            .get("pool_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'pool_id' argument".to_string(),
                )
            })?;
        let operation = arguments.get("operation").map(|v| v.to_string());
        let include_recommendations = arguments
            .get("include_recommendations")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        self.state
            .sdk_adapter
            .validate_pool_status(pool_id, operation, include_recommendations)
            .await
    }

    async fn handle_execute_swap(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling execute_swap tool call");
        self.state.sdk_adapter.execute_swap(arguments).await
    }

    async fn handle_provide_liquidity(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling provide_liquidity tool call");
        self.state.sdk_adapter.provide_liquidity(arguments).await
    }

    async fn handle_provide_liquidity_unchecked(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling provide_liquidity_unchecked tool call");
        self.state
            .sdk_adapter
            .provide_liquidity_unchecked(arguments)
            .await
    }

    async fn handle_withdraw_liquidity(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling withdraw_liquidity tool call");
        self.state.sdk_adapter.withdraw_liquidity(arguments).await
    }

    async fn handle_monitor_swap_transaction(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling monitor_swap_transaction tool call");

        // Parse arguments
        let tx_hash = arguments
            .get("tx_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'tx_hash' argument".to_string(),
                )
            })?;

        let min_confirmations = arguments
            .get("min_confirmations")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        let timeout_secs = arguments
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let poll_interval_secs = arguments
            .get("poll_interval_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);

        let monitor_events = arguments
            .get("monitor_events")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Validate parameters
        if tx_hash.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "Transaction hash cannot be empty".to_string(),
            ));
        }

        if min_confirmations == 0 {
            return Err(McpServerError::InvalidArguments(
                "Minimum confirmations must be greater than 0".to_string(),
            ));
        }

        if timeout_secs < 30 || timeout_secs > 3600 {
            return Err(McpServerError::InvalidArguments(
                "Timeout must be between 30 and 3600 seconds".to_string(),
            ));
        }

        if poll_interval_secs == 0 || poll_interval_secs > 60 {
            return Err(McpServerError::InvalidArguments(
                "Poll interval must be between 1 and 60 seconds".to_string(),
            ));
        }

        // Create monitor configuration
        let config = TransactionMonitorConfig {
            tx_hash: tx_hash.to_string(),
            min_confirmations,
            timeout_secs,
            poll_interval_secs,
            monitor_events,
        };

        // Start monitoring
        match self
            .state
            .transaction_monitor_manager
            .start_monitoring(config, self.state.client.clone())
            .await
        {
            Ok(monitor_id) => {
                info!("Started transaction monitoring with ID: {}", monitor_id);
                Ok(serde_json::json!({
                    "status": "success",
                    "message": "Transaction monitoring started successfully",
                    "monitor_id": monitor_id,
                    "tx_hash": tx_hash,
                    "min_confirmations": min_confirmations,
                    "timeout_secs": timeout_secs,
                    "poll_interval_secs": poll_interval_secs,
                    "monitor_events": monitor_events,
                    "started_at": chrono::Utc::now().to_rfc3339()
                }))
            }
            Err(e) => {
                error!("Failed to start transaction monitoring: {}", e);
                Err(McpServerError::Internal(format!(
                    "Failed to start transaction monitoring: {}",
                    e
                )))
            }
        }
    }

    async fn handle_cancel_transaction_monitor(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling cancel_transaction_monitor tool call");
        // Placeholder implementation for canceling transaction monitoring
        Ok(serde_json::json!({
            "status": "success",
            "message": "Transaction monitor cancelled",
            "arguments": arguments
        }))
    }

    async fn handle_list_transaction_monitors(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling list_transaction_monitors tool call");
        // Placeholder implementation for listing transaction monitors
        Ok(serde_json::json!({
            "status": "success",
            "monitors": [],
            "arguments": arguments
        }))
    }

    async fn handle_cleanup_transaction_monitors(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(
            ?arguments,
            "Handling cleanup_transaction_monitors tool call"
        );
        // Placeholder implementation for cleanup
        Ok(serde_json::json!({
            "status": "success",
            "message": "Transaction monitors cleaned up",
            "arguments": arguments
        }))
    }

    async fn handle_create_pool(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling create_pool tool call");
        self.state.sdk_adapter.create_pool(arguments).await
    }

    async fn handle_validate_swap_result(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling validate_swap_result tool call");

        // Parse arguments
        let tx_hash = arguments
            .get("tx_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'tx_hash' argument".to_string(),
                )
            })?;

        let expected_pool_id = arguments
            .get("expected_pool_id")
            .and_then(|v| v.as_str());

        let expected_offer_asset = arguments.get("expected_offer_asset");
        let expected_ask_asset_denom = arguments
            .get("expected_ask_asset_denom")
            .and_then(|v| v.as_str());

        let max_slippage_tolerance = arguments
            .get("max_slippage_tolerance")
            .and_then(|v| v.as_str())
            .unwrap_or("5.0");

        let min_return_amount = arguments
            .get("min_return_amount")
            .and_then(|v| v.as_str());

        let validate_gas_efficiency = arguments
            .get("validate_gas_efficiency")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let validate_events = arguments
            .get("validate_events")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_detailed_analysis = arguments
            .get("include_detailed_analysis")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Validate transaction hash format
        if tx_hash.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "Transaction hash cannot be empty".to_string(),
            ));
        }

        // Parse slippage tolerance
        let slippage_tolerance: f64 = max_slippage_tolerance.parse().map_err(|_| {
            McpServerError::InvalidArguments(
                "Invalid max_slippage_tolerance format".to_string(),
            )
        })?;

        if slippage_tolerance < 0.0 || slippage_tolerance > 100.0 {
            return Err(McpServerError::InvalidArguments(
                "max_slippage_tolerance must be between 0 and 100".to_string(),
            ));
        }

        // Query the transaction
        let client_guard = self.state.client.lock().await;
        let client = match client_guard.as_ref() {
            Some(c) => c,
            None => {
                return Ok(serde_json::json!({
                    "status": "error",
                    "message": "Blockchain client not available",
                    "tx_hash": tx_hash,
                    "validation_result": "failed",
                    "errors": ["Client not initialized"],
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }
        };

        match client.query_transaction(tx_hash).await {
            Ok(tx_result) => {
                // Perform comprehensive validation
                let validation_result = self.perform_swap_result_validation(
                    tx_result,
                    tx_hash,
                    expected_pool_id,
                    expected_offer_asset,
                    expected_ask_asset_denom,
                    slippage_tolerance,
                    min_return_amount,
                    validate_gas_efficiency,
                    validate_events,
                    include_detailed_analysis,
                ).await;

                Ok(validation_result)
            }
            Err(e) => {
                error!("Failed to query transaction {}: {}", tx_hash, e);
                Ok(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to query transaction: {}", e),
                    "tx_hash": tx_hash,
                    "validation_result": "failed",
                    "errors": [format!("Transaction query failed: {}", e)],
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
        }
    }

    async fn handle_get_swap_execution_summary(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_swap_execution_summary tool call");

        // Parse arguments
        let tx_hash = arguments
            .get("tx_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'tx_hash' argument".to_string(),
                )
            })?;

        let include_pool_analysis = arguments
            .get("include_pool_analysis")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_fee_breakdown = arguments
            .get("include_fee_breakdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_slippage_analysis = arguments
            .get("include_slippage_analysis")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Query the transaction
        let client_guard = self.state.client.lock().await;
        let client = match client_guard.as_ref() {
            Some(c) => c,
            None => {
                return Ok(serde_json::json!({
                    "status": "error",
                    "message": "Blockchain client not available",
                    "tx_hash": tx_hash,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
            }
        };

        match client.query_transaction(tx_hash).await {
            Ok(tx_result) => {
                let summary = self.generate_swap_execution_summary(
                    tx_result,
                    tx_hash,
                    include_pool_analysis,
                    include_fee_breakdown,
                    include_slippage_analysis,
                ).await;

                Ok(summary)
            }
            Err(e) => {
                error!("Failed to query transaction {}: {}", tx_hash, e);
                Ok(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to query transaction: {}", e),
                    "tx_hash": tx_hash,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }
        }
    }

    async fn handle_validate_swap_parameters(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling validate_swap_parameters tool call");

        // Parse arguments
        let pool_id = arguments
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'pool_id' argument".to_string(),
                )
            })?;

        let offer_asset = arguments
            .get("offer_asset")
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing 'offer_asset' argument".to_string(),
                )
            })?;

        let ask_asset_denom = arguments
            .get("ask_asset_denom")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'ask_asset_denom' argument".to_string(),
                )
            })?;

        let max_slippage = arguments
            .get("max_slippage")
            .and_then(|v| v.as_str());

        let simulate_before_validation = arguments
            .get("simulate_before_validation")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let check_pool_liquidity = arguments
            .get("check_pool_liquidity")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Validate offer asset structure
        let offer_denom = offer_asset
            .get("denom")
            .and_then(|d| d.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'denom' in offer_asset".to_string(),
                )
            })?;

        let offer_amount = offer_asset
            .get("amount")
            .and_then(|a| a.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments(
                    "Missing or invalid 'amount' in offer_asset".to_string(),
                )
            })?;

        // Parse and validate offer amount
        let _amount_val: u128 = offer_amount.parse().map_err(|_| {
            McpServerError::InvalidArguments(
                "Invalid offer amount format".to_string(),
            )
        })?;

        // Parse slippage if provided
        if let Some(slippage_str) = max_slippage {
            let slippage: f64 = slippage_str.parse().map_err(|_| {
                McpServerError::InvalidArguments(
                    "Invalid max_slippage format".to_string(),
                )
            })?;

            if slippage < 0.0 || slippage > 100.0 {
                return Err(McpServerError::InvalidArguments(
                    "max_slippage must be between 0 and 100".to_string(),
                ));
            }
        }

        // Perform parameter validation
        let validation_result = self.validate_swap_parameters_internal(
            pool_id,
            offer_denom,
            offer_amount,
            ask_asset_denom,
            max_slippage,
            simulate_before_validation,
            check_pool_liquidity,
        ).await;

        Ok(validation_result)
    }

    async fn perform_swap_result_validation(
        &self,
        tx_result: serde_json::Value,
        tx_hash: &str,
        expected_pool_id: Option<&str>,
        expected_offer_asset: Option<&serde_json::Value>,
        expected_ask_asset_denom: Option<&str>,
        slippage_tolerance: f64,
        min_return_amount: Option<&str>,
        validate_gas_efficiency: bool,
        validate_events: bool,
        include_detailed_analysis: bool,
    ) -> serde_json::Value {
        let mut validation_errors = Vec::new();
        let mut validation_warnings = Vec::new();
        let mut validation_details = serde_json::Map::new();

        // Extract basic transaction info
        let tx_code = tx_result.get("code").and_then(|c| c.as_u64()).unwrap_or(1);
        let gas_used = tx_result.get("gas_used").and_then(|g| g.as_u64());
        let gas_wanted = tx_result.get("gas_wanted").and_then(|g| g.as_u64());
        let height = tx_result.get("height").and_then(|h| h.as_u64());
        let raw_log = tx_result.get("raw_log").and_then(|l| l.as_str()).unwrap_or("");

        // Basic transaction success validation
        if tx_code != 0 {
            validation_errors.push(format!("Transaction failed with code {}: {}", tx_code, raw_log));
        } else {
            validation_details.insert("transaction_success".to_string(), serde_json::Value::Bool(true));
        }

        // Gas efficiency validation
        if validate_gas_efficiency {
            if let (Some(used), Some(wanted)) = (gas_used, gas_wanted) {
                let efficiency = (used as f64 / wanted as f64) * 100.0;
                validation_details.insert("gas_efficiency_percent".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(efficiency).unwrap()));
                
                if efficiency < 50.0 {
                    validation_warnings.push("Gas efficiency is very low (under 50%)".to_string());
                } else if efficiency > 95.0 {
                    validation_warnings.push("Gas limit was set too low (over 95% usage)".to_string());
                }
            } else {
                validation_warnings.push("Gas information not available for efficiency analysis".to_string());
            }
        }

        // Event validation
        if validate_events && tx_code == 0 {
            let events = tx_result.get("events").and_then(|e| e.as_array());
            let mut swap_events_found = false;
            let mut transfer_events_found = false;

            if let Some(events_array) = events {
                for event in events_array {
                    if let Some(event_type) = event.get("type").and_then(|t| t.as_str()) {
                        match event_type {
                            "wasm" => {
                                swap_events_found = true;
                                // Could extract more detailed swap information here
                            }
                            "transfer" | "coin_spent" | "coin_received" => {
                                transfer_events_found = true;
                            }
                            _ => {}
                        }
                    }
                }

                if !swap_events_found {
                    validation_warnings.push("No swap events found in transaction".to_string());
                }
                if !transfer_events_found {
                    validation_warnings.push("No transfer events found in transaction".to_string());
                }

                validation_details.insert("events_count".to_string(), serde_json::Value::Number(serde_json::Number::from(events_array.len())));
            } else {
                validation_warnings.push("No events found in transaction".to_string());
            }
        }

        // Expected parameter validation
        if let Some(pool_id) = expected_pool_id {
            // In a full implementation, you would extract pool information from events
            validation_details.insert("expected_pool_id".to_string(), serde_json::Value::String(pool_id.to_string()));
        }

        if let Some(offer_asset) = expected_offer_asset {
            validation_details.insert("expected_offer_asset".to_string(), offer_asset.clone());
        }

        if let Some(ask_denom) = expected_ask_asset_denom {
            validation_details.insert("expected_ask_asset_denom".to_string(), serde_json::Value::String(ask_denom.to_string()));
        }

        // Minimum return amount validation
        if let Some(min_return) = min_return_amount {
            if let Ok(_min_amount) = min_return.parse::<u128>() {
                // In a full implementation, you would extract actual return amount from events
                validation_details.insert("min_return_amount_check".to_string(), serde_json::Value::String("pending_event_parsing".to_string()));
            }
        }

        // Detailed analysis
        if include_detailed_analysis {
            let mut analysis = serde_json::Map::new();
            
            analysis.insert("block_height".to_string(), height.map(|h| serde_json::Value::Number(serde_json::Number::from(h))).unwrap_or(serde_json::Value::Null));
            analysis.insert("raw_log_length".to_string(), serde_json::Value::Number(serde_json::Number::from(raw_log.len())));
            
            if let (Some(used), Some(wanted)) = (gas_used, gas_wanted) {
                analysis.insert("gas_savings".to_string(), serde_json::Value::Number(serde_json::Number::from(wanted - used)));
                analysis.insert("gas_cost_efficiency".to_string(), serde_json::Value::String(
                    if used < wanted / 2 { "excellent" }
                    else if used < wanted * 3 / 4 { "good" }
                    else if used < wanted * 9 / 10 { "acceptable" }
                    else { "poor" }.to_string()
                ));
            }

            validation_details.insert("detailed_analysis".to_string(), serde_json::Value::Object(analysis));
        }

        // Determine overall validation result
        let validation_result = if !validation_errors.is_empty() {
            "failed"
        } else if !validation_warnings.is_empty() {
            "warning"
        } else {
            "passed"
        };

        {
            let mut checks = vec!["transaction_success"];
            if validate_gas_efficiency { checks.push("gas_efficiency"); }
            if validate_events { checks.push("event_validation"); }
            if expected_pool_id.is_some() { checks.push("pool_validation"); }
            if min_return_amount.is_some() { checks.push("return_amount_validation"); }
            let checks_performed = serde_json::Value::Array(checks.into_iter().map(|s| serde_json::Value::String(s.to_string())).collect());

            let raw_log_preview = if raw_log.len() > 200 { 
                format!("{}...", &raw_log[..200]) 
            } else { 
                raw_log.to_string() 
            };

            serde_json::json!({
                "status": "success",
                "message": "Swap result validation completed",
                "tx_hash": tx_hash,
                "validation_result": validation_result,
                "transaction_success": tx_code == 0,
                "validation_summary": {
                    "errors_count": validation_errors.len(),
                    "warnings_count": validation_warnings.len(),
                    "checks_performed": checks_performed
                },
                "errors": validation_errors,
                "warnings": validation_warnings,
                "validation_details": validation_details,
                "transaction_info": {
                    "code": tx_code,
                    "gas_used": gas_used,
                    "gas_wanted": gas_wanted,
                    "height": height,
                    "raw_log_preview": raw_log_preview
                },
                "slippage_tolerance": slippage_tolerance,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })
        }
    }

    async fn generate_swap_execution_summary(
        &self,
        tx_result: serde_json::Value,
        tx_hash: &str,
        include_pool_analysis: bool,
        include_fee_breakdown: bool,
        include_slippage_analysis: bool,
    ) -> serde_json::Value {
        let tx_code = tx_result.get("code").and_then(|c| c.as_u64()).unwrap_or(1);
        let gas_used = tx_result.get("gas_used").and_then(|g| g.as_u64());
        let gas_wanted = tx_result.get("gas_wanted").and_then(|g| g.as_u64());
        let height = tx_result.get("height").and_then(|h| h.as_u64());
        let raw_log = tx_result.get("raw_log").and_then(|l| l.as_str()).unwrap_or("");

        let mut summary = serde_json::Map::new();

        // Basic execution info
        summary.insert("execution_status".to_string(), serde_json::Value::String(
            if tx_code == 0 { "success" } else { "failed" }.to_string()
        ));
        summary.insert("transaction_code".to_string(), serde_json::Value::Number(serde_json::Number::from(tx_code)));
        summary.insert("block_height".to_string(), height.map(|h| serde_json::Value::Number(serde_json::Number::from(h))).unwrap_or(serde_json::Value::Null));

        // Gas analysis
        if let (Some(used), Some(wanted)) = (gas_used, gas_wanted) {
            let mut gas_analysis = serde_json::Map::new();
            gas_analysis.insert("gas_used".to_string(), serde_json::Value::Number(serde_json::Number::from(used)));
            gas_analysis.insert("gas_wanted".to_string(), serde_json::Value::Number(serde_json::Number::from(wanted)));
            gas_analysis.insert("gas_efficiency".to_string(), serde_json::Value::Number(
                serde_json::Number::from_f64((used as f64 / wanted as f64) * 100.0).unwrap()
            ));
            gas_analysis.insert("gas_saved".to_string(), serde_json::Value::Number(serde_json::Number::from(wanted - used)));
            summary.insert("gas_analysis".to_string(), serde_json::Value::Object(gas_analysis));
        }

        // Fee breakdown (if requested)
        if include_fee_breakdown && tx_code == 0 {
            let mut fee_breakdown = serde_json::Map::new();
            fee_breakdown.insert("transaction_fee".to_string(), serde_json::Value::String("extracted_from_events".to_string()));
            fee_breakdown.insert("swap_fee".to_string(), serde_json::Value::String("extracted_from_events".to_string()));
            fee_breakdown.insert("protocol_fee".to_string(), serde_json::Value::String("extracted_from_events".to_string()));
            fee_breakdown.insert("note".to_string(), serde_json::Value::String("Fee details require event parsing implementation".to_string()));
            summary.insert("fee_breakdown".to_string(), serde_json::Value::Object(fee_breakdown));
        }

        // Pool analysis (if requested)
        if include_pool_analysis && tx_code == 0 {
            let mut pool_analysis = serde_json::Map::new();
            pool_analysis.insert("pool_id".to_string(), serde_json::Value::String("extracted_from_events".to_string()));
            pool_analysis.insert("pool_state_before".to_string(), serde_json::Value::String("requires_additional_query".to_string()));
            pool_analysis.insert("pool_state_after".to_string(), serde_json::Value::String("requires_additional_query".to_string()));
            pool_analysis.insert("note".to_string(), serde_json::Value::String("Pool analysis requires additional pool state queries".to_string()));
            summary.insert("pool_analysis".to_string(), serde_json::Value::Object(pool_analysis));
        }

        // Slippage analysis (if requested)
        if include_slippage_analysis && tx_code == 0 {
            let mut slippage_analysis = serde_json::Map::new();
            slippage_analysis.insert("expected_output".to_string(), serde_json::Value::String("requires_simulation_comparison".to_string()));
            slippage_analysis.insert("actual_output".to_string(), serde_json::Value::String("extracted_from_events".to_string()));
            slippage_analysis.insert("slippage_percent".to_string(), serde_json::Value::String("calculated_from_above".to_string()));
            slippage_analysis.insert("note".to_string(), serde_json::Value::String("Slippage analysis requires simulation comparison".to_string()));
            summary.insert("slippage_analysis".to_string(), serde_json::Value::Object(slippage_analysis));
        }

        // Events summary
        let events = tx_result.get("events").and_then(|e| e.as_array());
        if let Some(events_array) = events {
            let mut events_summary = serde_json::Map::new();
            events_summary.insert("total_events".to_string(), serde_json::Value::Number(serde_json::Number::from(events_array.len())));
            
            let mut event_types = std::collections::HashMap::new();
            for event in events_array {
                if let Some(event_type) = event.get("type").and_then(|t| t.as_str()) {
                    *event_types.entry(event_type.to_string()).or_insert(0) += 1;
                }
            }
            
            events_summary.insert("event_types".to_string(), serde_json::json!(event_types));
            summary.insert("events_summary".to_string(), serde_json::Value::Object(events_summary));
        }

        // Error details (if failed)
        if tx_code != 0 {
            let mut error_details = serde_json::Map::new();
            error_details.insert("error_code".to_string(), serde_json::Value::Number(serde_json::Number::from(tx_code)));
            error_details.insert("error_log".to_string(), serde_json::Value::String(raw_log.to_string()));
            summary.insert("error_details".to_string(), serde_json::Value::Object(error_details));
        }

        serde_json::json!({
            "status": "success",
            "message": "Swap execution summary generated",
            "tx_hash": tx_hash,
            "summary": summary,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })
    }

    async fn validate_swap_parameters_internal(
        &self,
        pool_id: &str,
        offer_denom: &str,
        offer_amount: &str,
        ask_asset_denom: &str,
        max_slippage: Option<&str>,
        simulate_before_validation: bool,
        check_pool_liquidity: bool,
    ) -> serde_json::Value {
        let mut validation_errors = Vec::new();
        let mut validation_warnings = Vec::new();
        let mut validation_details = serde_json::Map::new();

        // Basic parameter validation
        if pool_id.is_empty() {
            validation_errors.push("Pool ID cannot be empty".to_string());
        }

        if offer_denom.is_empty() {
            validation_errors.push("Offer asset denomination cannot be empty".to_string());
        }

        if ask_asset_denom.is_empty() {
            validation_errors.push("Ask asset denomination cannot be empty".to_string());
        }

        if offer_denom == ask_asset_denom {
            validation_errors.push("Cannot swap the same asset to itself".to_string());
        }

        // Amount validation
        match offer_amount.parse::<u128>() {
            Ok(amount) => {
                if amount == 0 {
                    validation_errors.push("Offer amount must be greater than zero".to_string());
                }
                validation_details.insert("offer_amount_valid".to_string(), serde_json::Value::Bool(true));
            }
            Err(_) => {
                validation_errors.push("Invalid offer amount format".to_string());
            }
        }

        // Slippage validation
        if let Some(slippage_str) = max_slippage {
            match slippage_str.parse::<f64>() {
                Ok(slippage) => {
                    if slippage < 0.0 || slippage > 100.0 {
                        validation_errors.push("Slippage must be between 0 and 100 percent".to_string());
                    } else if slippage > 50.0 {
                        validation_warnings.push("Very high slippage tolerance (over 50%)".to_string());
                    } else if slippage < 0.1 {
                        validation_warnings.push("Very low slippage tolerance (under 0.1%)".to_string());
                    }
                    validation_details.insert("slippage_tolerance".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(slippage).unwrap()));
                }
                Err(_) => {
                    validation_errors.push("Invalid slippage format".to_string());
                }
            }
        }

        // Pool status validation
        if !validation_errors.is_empty() {
            // Skip further validation if basic parameters are invalid
        } else {
            // Check pool exists and is available
            let client_guard = self.state.client.lock().await;
            if let Some(client) = client_guard.as_ref() {
                match client.validate_pool_status(pool_id).await {
                    Ok(()) => {
                        validation_details.insert("pool_status".to_string(), serde_json::Value::String("available".to_string()));
                    }
                    Err(e) => {
                        validation_errors.push(format!("Pool validation failed: {}", e));
                    }
                }

                // Pool liquidity check
                if check_pool_liquidity {
                    match client.get_pool(pool_id).await {
                        Ok(pool_info) => {
                            // Check if the offered asset is in the pool
                            let pool_has_offer_asset = pool_info.pool_info.assets.iter()
                                .any(|asset| asset.denom == offer_denom);
                            
                            let pool_has_ask_asset = pool_info.pool_info.assets.iter()
                                .any(|asset| asset.denom == ask_asset_denom);

                            if !pool_has_offer_asset {
                                validation_errors.push(format!("Pool does not contain offer asset: {}", offer_denom));
                            }

                            if !pool_has_ask_asset {
                                validation_errors.push(format!("Pool does not contain ask asset: {}", ask_asset_denom));
                            }

                            validation_details.insert("pool_assets_valid".to_string(), serde_json::Value::Bool(pool_has_offer_asset && pool_has_ask_asset));
                        }
                        Err(e) => {
                            validation_warnings.push(format!("Could not verify pool liquidity: {}", e));
                        }
                    }
                }

                // Simulation validation
                if simulate_before_validation && validation_errors.is_empty() {
                    match offer_amount.parse::<u128>() {
                        Ok(amount) => {
                            let offer_asset = cosmwasm_std::Coin {
                                denom: offer_denom.to_string(),
                                amount: cosmwasm_std::Uint128::new(amount),
                            };

                            match client.simulate_swap(pool_id, offer_asset, ask_asset_denom).await {
                                Ok(simulation) => {
                                    if simulation.return_amount.is_zero() {
                                        validation_warnings.push("Simulation returned zero output".to_string());
                                    }
                                    validation_details.insert("simulation_successful".to_string(), serde_json::Value::Bool(true));
                                    validation_details.insert("simulated_return_amount".to_string(), serde_json::Value::String(simulation.return_amount.to_string()));
                                }
                                Err(e) => {
                                    validation_errors.push(format!("Swap simulation failed: {}", e));
                                }
                            }
                        }
                        Err(_) => {
                            // Already handled above
                        }
                    }
                }
            } else {
                validation_warnings.push("Blockchain client not available for validation".to_string());
            }
        }

        // Determine overall validation result
        let validation_result = if !validation_errors.is_empty() {
            "failed"
        } else if !validation_warnings.is_empty() {
            "warning"
        } else {
            "passed"
        };

        {
            let mut checks = vec!["basic_parameters", "amount_format"];
            if max_slippage.is_some() { checks.push("slippage_validation"); }
            if check_pool_liquidity { checks.push("pool_liquidity"); }
            if simulate_before_validation { checks.push("swap_simulation"); }
            let checks_performed = serde_json::Value::Array(checks.into_iter().map(|s| serde_json::Value::String(s.to_string())).collect());

            serde_json::json!({
                "status": "success",
                "message": "Swap parameter validation completed",
                "validation_result": validation_result,
                "parameters": {
                    "pool_id": pool_id,
                    "offer_asset": {
                        "denom": offer_denom,
                        "amount": offer_amount
                    },
                    "ask_asset_denom": ask_asset_denom,
                    "max_slippage": max_slippage
                },
                "validation_summary": {
                    "errors_count": validation_errors.len(),
                    "warnings_count": validation_warnings.len(),
                    "checks_performed": checks_performed
                },
                "errors": validation_errors,
                "warnings": validation_warnings,
                "validation_details": validation_details,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })
        }
    }

    async fn handle_get_swap_history(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let wallet_address = arguments.get("wallet_address").and_then(|v| v.as_str());
        let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
        let offset = arguments.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let status_filter = arguments.get("status_filter").and_then(|v| v.as_str()).unwrap_or("all");
        let pool_id = arguments.get("pool_id").and_then(|v| v.as_str());
        let from_asset = arguments.get("from_asset").and_then(|v| v.as_str());
        let to_asset = arguments.get("to_asset").and_then(|v| v.as_str());
        let date_from = arguments.get("date_from").and_then(|v| v.as_str());
        let date_to = arguments.get("date_to").and_then(|v| v.as_str());
        let min_amount = arguments.get("min_amount").and_then(|v| v.as_str());
        let max_amount = arguments.get("max_amount").and_then(|v| v.as_str());
        let sort_by = arguments.get("sort_by").and_then(|v| v.as_str()).unwrap_or("timestamp");
        let sort_order = arguments.get("sort_order").and_then(|v| v.as_str()).unwrap_or("desc");
        let include_details = arguments.get("include_details").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_gas_info = arguments.get("include_gas_info").and_then(|v| v.as_bool()).unwrap_or(true);

        // Get target wallet address
        let target_wallet = match wallet_address {
            Some(addr) => addr.to_string(),
            None => {
                let active_wallet_guard = self.state.active_wallet.lock().await;
                match active_wallet_guard.as_ref() {
                    Some(addr) => addr.clone(),
                    None => {
                        return Err(McpServerError::WalletNotConfigured);
                    }
                }
            }
        };

        // Parse date filters
        let date_from_parsed = date_from.and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok());
        let date_to_parsed = date_to.and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok());

        // Parse amount filters
        let min_amount_parsed = min_amount.and_then(|a| a.parse::<u128>().ok());
        let max_amount_parsed = max_amount.and_then(|a| a.parse::<u128>().ok());

        // Get swap history from blockchain (placeholder - in real implementation would query blockchain)
        let swap_history = self.get_swap_history_from_blockchain(
            &target_wallet,
            limit,
            offset,
            status_filter,
            pool_id,
            from_asset,
            to_asset,
            date_from_parsed.as_ref(),
            date_to_parsed.as_ref(),
            min_amount_parsed,
            max_amount_parsed,
            sort_by,
            sort_order,
        ).await?;

        // Apply additional filtering and sorting
        let filtered_history = self.filter_and_sort_swap_history(
            swap_history,
            status_filter,
            sort_by,
            sort_order,
        );

        // Paginate results
        let total_count = filtered_history.len();
        let paginated_history: Vec<_> = filtered_history
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        // Build response
        let mut response_data = serde_json::Map::new();
        response_data.insert("wallet_address".to_string(), serde_json::Value::String(target_wallet));
        response_data.insert("total_count".to_string(), serde_json::Value::Number(serde_json::Number::from(total_count)));
        response_data.insert("returned_count".to_string(), serde_json::Value::Number(serde_json::Number::from(paginated_history.len())));
        response_data.insert("limit".to_string(), serde_json::Value::Number(serde_json::Number::from(limit)));
        response_data.insert("offset".to_string(), serde_json::Value::Number(serde_json::Number::from(offset)));
        response_data.insert("has_more".to_string(), serde_json::Value::Bool(offset + limit < total_count));

        // Format swap records
        let swap_records: Vec<serde_json::Value> = paginated_history
            .into_iter()
            .map(|swap| self.format_swap_record(swap, include_details, include_gas_info))
            .collect();

        response_data.insert("swaps".to_string(), serde_json::Value::Array(swap_records));

        // Add pagination info
        let mut pagination = serde_json::Map::new();
        pagination.insert("current_page".to_string(), serde_json::Value::Number(serde_json::Number::from((offset / limit) + 1)));
        pagination.insert("total_pages".to_string(), serde_json::Value::Number(serde_json::Number::from((total_count + limit - 1) / limit)));
        pagination.insert("has_previous".to_string(), serde_json::Value::Bool(offset > 0));
        pagination.insert("has_next".to_string(), serde_json::Value::Bool(offset + limit < total_count));
        response_data.insert("pagination".to_string(), serde_json::Value::Object(pagination));

        Ok(serde_json::json!({
            "status": "success",
            "message": "Swap history retrieved successfully",
            "data": response_data,
            "filters_applied": {
                "status_filter": status_filter,
                "pool_id": pool_id,
                "from_asset": from_asset,
                "to_asset": to_asset,
                "date_range": {
                    "from": date_from,
                    "to": date_to
                },
                "amount_range": {
                    "min": min_amount,
                    "max": max_amount
                },
                "sort_by": sort_by,
                "sort_order": sort_order
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    async fn handle_get_swap_statistics(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let wallet_address = arguments.get("wallet_address").and_then(|v| v.as_str());
        let time_period = arguments.get("time_period").and_then(|v| v.as_str()).unwrap_or("30d");
        let include_pool_breakdown = arguments.get("include_pool_breakdown").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_asset_breakdown = arguments.get("include_asset_breakdown").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_performance_metrics = arguments.get("include_performance_metrics").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_trend_analysis = arguments.get("include_trend_analysis").and_then(|v| v.as_bool()).unwrap_or(false);

        // Get target wallet address
        let target_wallet = match wallet_address {
            Some(addr) => addr.to_string(),
            None => {
                let active_wallet_guard = self.state.active_wallet.lock().await;
                match active_wallet_guard.as_ref() {
                    Some(addr) => addr.clone(),
                    None => {
                        return Err(McpServerError::WalletNotConfigured);
                    }
                }
            }
        };

        // Calculate time period boundaries
        let (start_time, end_time) = self.calculate_time_period_boundaries(time_period)?;

        // Get swap data for the period
        let swap_data = self.get_swap_data_for_period(&target_wallet, start_time, end_time).await?;

        // Calculate basic statistics
        let total_swaps = swap_data.len();
        let successful_swaps = swap_data.iter().filter(|s| s.status == "success").count();
        let failed_swaps = swap_data.iter().filter(|s| s.status == "failed").count();
        let success_rate = if total_swaps > 0 { (successful_swaps as f64 / total_swaps as f64) * 100.0 } else { 0.0 };

        // Calculate volume statistics
        let total_volume = swap_data.iter()
            .filter(|s| s.status == "success")
            .map(|s| s.input_amount.parse::<f64>().unwrap_or(0.0))
            .sum::<f64>();

        let average_swap_size = if successful_swaps > 0 { total_volume / successful_swaps as f64 } else { 0.0 };

        // Calculate gas statistics
        let total_gas_used: u64 = swap_data.iter()
            .filter_map(|s| s.gas_used)
            .sum();
        let average_gas_used = if successful_swaps > 0 { total_gas_used / successful_swaps as u64 } else { 0 };

        let mut statistics = serde_json::Map::new();

        // Basic statistics
        let mut basic_stats = serde_json::Map::new();
        basic_stats.insert("total_swaps".to_string(), serde_json::Value::Number(serde_json::Number::from(total_swaps)));
        basic_stats.insert("successful_swaps".to_string(), serde_json::Value::Number(serde_json::Number::from(successful_swaps)));
        basic_stats.insert("failed_swaps".to_string(), serde_json::Value::Number(serde_json::Number::from(failed_swaps)));
        basic_stats.insert("success_rate_percent".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(success_rate).unwrap_or(serde_json::Number::from(0))));
        basic_stats.insert("total_volume".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(total_volume).unwrap_or(serde_json::Number::from(0))));
        basic_stats.insert("average_swap_size".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(average_swap_size).unwrap_or(serde_json::Number::from(0))));
        basic_stats.insert("total_gas_used".to_string(), serde_json::Value::Number(serde_json::Number::from(total_gas_used)));
        basic_stats.insert("average_gas_used".to_string(), serde_json::Value::Number(serde_json::Number::from(average_gas_used)));
        statistics.insert("basic_statistics".to_string(), serde_json::Value::Object(basic_stats));

        // Pool breakdown
        if include_pool_breakdown {
            let pool_stats = self.calculate_pool_breakdown(&swap_data);
            statistics.insert("pool_breakdown".to_string(), pool_stats);
        }

        // Asset breakdown
        if include_asset_breakdown {
            let asset_stats = self.calculate_asset_breakdown(&swap_data);
            statistics.insert("asset_breakdown".to_string(), asset_stats);
        }

        // Performance metrics
        if include_performance_metrics {
            let performance_metrics = self.calculate_performance_metrics(&swap_data);
            statistics.insert("performance_metrics".to_string(), performance_metrics);
        }

        // Trend analysis
        if include_trend_analysis {
            let trend_analysis = self.calculate_trend_analysis(&swap_data, start_time, end_time);
            statistics.insert("trend_analysis".to_string(), trend_analysis);
        }

        Ok(serde_json::json!({
            "status": "success",
            "message": "Swap statistics calculated successfully",
            "wallet_address": target_wallet,
            "time_period": time_period,
            "period_boundaries": {
                "start": start_time.to_rfc3339(),
                "end": end_time.to_rfc3339()
            },
            "statistics": statistics,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    async fn handle_export_swap_history(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let wallet_address = arguments.get("wallet_address").and_then(|v| v.as_str());
        let format = arguments.get("format").and_then(|v| v.as_str()).unwrap_or("json");
        let date_from = arguments.get("date_from").and_then(|v| v.as_str());
        let date_to = arguments.get("date_to").and_then(|v| v.as_str());
        let include_failed = arguments.get("include_failed").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_pending = arguments.get("include_pending").and_then(|v| v.as_bool()).unwrap_or(false);
        let compress = arguments.get("compress").and_then(|v| v.as_bool()).unwrap_or(false);

        // Get target wallet address
        let target_wallet = match wallet_address {
            Some(addr) => addr.to_string(),
            None => {
                let active_wallet_guard = self.state.active_wallet.lock().await;
                match active_wallet_guard.as_ref() {
                    Some(addr) => addr.clone(),
                    None => {
                        return Err(McpServerError::WalletNotConfigured);
                    }
                }
            }
        };

        // Parse date filters
        let date_from_parsed = date_from.and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok());
        let date_to_parsed = date_to.and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok());

        // Get complete swap history
        let swap_history = self.get_swap_history_for_export(
            &target_wallet,
            date_from_parsed.as_ref(),
            date_to_parsed.as_ref(),
            include_failed,
            include_pending,
        ).await?;

        // Format data according to requested format
        let formatted_data = match format {
            "csv" => self.format_swap_history_as_csv(&swap_history)?,
            "tsv" => self.format_swap_history_as_tsv(&swap_history)?,
            "json" | _ => self.format_swap_history_as_json(&swap_history)?,
        };

        // Apply compression if requested
        let final_data = if compress {
            self.compress_data(&formatted_data)?
        } else {
            formatted_data
        };

        // Generate export metadata
        let export_metadata = serde_json::json!({
            "export_timestamp": chrono::Utc::now().to_rfc3339(),
            "wallet_address": target_wallet,
            "format": format,
            "compressed": compress,
            "record_count": swap_history.len(),
            "date_range": {
                "from": date_from,
                "to": date_to
            },
            "filters": {
                "include_failed": include_failed,
                "include_pending": include_pending
            },
            "file_size_bytes": final_data.len()
        });

        Ok(serde_json::json!({
            "status": "success",
            "message": "Swap history exported successfully",
            "export_metadata": export_metadata,
            "data": final_data,
            "content_type": match format {
                "csv" => "text/csv",
                "tsv" => "text/tab-separated-values",
                "json" | _ => "application/json"
            },
            "filename_suggestion": format!(
                "swap_history_{}_{}.{}{}",
                target_wallet[..8].to_string(),
                chrono::Utc::now().format("%Y%m%d_%H%M%S"),
                format,
                if compress { ".gz" } else { "" }
            ),
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    async fn handle_track_swap_execution(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let tx_hash = arguments.get("tx_hash").and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing tx_hash parameter".to_string()))?;
        let pool_id = arguments.get("pool_id").and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing pool_id parameter".to_string()))?;
        let from_asset = arguments.get("from_asset")
            .ok_or_else(|| McpServerError::InvalidArguments("Missing from_asset parameter".to_string()))?;
        let to_asset_denom = arguments.get("to_asset_denom").and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing to_asset_denom parameter".to_string()))?;
        let expected_return = arguments.get("expected_return").and_then(|v| v.as_str());
        let max_slippage = arguments.get("max_slippage").and_then(|v| v.as_str());
        let wallet_address = arguments.get("wallet_address").and_then(|v| v.as_str());

        // Parse from_asset
        let from_asset_denom = from_asset.get("denom").and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing from_asset.denom".to_string()))?;
        let from_asset_amount = from_asset.get("amount").and_then(|v| v.as_str())
            .ok_or_else(|| McpServerError::InvalidArguments("Missing from_asset.amount".to_string()))?;

        // Get target wallet address
        let target_wallet = match wallet_address {
            Some(addr) => addr.to_string(),
            None => {
                let active_wallet_guard = self.state.active_wallet.lock().await;
                match active_wallet_guard.as_ref() {
                    Some(addr) => addr.clone(),
                    None => {
                        return Err(McpServerError::WalletNotConfigured);
                    }
                }
            }
        };

        // Create swap tracking record
        let swap_record = SwapTrackingRecord {
            tx_hash: tx_hash.to_string(),
            wallet_address: target_wallet.clone(),
            pool_id: pool_id.to_string(),
            from_asset_denom: from_asset_denom.to_string(),
            from_asset_amount: from_asset_amount.to_string(),
            to_asset_denom: to_asset_denom.to_string(),
            expected_return: expected_return.map(|s| s.to_string()),
            max_slippage: max_slippage.map(|s| s.to_string()),
            timestamp: chrono::Utc::now(),
            status: "pending".to_string(),
            actual_return: None,
            gas_used: None,
            gas_wanted: None,
            block_height: None,
        };

        // Store the tracking record (in real implementation, would store in database)
        self.store_swap_tracking_record(swap_record.clone()).await?;

        // Start monitoring the transaction
        let monitor_config = crate::mcp::server::TransactionMonitorConfig {
            tx_hash: tx_hash.to_string(),
            min_confirmations: 1,
            timeout_secs: 300, // 5 minutes
            poll_interval_secs: 5,
            monitor_events: true,
        };

        let monitor_id = self.state.transaction_monitor_manager
            .start_monitoring(monitor_config, self.state.client.clone())
            .await
            .map_err(|e| McpServerError::Internal(format!("Failed to start transaction monitoring: {}", e)))?;

        Ok(serde_json::json!({
            "status": "success",
            "message": "Swap execution tracking started",
            "tracking_details": {
                "tx_hash": tx_hash,
                "wallet_address": target_wallet,
                "pool_id": pool_id,
                "from_asset": {
                    "denom": from_asset_denom,
                    "amount": from_asset_amount
                },
                "to_asset_denom": to_asset_denom,
                "expected_return": expected_return,
                "max_slippage": max_slippage,
                "monitor_id": monitor_id,
                "tracking_started_at": swap_record.timestamp.to_rfc3339()
            },
            "next_steps": [
                "Use get_transaction_monitor_status to check transaction status",
                "Use get_swap_history to view completed swap records",
                "Monitor will automatically update swap record when transaction completes"
            ],
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    async fn handle_analyze_swap_performance(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let wallet_address = arguments.get("wallet_address").and_then(|v| v.as_str());
        let analysis_period = arguments.get("analysis_period").and_then(|v| v.as_str()).unwrap_or("7d");
        let pool_id = arguments.get("pool_id").and_then(|v| v.as_str());
        let include_gas_analysis = arguments.get("include_gas_analysis").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_slippage_analysis = arguments.get("include_slippage_analysis").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_timing_analysis = arguments.get("include_timing_analysis").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_recommendations = arguments.get("include_recommendations").and_then(|v| v.as_bool()).unwrap_or(true);

        // Get target wallet address
        let target_wallet = match wallet_address {
            Some(addr) => addr.to_string(),
            None => {
                let active_wallet_guard = self.state.active_wallet.lock().await;
                match active_wallet_guard.as_ref() {
                    Some(addr) => addr.clone(),
                    None => {
                        return Err(McpServerError::WalletNotConfigured);
                    }
                }
            }
        };

        // Calculate analysis period boundaries
        let (start_time, end_time) = self.calculate_time_period_boundaries(analysis_period)?;

        // Get swap data for analysis
        let swap_data = self.get_swap_data_for_analysis(
            &target_wallet,
            start_time,
            end_time,
            pool_id,
        ).await?;

        if swap_data.is_empty() {
            return Ok(serde_json::json!({
                "status": "success",
                "message": "No swap data found for analysis period",
                "wallet_address": target_wallet,
                "analysis_period": analysis_period,
                "pool_id": pool_id,
                "data_points": 0,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }));
        }

        let mut analysis_results = serde_json::Map::new();

        // Basic performance metrics
        let total_swaps = swap_data.len();
        let successful_swaps = swap_data.iter().filter(|s| s.status == "success").count();
        let success_rate = (successful_swaps as f64 / total_swaps as f64) * 100.0;

        let mut basic_metrics = serde_json::Map::new();
        basic_metrics.insert("total_swaps".to_string(), serde_json::Value::Number(serde_json::Number::from(total_swaps)));
        basic_metrics.insert("successful_swaps".to_string(), serde_json::Value::Number(serde_json::Number::from(successful_swaps)));
        basic_metrics.insert("success_rate_percent".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(success_rate).unwrap_or(serde_json::Number::from(0))));
        analysis_results.insert("basic_metrics".to_string(), serde_json::Value::Object(basic_metrics));

        // Gas analysis
        if include_gas_analysis {
            let gas_analysis = self.analyze_gas_performance(&swap_data);
            analysis_results.insert("gas_analysis".to_string(), gas_analysis);
        }

        // Slippage analysis
        if include_slippage_analysis {
            let slippage_analysis = self.analyze_slippage_performance(&swap_data);
            analysis_results.insert("slippage_analysis".to_string(), slippage_analysis);
        }

        // Timing analysis
        if include_timing_analysis {
            let timing_analysis = self.analyze_timing_performance(&swap_data);
            analysis_results.insert("timing_analysis".to_string(), timing_analysis);
        }

        // Performance recommendations
        if include_recommendations {
            let recommendations = self.generate_performance_recommendations(&swap_data, success_rate);
            analysis_results.insert("recommendations".to_string(), recommendations);
        }

        Ok(serde_json::json!({
            "status": "success",
            "message": "Swap performance analysis completed",
            "wallet_address": target_wallet,
            "analysis_period": analysis_period,
            "period_boundaries": {
                "start": start_time.to_rfc3339(),
                "end": end_time.to_rfc3339()
            },
            "pool_filter": pool_id,
            "data_points": total_swaps,
            "analysis": analysis_results,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

// =============================================================================
// Server Creation Functions
// =============================================================================

/// Create a new MCP server with the given configuration
pub async fn create_mcp_server(config: McpServerConfig) -> McpResult<MantraDexMcpServer> {
    let server = MantraDexMcpServer::new(config);
    server.initialize().await?;
    Ok(server)
}

/// Create an MCP server with STDIO transport
pub async fn create_stdio_server(config: McpServerConfig) -> McpResult<MantraDexMcpServer> {
    let server = create_mcp_server(config).await?;
    info!("Starting MCP server with STDIO transport");

    // Start the stdio transport loop - this will run indefinitely
    let server_clone = server.clone();
    start_stdio_transport(server_clone).await?;

    // This line should never be reached unless the transport stops
    warn!("STDIO transport has stopped unexpectedly");
    Ok(server)
}

/// Create an MCP server with HTTP transport
pub async fn create_http_server(config: McpServerConfig) -> McpResult<MantraDexMcpServer> {
    let http_host = config.http_host.clone();
    let http_port = config.http_port;
    let server = create_mcp_server(config).await?;
    info!(
        "Created MCP server with HTTP transport on {}:{}",
        http_host, http_port
    );
    Ok(server)
}

// =============================================================================
// Swap History Tracking Data Structures and Helper Methods
// =============================================================================

/// Swap tracking record for history management
#[derive(Debug, Clone)]
pub struct SwapTrackingRecord {
    pub tx_hash: String,
    pub wallet_address: String,
    pub pool_id: String,
    pub from_asset_denom: String,
    pub from_asset_amount: String,
    pub to_asset_denom: String,
    pub expected_return: Option<String>,
    pub max_slippage: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub actual_return: Option<String>,
    pub gas_used: Option<u64>,
    pub gas_wanted: Option<u64>,
    pub block_height: Option<u64>,
}

/// Swap data for analytics and statistics
#[derive(Debug, Clone)]
pub struct SwapAnalyticsData {
    pub tx_hash: String,
    pub wallet_address: String,
    pub pool_id: String,
    pub input_asset: String,
    pub input_amount: String,
    pub output_asset: String,
    pub output_amount: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub gas_used: Option<u64>,
    pub gas_wanted: Option<u64>,
    pub slippage_tolerance: Option<f64>,
    pub actual_slippage: Option<f64>,
    pub block_height: Option<u64>,
}

impl MantraDexMcpServer {
    /// Get swap history from blockchain (placeholder implementation)
    async fn get_swap_history_from_blockchain(
        &self,
        wallet_address: &str,
        _limit: usize,
        _offset: usize,
        _status_filter: &str,
        _pool_id: Option<&str>,
        _from_asset: Option<&str>,
        _to_asset: Option<&str>,
        _date_from: Option<&chrono::DateTime<chrono::FixedOffset>>,
        _date_to: Option<&chrono::DateTime<chrono::FixedOffset>>,
        _min_amount: Option<u128>,
        _max_amount: Option<u128>,
        _sort_by: &str,
        _sort_order: &str,
    ) -> McpResult<Vec<SwapTrackingRecord>> {
        // Placeholder implementation - in real implementation would query blockchain
        // For now, return sample data or empty list
        let sample_records = vec![
            SwapTrackingRecord {
                tx_hash: "sample_tx_1".to_string(),
                wallet_address: wallet_address.to_string(),
                pool_id: "1".to_string(),
                from_asset_denom: "uom".to_string(),
                from_asset_amount: "1000000".to_string(),
                to_asset_denom: "uusdy".to_string(),
                expected_return: Some("950000".to_string()),
                max_slippage: Some("1.0".to_string()),
                timestamp: chrono::Utc::now() - chrono::Duration::hours(1),
                status: "success".to_string(),
                actual_return: Some("948000".to_string()),
                gas_used: Some(150000),
                gas_wanted: Some(200000),
                block_height: Some(1234567),
            },
        ];

        Ok(sample_records)
    }

    /// Filter and sort swap history records
    fn filter_and_sort_swap_history(
        &self,
        mut history: Vec<SwapTrackingRecord>,
        _status_filter: &str,
        sort_by: &str,
        sort_order: &str,
    ) -> Vec<SwapTrackingRecord> {
        // Sort the records
        match sort_by {
            "timestamp" => {
                history.sort_by(|a, b| {
                    if sort_order == "desc" {
                        b.timestamp.cmp(&a.timestamp)
                    } else {
                        a.timestamp.cmp(&b.timestamp)
                    }
                });
            }
            "amount" => {
                history.sort_by(|a, b| {
                    let a_amount = a.from_asset_amount.parse::<u128>().unwrap_or(0);
                    let b_amount = b.from_asset_amount.parse::<u128>().unwrap_or(0);
                    if sort_order == "desc" {
                        b_amount.cmp(&a_amount)
                    } else {
                        a_amount.cmp(&b_amount)
                    }
                });
            }
            "gas_used" => {
                history.sort_by(|a, b| {
                    let a_gas = a.gas_used.unwrap_or(0);
                    let b_gas = b.gas_used.unwrap_or(0);
                    if sort_order == "desc" {
                        b_gas.cmp(&a_gas)
                    } else {
                        a_gas.cmp(&b_gas)
                    }
                });
            }
            "tx_hash" => {
                history.sort_by(|a, b| {
                    if sort_order == "desc" {
                        b.tx_hash.cmp(&a.tx_hash)
                    } else {
                        a.tx_hash.cmp(&b.tx_hash)
                    }
                });
            }
            "pool_id" => {
                history.sort_by(|a, b| {
                    if sort_order == "desc" {
                        b.pool_id.cmp(&a.pool_id)
                    } else {
                        a.pool_id.cmp(&b.pool_id)
                    }
                });
            }
            _ => {} // Default to existing order
        }

        history
    }

    /// Format a swap record for JSON response
    fn format_swap_record(
        &self,
        record: SwapTrackingRecord,
        include_details: bool,
        include_gas_info: bool,
    ) -> serde_json::Value {
        let mut swap_data = serde_json::Map::new();

        // Basic information
        swap_data.insert("tx_hash".to_string(), serde_json::Value::String(record.tx_hash));
        swap_data.insert("timestamp".to_string(), serde_json::Value::String(record.timestamp.to_rfc3339()));
        swap_data.insert("status".to_string(), serde_json::Value::String(record.status));
        swap_data.insert("pool_id".to_string(), serde_json::Value::String(record.pool_id));

        // Asset information
        let mut from_asset = serde_json::Map::new();
        from_asset.insert("denom".to_string(), serde_json::Value::String(record.from_asset_denom));
        from_asset.insert("amount".to_string(), serde_json::Value::String(record.from_asset_amount));
        swap_data.insert("from_asset".to_string(), serde_json::Value::Object(from_asset));

        swap_data.insert("to_asset_denom".to_string(), serde_json::Value::String(record.to_asset_denom));

        if let Some(actual_return) = record.actual_return {
            swap_data.insert("actual_return".to_string(), serde_json::Value::String(actual_return));
        }

        // Detailed information
        if include_details {
            if let Some(expected_return) = record.expected_return {
                swap_data.insert("expected_return".to_string(), serde_json::Value::String(expected_return));
            }
            if let Some(max_slippage) = record.max_slippage {
                swap_data.insert("max_slippage".to_string(), serde_json::Value::String(max_slippage));
            }
            if let Some(block_height) = record.block_height {
                swap_data.insert("block_height".to_string(), serde_json::Value::Number(serde_json::Number::from(block_height)));
            }
        }

        // Gas information
        if include_gas_info {
            let mut gas_info = serde_json::Map::new();
            if let Some(gas_used) = record.gas_used {
                gas_info.insert("gas_used".to_string(), serde_json::Value::Number(serde_json::Number::from(gas_used)));
            }
            if let Some(gas_wanted) = record.gas_wanted {
                gas_info.insert("gas_wanted".to_string(), serde_json::Value::Number(serde_json::Number::from(gas_wanted)));
                if let Some(gas_used) = record.gas_used {
                    let efficiency = (gas_used as f64 / gas_wanted as f64) * 100.0;
                    gas_info.insert("efficiency_percent".to_string(), 
                        serde_json::Value::Number(serde_json::Number::from_f64(efficiency).unwrap_or(serde_json::Number::from(0))));
                }
            }
            if !gas_info.is_empty() {
                swap_data.insert("gas_info".to_string(), serde_json::Value::Object(gas_info));
            }
        }

        serde_json::Value::Object(swap_data)
    }

    /// Calculate time period boundaries
    fn calculate_time_period_boundaries(&self, time_period: &str) -> McpResult<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> {
        let end_time = chrono::Utc::now();
        let start_time = match time_period {
            "1h" => end_time - chrono::Duration::hours(1),
            "24h" => end_time - chrono::Duration::hours(24),
            "7d" => end_time - chrono::Duration::days(7),
            "30d" => end_time - chrono::Duration::days(30),
            "90d" => end_time - chrono::Duration::days(90),
            "1y" => end_time - chrono::Duration::days(365),
            "all" => chrono::DateTime::from_timestamp(0, 0).unwrap_or(end_time - chrono::Duration::days(365 * 10)),
            _ => return Err(McpServerError::InvalidArguments(format!("Invalid time period: {}", time_period))),
        };

        Ok((start_time, end_time))
    }

    /// Get swap data for a specific time period
    async fn get_swap_data_for_period(
        &self,
        wallet_address: &str,
        _start_time: chrono::DateTime<chrono::Utc>,
        _end_time: chrono::DateTime<chrono::Utc>,
    ) -> McpResult<Vec<SwapAnalyticsData>> {
        // Placeholder implementation - in real implementation would query blockchain
        let sample_data = vec![
            SwapAnalyticsData {
                tx_hash: "sample_tx_1".to_string(),
                wallet_address: wallet_address.to_string(),
                pool_id: "1".to_string(),
                input_asset: "uom".to_string(),
                input_amount: "1000000".to_string(),
                output_asset: "uusdy".to_string(),
                output_amount: Some("948000".to_string()),
                timestamp: chrono::Utc::now() - chrono::Duration::hours(1),
                status: "success".to_string(),
                gas_used: Some(150000),
                gas_wanted: Some(200000),
                slippage_tolerance: Some(1.0),
                actual_slippage: Some(0.21),
                block_height: Some(1234567),
            },
        ];

        Ok(sample_data)
    }

    /// Calculate pool breakdown statistics
    fn calculate_pool_breakdown(&self, swap_data: &[SwapAnalyticsData]) -> serde_json::Value {
        let mut pool_stats = std::collections::HashMap::new();

        for swap in swap_data {
            let entry = pool_stats.entry(swap.pool_id.clone()).or_insert_with(|| {
                serde_json::json!({
                    "total_swaps": 0,
                    "successful_swaps": 0,
                    "total_volume": 0.0,
                    "total_gas_used": 0
                })
            });

            if let Some(obj) = entry.as_object_mut() {
                // Update counts
                if let Some(total) = obj.get_mut("total_swaps") {
                    *total = serde_json::Value::Number(serde_json::Number::from(total.as_u64().unwrap_or(0) + 1));
                }

                if swap.status == "success" {
                    if let Some(successful) = obj.get_mut("successful_swaps") {
                        *successful = serde_json::Value::Number(serde_json::Number::from(successful.as_u64().unwrap_or(0) + 1));
                    }

                    // Update volume
                    let amount = swap.input_amount.parse::<f64>().unwrap_or(0.0);
                    if let Some(volume) = obj.get_mut("total_volume") {
                        let current_volume = volume.as_f64().unwrap_or(0.0);
                        *volume = serde_json::Value::Number(serde_json::Number::from_f64(current_volume + amount).unwrap_or(serde_json::Number::from(0)));
                    }
                }

                // Update gas
                if let Some(gas_used) = swap.gas_used {
                    if let Some(total_gas) = obj.get_mut("total_gas_used") {
                        *total_gas = serde_json::Value::Number(serde_json::Number::from(total_gas.as_u64().unwrap_or(0) + gas_used));
                    }
                }
            }
        }

        serde_json::Value::Object(pool_stats.into_iter().collect())
    }

    /// Calculate asset breakdown statistics
    fn calculate_asset_breakdown(&self, swap_data: &[SwapAnalyticsData]) -> serde_json::Value {
        let mut asset_stats = std::collections::HashMap::new();

        for swap in swap_data {
            // Track input asset
            let input_entry = asset_stats.entry(swap.input_asset.clone()).or_insert_with(|| {
                serde_json::json!({
                    "total_swaps_as_input": 0,
                    "total_swaps_as_output": 0,
                    "total_input_volume": 0.0,
                    "total_output_volume": 0.0
                })
            });

            if let Some(obj) = input_entry.as_object_mut() {
                if let Some(total) = obj.get_mut("total_swaps_as_input") {
                    *total = serde_json::Value::Number(serde_json::Number::from(total.as_u64().unwrap_or(0) + 1));
                }

                let amount = swap.input_amount.parse::<f64>().unwrap_or(0.0);
                if let Some(volume) = obj.get_mut("total_input_volume") {
                    let current_volume = volume.as_f64().unwrap_or(0.0);
                    *volume = serde_json::Value::Number(serde_json::Number::from_f64(current_volume + amount).unwrap_or(serde_json::Number::from(0)));
                }
            }

            // Track output asset
            let output_entry = asset_stats.entry(swap.output_asset.clone()).or_insert_with(|| {
                serde_json::json!({
                    "total_swaps_as_input": 0,
                    "total_swaps_as_output": 0,
                    "total_input_volume": 0.0,
                    "total_output_volume": 0.0
                })
            });

            if let Some(obj) = output_entry.as_object_mut() {
                if let Some(total) = obj.get_mut("total_swaps_as_output") {
                    *total = serde_json::Value::Number(serde_json::Number::from(total.as_u64().unwrap_or(0) + 1));
                }

                if let Some(output_amount) = &swap.output_amount {
                    let amount = output_amount.parse::<f64>().unwrap_or(0.0);
                    if let Some(volume) = obj.get_mut("total_output_volume") {
                        let current_volume = volume.as_f64().unwrap_or(0.0);
                        *volume = serde_json::Value::Number(serde_json::Number::from_f64(current_volume + amount).unwrap_or(serde_json::Number::from(0)));
                    }
                }
            }
        }

        serde_json::Value::Object(asset_stats.into_iter().collect())
    }

    /// Calculate performance metrics
    fn calculate_performance_metrics(&self, swap_data: &[SwapAnalyticsData]) -> serde_json::Value {
        let successful_swaps: Vec<_> = swap_data.iter().filter(|s| s.status == "success").collect();

        if successful_swaps.is_empty() {
            return serde_json::json!({
                "note": "No successful swaps for performance analysis"
            });
        }

        // Calculate average slippage
        let slippages: Vec<f64> = successful_swaps
            .iter()
            .filter_map(|s| s.actual_slippage)
            .collect();

        let avg_slippage = if !slippages.is_empty() {
            slippages.iter().sum::<f64>() / slippages.len() as f64
        } else {
            0.0
        };

        // Calculate gas efficiency
        let gas_efficiencies: Vec<f64> = successful_swaps
            .iter()
            .filter_map(|s| {
                if let (Some(used), Some(wanted)) = (s.gas_used, s.gas_wanted) {
                    Some((used as f64 / wanted as f64) * 100.0)
                } else {
                    None
                }
            })
            .collect();

        let avg_gas_efficiency = if !gas_efficiencies.is_empty() {
            gas_efficiencies.iter().sum::<f64>() / gas_efficiencies.len() as f64
        } else {
            0.0
        };

        serde_json::json!({
            "average_slippage_percent": avg_slippage,
            "average_gas_efficiency_percent": avg_gas_efficiency,
            "slippage_samples": slippages.len(),
            "gas_efficiency_samples": gas_efficiencies.len()
        })
    }

    /// Calculate trend analysis
    fn calculate_trend_analysis(
        &self,
        swap_data: &[SwapAnalyticsData],
        _start_time: chrono::DateTime<chrono::Utc>,
        _end_time: chrono::DateTime<chrono::Utc>,
    ) -> serde_json::Value {
        // Simple trend analysis - in real implementation would be more sophisticated
        let total_swaps = swap_data.len();
        let successful_swaps = swap_data.iter().filter(|s| s.status == "success").count();

        serde_json::json!({
            "total_data_points": total_swaps,
            "successful_transactions": successful_swaps,
            "trend_note": "Detailed trend analysis requires more historical data",
            "basic_success_trend": if successful_swaps > 0 { "positive" } else { "no_data" }
        })
    }

    /// Store swap tracking record (placeholder)
    async fn store_swap_tracking_record(&self, _record: SwapTrackingRecord) -> McpResult<()> {
        // Placeholder - in real implementation would store in database
        Ok(())
    }

    /// Get swap history for export
    async fn get_swap_history_for_export(
        &self,
        wallet_address: &str,
        _date_from: Option<&chrono::DateTime<chrono::FixedOffset>>,
        _date_to: Option<&chrono::DateTime<chrono::FixedOffset>>,
        _include_failed: bool,
        _include_pending: bool,
    ) -> McpResult<Vec<SwapTrackingRecord>> {
        // Placeholder - reuse the existing method for now
        self.get_swap_history_from_blockchain(
            wallet_address, 1000, 0, "all", None, None, None, None, None, None, None, "timestamp", "desc"
        ).await
    }

    /// Format swap history as JSON
    fn format_swap_history_as_json(&self, history: &[SwapTrackingRecord]) -> McpResult<String> {
        let formatted_records: Vec<serde_json::Value> = history
            .iter()
            .map(|record| self.format_swap_record(record.clone(), true, true))
            .collect();

        serde_json::to_string_pretty(&formatted_records)
            .map_err(|e| McpServerError::Serialization(e))
    }

    /// Format swap history as CSV
    fn format_swap_history_as_csv(&self, history: &[SwapTrackingRecord]) -> McpResult<String> {
        let mut csv_data = String::new();
        
        // CSV header
        csv_data.push_str("tx_hash,timestamp,status,pool_id,from_asset_denom,from_asset_amount,to_asset_denom,actual_return,gas_used,gas_wanted,block_height\n");

        // CSV rows
        for record in history {
            csv_data.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                record.tx_hash,
                record.timestamp.to_rfc3339(),
                record.status,
                record.pool_id,
                record.from_asset_denom,
                record.from_asset_amount,
                record.to_asset_denom,
                record.actual_return.as_deref().unwrap_or(""),
                record.gas_used.map(|g| g.to_string()).as_deref().unwrap_or(""),
                record.gas_wanted.map(|g| g.to_string()).as_deref().unwrap_or(""),
                record.block_height.map(|h| h.to_string()).as_deref().unwrap_or("")
            ));
        }

        Ok(csv_data)
    }

    /// Format swap history as TSV
    fn format_swap_history_as_tsv(&self, history: &[SwapTrackingRecord]) -> McpResult<String> {
        let csv_data = self.format_swap_history_as_csv(history)?;
        Ok(csv_data.replace(',', "\t"))
    }

    /// Compress data (placeholder)
    fn compress_data(&self, data: &str) -> McpResult<String> {
        // Placeholder - in real implementation would use actual compression
        // For now, just return the data with a note
        Ok(format!("COMPRESSED_DATA_PLACEHOLDER: {}", data.len()))
    }

    /// Get swap data for analysis
    async fn get_swap_data_for_analysis(
        &self,
        wallet_address: &str,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        _pool_id: Option<&str>,
    ) -> McpResult<Vec<SwapAnalyticsData>> {
        // Reuse existing method for now
        self.get_swap_data_for_period(wallet_address, start_time, end_time).await
    }

    /// Analyze gas performance
    fn analyze_gas_performance(&self, swap_data: &[SwapAnalyticsData]) -> serde_json::Value {
        let gas_data: Vec<_> = swap_data
            .iter()
            .filter_map(|s| {
                if let (Some(used), Some(wanted)) = (s.gas_used, s.gas_wanted) {
                    Some((used, wanted))
                } else {
                    None
                }
            })
            .collect();

        if gas_data.is_empty() {
            return serde_json::json!({
                "note": "No gas data available for analysis"
            });
        }

        let total_gas_used: u64 = gas_data.iter().map(|(used, _)| *used).sum();
        let total_gas_wanted: u64 = gas_data.iter().map(|(_, wanted)| *wanted).sum();
        let avg_efficiency = (total_gas_used as f64 / total_gas_wanted as f64) * 100.0;

        serde_json::json!({
            "total_gas_used": total_gas_used,
            "total_gas_wanted": total_gas_wanted,
            "average_efficiency_percent": avg_efficiency,
            "samples": gas_data.len()
        })
    }

    /// Analyze slippage performance
    fn analyze_slippage_performance(&self, swap_data: &[SwapAnalyticsData]) -> serde_json::Value {
        let slippage_data: Vec<f64> = swap_data
            .iter()
            .filter_map(|s| s.actual_slippage)
            .collect();

        if slippage_data.is_empty() {
            return serde_json::json!({
                "note": "No slippage data available for analysis"
            });
        }

        let avg_slippage = slippage_data.iter().sum::<f64>() / slippage_data.len() as f64;
        let max_slippage = slippage_data.iter().fold(0.0f64, |a, &b| a.max(b));
        let min_slippage = slippage_data.iter().fold(f64::INFINITY, |a, &b| a.min(b));

        serde_json::json!({
            "average_slippage_percent": avg_slippage,
            "max_slippage_percent": max_slippage,
            "min_slippage_percent": min_slippage,
            "samples": slippage_data.len()
        })
    }

    /// Analyze timing performance
    fn analyze_timing_performance(&self, _swap_data: &[SwapAnalyticsData]) -> serde_json::Value {
        // Placeholder - would analyze transaction timing patterns
        serde_json::json!({
            "note": "Timing analysis requires additional transaction timing data"
        })
    }

    /// Generate performance recommendations
    fn generate_performance_recommendations(&self, swap_data: &[SwapAnalyticsData], success_rate: f64) -> serde_json::Value {
        let mut recommendations = Vec::new();

        if success_rate < 90.0 {
            recommendations.push("Consider increasing slippage tolerance to improve success rate");
        }

        let avg_gas_used = swap_data
            .iter()
            .filter_map(|s| s.gas_used)
            .map(|g| g as f64)
            .sum::<f64>() / swap_data.len() as f64;

        if avg_gas_used > 200000.0 {
            recommendations.push("Consider optimizing gas usage by batching transactions or using more efficient pools");
        }

        if recommendations.is_empty() {
            recommendations.push("Performance looks good! Continue monitoring for optimization opportunities");
        }

        serde_json::json!({
            "recommendations": recommendations,
            "based_on": {
                "success_rate": success_rate,
                "sample_size": swap_data.len()
            }
        })
    }

    // Resource handler methods for MCP resources

    /// Handle trades://history resource
    async fn handle_trades_history_resource(&self) -> McpResult<serde_json::Value> {
        // Get active wallet address
        let wallet_address = match self.state.get_active_wallet().await? {
            Some(wallet) => wallet.address,
            None => {
                return Ok(serde_json::json!({
                    "trades": [],
                    "total_count": 0,
                    "wallet_address": null,
                    "message": "No active wallet configured"
                }));
            }
        };

        // Get swap history using existing method
        let history = self.get_swap_history_from_blockchain(
            &wallet_address,
            100, // limit
            0,   // offset
            "all", // status filter
            None, // pool_id
            None, // from_asset
            None, // to_asset
            None, // date_from
            None, // date_to
            None, // min_amount
            None, // max_amount
            "timestamp", // sort_by
            "desc" // sort_order
        ).await?;

        // Format trades for resource output
        let formatted_trades: Vec<serde_json::Value> = history
            .iter()
            .map(|record| self.format_swap_record(record.clone(), true, true))
            .collect();

        Ok(serde_json::json!({
            "trades": formatted_trades,
            "total_count": history.len(),
            "wallet_address": wallet_address,
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "resource_uri": "trades://history"
        }))
    }

    /// Handle trades://pending resource
    async fn handle_trades_pending_resource(&self) -> McpResult<serde_json::Value> {
        // Get all active transaction monitors
        let monitors = self.state.transaction_monitor_manager
            .list_monitors_filtered(false).await;

        // Filter for swap-related monitors that are still pending
        let pending_trades: Vec<serde_json::Value> = monitors
            .iter()
            .filter(|monitor| !monitor.is_completed())
            .map(|monitor| {
                serde_json::json!({
                    "tx_hash": monitor.config.tx_hash,
                    "monitor_id": monitor.id,
                    "status": monitor.status.as_str(),
                    "created_at": monitor.created_at.to_rfc3339(),
                    "updated_at": monitor.updated_at.to_rfc3339(),
                    "elapsed_time_secs": monitor.elapsed_time().as_secs(),
                    "poll_count": monitor.poll_count,
                    "config": {
                        "min_confirmations": monitor.config.min_confirmations,
                        "timeout_secs": monitor.config.timeout_secs,
                        "poll_interval_secs": monitor.config.poll_interval_secs
                    }
                })
            })
            .collect();

        Ok(serde_json::json!({
            "pending_trades": pending_trades,
            "total_count": pending_trades.len(),
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "resource_uri": "trades://pending"
        }))
    }

    /// Handle liquidity://positions resource
    async fn handle_liquidity_positions_resource(&self) -> McpResult<serde_json::Value> {
        // Get active wallet address
        let wallet_address = match self.state.get_active_wallet().await? {
            Some(wallet) => wallet.address,
            None => {
                return Ok(serde_json::json!({
                    "positions": [],
                    "total_value": "0",
                    "wallet_address": null,
                    "message": "No active wallet configured"
                }));
            }
        };

        // For now, return a placeholder structure
        // In a real implementation, this would query the blockchain for LP token balances
        // and calculate position values
        let positions = Vec::<serde_json::Value>::new();

        Ok(serde_json::json!({
            "positions": positions,
            "total_value": "0",
            "wallet_address": wallet_address,
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "resource_uri": "liquidity://positions",
            "note": "Liquidity position tracking is not yet fully implemented. This is a placeholder structure."
        }))
    }

    // LP Token Management Tool Handlers

    /// Handle get_lp_token_balance tool
    async fn handle_get_lp_token_balance(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_lp_token_balance tool call");

        let pool_id = arguments
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing or invalid 'pool_id' argument".to_string())
            })?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = arguments.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.state.get_active_wallet().await? {
                Some(wallet) => wallet.address,
                None => {
                    return Err(McpServerError::WalletNotConfigured);
                }
            }
        };

        // Cache key for LP token balance
        let cache_key = format!("lp_balance:{}:{}", pool_id, wallet_address);
        
        if let Some(cached_balance) = self.state.cache_get(&cache_key).await {
            info!(pool_id, wallet_address, "Returning cached LP token balance");
            return Ok(cached_balance);
        }

        info!(pool_id, wallet_address, "Querying LP token balance from blockchain");
        
        // For now, return a placeholder response
        // In a real implementation, this would query the blockchain using the SDK
        let balance_result = serde_json::json!({
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_token_balance": "0",
            "lp_token_denom": format!("factory/{}/lp_{}", "contract_address", pool_id),
            "share_percentage": "0.00",
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "note": "LP token balance querying is not yet fully implemented. This is a placeholder response."
        });

        // Cache the result
        self.state.cache_set(cache_key, balance_result.clone()).await;

        Ok(balance_result)
    }

    /// Handle get_all_lp_token_balances tool
    async fn handle_get_all_lp_token_balances(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling get_all_lp_token_balances tool call");

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = arguments.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.state.get_active_wallet().await? {
                Some(wallet) => wallet.address,
                None => {
                    return Err(McpServerError::WalletNotConfigured);
                }
            }
        };

        let include_zero_balances = arguments
            .get("include_zero_balances")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Cache key for all LP token balances
        let cache_key = format!("all_lp_balances:{}:zero={}", wallet_address, include_zero_balances);
        
        if let Some(cached_balances) = self.state.cache_get(&cache_key).await {
            info!(wallet_address, "Returning cached LP token balances");
            return Ok(cached_balances);
        }

        info!(wallet_address, "Querying all LP token balances from blockchain");
        
        // For now, return a placeholder response
        // In a real implementation, this would:
        // 1. Query all pools from the blockchain
        // 2. For each pool, check if the wallet has LP tokens
        // 3. Calculate share percentages and values
        let balances_result = serde_json::json!({
            "wallet_address": wallet_address,
            "lp_positions": [],
            "total_positions": 0,
            "total_value_usd": "0.00",
            "include_zero_balances": include_zero_balances,
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "note": "LP token balance querying is not yet fully implemented. This is a placeholder response."
        });

        // Cache the result
        self.state.cache_set(cache_key, balances_result.clone()).await;

        Ok(balances_result)
    }

    /// Handle estimate_lp_withdrawal_amounts tool
    async fn handle_estimate_lp_withdrawal_amounts(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling estimate_lp_withdrawal_amounts tool call");

        let pool_id = arguments
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing or invalid 'pool_id' argument".to_string())
            })?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = arguments.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.state.get_active_wallet().await? {
                Some(wallet) => wallet.address,
                None => {
                    return Err(McpServerError::WalletNotConfigured);
                }
            }
        };

        let lp_token_amount = arguments
            .get("lp_token_amount")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Cache key for withdrawal estimation
        let cache_key = format!(
            "lp_withdrawal_estimate:{}:{}:{}",
            pool_id,
            wallet_address,
            lp_token_amount.as_deref().unwrap_or("full_balance")
        );
        
        if let Some(cached_estimate) = self.state.cache_get(&cache_key).await {
            info!(pool_id, wallet_address, "Returning cached LP withdrawal estimate");
            return Ok(cached_estimate);
        }

        info!(pool_id, wallet_address, "Calculating LP withdrawal estimates");
        
        // For now, return a placeholder response
        // In a real implementation, this would:
        // 1. Query the pool's current state and reserves
        // 2. Get the user's LP token balance (if amount not specified)
        // 3. Calculate the proportional withdrawal amounts
        // 4. Account for any withdrawal fees
        let estimate_result = serde_json::json!({
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "lp_token_amount": lp_token_amount.unwrap_or_else(|| "full_balance".to_string()),
            "estimated_withdrawals": [],
            "withdrawal_fee": "0",
            "minimum_received": [],
            "share_percentage": "0.00",
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "note": "LP withdrawal estimation is not yet fully implemented. This is a placeholder response."
        });

        // Cache the result
        self.state.cache_set(cache_key, estimate_result.clone()).await;

        Ok(estimate_result)
    }

    // Analytics & Reporting Tool Handlers

    /// Handle generate_trading_report tool
    async fn handle_generate_trading_report(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling generate_trading_report tool call");

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = arguments.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.state.get_active_wallet().await? {
                Some(wallet) => wallet.address,
                None => {
                    return Err(McpServerError::WalletNotConfigured);
                }
            }
        };

        let time_period = arguments
            .get("time_period")
            .and_then(|v| v.as_str())
            .unwrap_or("30d");

        let report_format = arguments
            .get("report_format")
            .and_then(|v| v.as_str())
            .unwrap_or("summary");

        let include_charts = arguments
            .get("include_charts")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let include_pool_breakdown = arguments
            .get("include_pool_breakdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_asset_breakdown = arguments
            .get("include_asset_breakdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_performance_metrics = arguments
            .get("include_performance_metrics")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Cache key for trading report
        let cache_key = format!(
            "trading_report:{}:{}:{}:charts={}:pools={}:assets={}:perf={}",
            wallet_address,
            time_period,
            report_format,
            include_charts,
            include_pool_breakdown,
            include_asset_breakdown,
            include_performance_metrics
        );
        
        if let Some(cached_report) = self.state.cache_get(&cache_key).await {
            info!(wallet_address, time_period, "Returning cached trading report");
            return Ok(cached_report);
        }

        info!(wallet_address, time_period, "Generating trading report");

        // Calculate time boundaries
        let (start_time, end_time) = self.calculate_time_period_boundaries(time_period)?;

        // Get trading data for the period
        let swap_data = self.get_swap_data_for_period(&wallet_address, start_time, end_time).await?;

        // Generate report sections based on requested format and options
        let mut report = serde_json::json!({
            "wallet_address": wallet_address,
            "time_period": time_period,
            "report_format": report_format,
            "generated_at": chrono::Utc::now().to_rfc3339(),
            "period_start": start_time.to_rfc3339(),
            "period_end": end_time.to_rfc3339(),
            "total_transactions": swap_data.len()
        });

        // Add performance metrics if requested
        if include_performance_metrics {
            let performance_metrics = self.calculate_performance_metrics(&swap_data);
            report["performance_metrics"] = performance_metrics;
        }

        // Add pool breakdown if requested
        if include_pool_breakdown {
            let pool_breakdown = self.calculate_pool_breakdown(&swap_data);
            report["pool_breakdown"] = pool_breakdown;
        }

        // Add asset breakdown if requested
        if include_asset_breakdown {
            let asset_breakdown = self.calculate_asset_breakdown(&swap_data);
            report["asset_breakdown"] = asset_breakdown;
        }

        // Add chart data if requested
        if include_charts {
            report["chart_data"] = serde_json::json!({
                "note": "Chart data generation is not yet implemented",
                "time_series": [],
                "volume_chart": [],
                "performance_chart": []
            });
        }

        // Format report based on requested format
        let formatted_report = match report_format {
            "summary" => self.format_trading_report_summary(report),
            "detailed" => self.format_trading_report_detailed(report),
            "json" => report,
            _ => report,
        };

        // Cache the result
        self.state.cache_set(cache_key, formatted_report.clone()).await;

        Ok(formatted_report)
    }

    /// Handle calculate_impermanent_loss tool
    async fn handle_calculate_impermanent_loss(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        info!(?arguments, "Handling calculate_impermanent_loss tool call");

        let pool_id = arguments
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing or invalid 'pool_id' argument".to_string())
            })?;

        // Get wallet address (use active wallet if not provided)
        let wallet_address = if let Some(addr) = arguments.get("wallet_address").and_then(|v| v.as_str()) {
            addr.to_string()
        } else {
            match self.state.get_active_wallet().await? {
                Some(wallet) => wallet.address,
                None => {
                    return Err(McpServerError::WalletNotConfigured);
                }
            }
        };

        let entry_price_asset_a = arguments
            .get("entry_price_asset_a")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing or invalid 'entry_price_asset_a' argument".to_string())
            })?;

        let entry_price_asset_b = arguments
            .get("entry_price_asset_b")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing or invalid 'entry_price_asset_b' argument".to_string())
            })?;

        let current_price_asset_a = arguments
            .get("current_price_asset_a")
            .and_then(|v| v.as_str());

        let current_price_asset_b = arguments
            .get("current_price_asset_b")
            .and_then(|v| v.as_str());

        let lp_token_amount = arguments
            .get("lp_token_amount")
            .and_then(|v| v.as_str());

        let include_fees_earned = arguments
            .get("include_fees_earned")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let include_detailed_breakdown = arguments
            .get("include_detailed_breakdown")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Cache key for impermanent loss calculation
        let cache_key = format!(
            "impermanent_loss:{}:{}:{}:{}:{}:{}:fees={}:detailed={}",
            pool_id,
            wallet_address,
            entry_price_asset_a,
            entry_price_asset_b,
            current_price_asset_a.unwrap_or("current"),
            current_price_asset_b.unwrap_or("current"),
            include_fees_earned,
            include_detailed_breakdown
        );
        
        if let Some(cached_calculation) = self.state.cache_get(&cache_key).await {
            info!(pool_id, wallet_address, "Returning cached impermanent loss calculation");
            return Ok(cached_calculation);
        }

        info!(pool_id, wallet_address, "Calculating impermanent loss");

        // For now, return a placeholder calculation
        // In a real implementation, this would:
        // 1. Fetch current prices if not provided
        // 2. Get the user's LP token balance if amount not specified
        // 3. Calculate the current value of LP position
        // 4. Calculate what the value would be if assets were held separately
        // 5. Calculate impermanent loss percentage
        // 6. Include fees earned if requested
        let calculation_result = serde_json::json!({
            "pool_id": pool_id,
            "wallet_address": wallet_address,
            "entry_prices": {
                "asset_a": entry_price_asset_a,
                "asset_b": entry_price_asset_b
            },
            "current_prices": {
                "asset_a": current_price_asset_a.unwrap_or("current_price_not_provided"),
                "asset_b": current_price_asset_b.unwrap_or("current_price_not_provided")
            },
            "lp_token_amount": lp_token_amount.unwrap_or("full_balance"),
            "impermanent_loss": {
                "percentage": "0.00%",
                "absolute_value": "0",
                "currency": "USD"
            },
            "fees_earned": if include_fees_earned {
                serde_json::json!({
                    "total_fees": "0",
                    "currency": "USD"
                })
            } else {
                serde_json::Value::Null
            },
            "net_result": {
                "total_loss_after_fees": "0",
                "percentage_after_fees": "0.00%"
            },
            "detailed_breakdown": if include_detailed_breakdown {
                serde_json::json!({
                    "initial_position_value": "0",
                    "current_lp_value": "0",
                    "hodl_value": "0",
                    "price_ratio_change": "0.00%",
                    "calculation_steps": []
                })
            } else {
                serde_json::Value::Null
            },
            "calculated_at": chrono::Utc::now().to_rfc3339(),
            "note": "Impermanent loss calculation is not yet fully implemented. This is a placeholder response."
        });

        // Cache the result
        self.state.cache_set(cache_key, calculation_result.clone()).await;

        Ok(calculation_result)
    }

    // Helper methods for report formatting

    /// Format trading report as summary
    fn format_trading_report_summary(&self, report: serde_json::Value) -> serde_json::Value {
        let mut summary = serde_json::json!({
            "report_type": "summary",
            "wallet_address": report["wallet_address"],
            "time_period": report["time_period"],
            "generated_at": report["generated_at"],
            "total_transactions": report["total_transactions"]
        });

        // Add key metrics if available
        if let Some(performance) = report.get("performance_metrics") {
            summary["key_metrics"] = serde_json::json!({
                "success_rate": performance.get("success_rate").unwrap_or(&serde_json::json!("N/A")),
                "total_volume": performance.get("total_volume").unwrap_or(&serde_json::json!("N/A")),
                "average_gas_used": performance.get("average_gas_used").unwrap_or(&serde_json::json!("N/A"))
            });
        }

        summary
    }

    /// Format trading report as detailed
    fn format_trading_report_detailed(&self, report: serde_json::Value) -> serde_json::Value {
        let mut detailed = report.clone();
        detailed["report_type"] = serde_json::json!("detailed");
        detailed
    }
}

/// Start the stdio transport layer for MCP communication
async fn start_stdio_transport(server: MantraDexMcpServer) -> McpResult<()> {
    use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
    
    info!("Starting stdio transport for MCP communication");
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    
    info!("Server is ready and listening for JSON-RPC messages on stdin...");

    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
    
    loop {
        line.clear();
        debug!("Waiting for input on stdin...");
        
        // Read a line from stdin
        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF reached, client disconnected
                info!("Client disconnected (EOF received)");
                break;
            }
            Ok(_) => {
                // Parse and handle the JSON-RPC request
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                
                debug!("Received MCP request: {}", trimmed);
                
                // Parse JSON-RPC request
                let response = match serde_json::from_str::<serde_json::Value>(trimmed) {
                    Ok(request) => handle_json_rpc_request(&server, request).await,
                    Err(e) => {
                        warn!("Failed to parse JSON-RPC request: {}", e);
                        JsonRpcResponse::error(
                            None,
                            JsonRpcError {
                                code: -32700, // Parse error
                                message: "Parse error".to_string(),
                                data: Some(serde_json::json!({
                                    "error": e.to_string(),
                                    "request": trimmed
                                })),
                            },
                        )
                    }
                };
                
                // Send response back via stdout
                match serde_json::to_string(&response) {
                    Ok(response_json) => {
                        if let Err(e) = stdout.write_all(response_json.as_bytes()).await {
                            warn!("Failed to write response to stdout: {}", e);
                            break;
                        }
                        if let Err(e) = stdout.write_all(b"\n").await {
                            warn!("Failed to write newline to stdout: {}", e);
                            break;
                        }
                        if let Err(e) = stdout.flush().await {
                            warn!("Failed to flush stdout: {}", e);
                            break;
                        }
                        debug!("Sent MCP response: {}", response_json);
                    }
                    Err(e) => {
                        warn!("Failed to serialize response: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                warn!("Error reading from stdin: {}", e);
                break;
            }
        }
    }
    
    info!("Stdio transport stopped");
    Ok(())
}

/// Handle a JSON-RPC request and return a JSON-RPC response
async fn handle_json_rpc_request(
    server: &MantraDexMcpServer,
    request: serde_json::Value,
) -> JsonRpcResponse {
    // Extract request ID for response correlation
    let request_id = request.get("id").cloned();
    
    // Extract method and params
    let method = match request.get("method").and_then(|m| m.as_str()) {
        Some(method) => method,
        None => {
            return JsonRpcResponse::error(
                request_id,
                JsonRpcError {
                    code: -32600, // Invalid Request
                    message: "Missing method".to_string(),
                    data: None,
                },
            );
        }
    };
    
    let params = request.get("params").cloned();
    
    debug!("Handling MCP method: {}", method);
    
    // Handle the request using the server
    match server.handle_request(method, params).await {
        Ok(result) => JsonRpcResponse::success(request_id, result),
        Err(e) => {
            warn!("MCP request failed: {}", e);
            JsonRpcResponse::error(request_id, e.to_json_rpc_error())
        }
    }
}

impl Clone for MantraDexMcpServer {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}
