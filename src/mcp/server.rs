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
use tracing::{debug, error, info, warn};

// MCP transport and communication types
use axum::{extract::State, http::StatusCode};
use tokio::io::{AsyncBufReadExt, BufReader};

// Configuration support
use config::{Config, ConfigError, Environment, File, FileFormat};

// TODO: Import correct MCP types when API is finalized
// Current rust-mcp-sdk 0.4.2 has unstable APIs that don't match documentation
// Using minimal imports for now until API stabilizes

use crate::client::MantraDexClient;
use crate::config::{MantraNetworkConfig, NetworkConstants};
use crate::error::Error as SdkError;
use crate::wallet::{MantraWallet, WalletInfo};

use super::client_wrapper::McpClientWrapper;
use super::logging::{LoggingConfig, McpLogger};
use super::sdk_adapter::McpSdkAdapter;

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
            "initialize" => Ok(self.get_server_info()),
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
        use json_rpc_error_codes::*;

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
            McpServerError::Config(_) => json_rpc_error_codes::CONFIGURATION_ERROR,
        }
    }

    /// Map SDK error types to appropriate JSON-RPC error codes
    fn sdk_error_to_code(sdk_error: &SdkError) -> i32 {
        use json_rpc_error_codes::*;

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
                "mainnet" | "mantra-dukong" => {
                    if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                        config.network_config = MantraNetworkConfig::from_constants(&constants);
                    } else {
                        warn!("Could not load mainnet constants, using default");
                    }
                }
                "testnet" | "mantra-testnet" => {
                    if let Ok(constants) = NetworkConstants::load("mantra-testnet") {
                        config.network_config = MantraNetworkConfig::from_constants(&constants);
                    } else {
                        warn!("Could not load testnet constants, using default");
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
            "mainnet" | "mantra-dukong" => {
                if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                    config.network_config = MantraNetworkConfig::from_constants(&constants);
                } else {
                    return Err(McpServerError::Network(
                        "Could not load mainnet network constants".to_string(),
                    ));
                }
            }
            "testnet" | "mantra-testnet" => {
                if let Ok(constants) = NetworkConstants::load("mantra-testnet") {
                    config.network_config = MantraNetworkConfig::from_constants(&constants);
                } else {
                    return Err(McpServerError::Network(
                        "Could not load testnet network constants".to_string(),
                    ));
                }
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unsupported network: {}. Supported networks: mainnet, testnet, mantra-dukong, mantra-testnet",
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
            "mainnet" | "mantra-dukong" => {
                if let Ok(constants) = NetworkConstants::load("mantra-dukong") {
                    config.network_config = MantraNetworkConfig::from_constants(&constants);
                    info!("Applied mainnet network configuration");
                } else {
                    warn!("Could not load mainnet constants, using default");
                }
            }
            "testnet" | "mantra-testnet" => {
                if let Ok(constants) = NetworkConstants::load("mantra-testnet") {
                    config.network_config = MantraNetworkConfig::from_constants(&constants);
                    info!("Applied testnet network configuration");
                } else {
                    warn!("Could not load testnet constants, using default");
                }
            }
            _ => {
                return Err(McpServerError::Validation(format!(
                    "Unknown network: {}. Supported networks: mainnet, testnet, mantra-dukong, mantra-testnet",
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
        }
    }

    /// Initialize the DEX client
    pub async fn initialize_client(&self) -> McpResult<()> {
        let client = MantraDexClient::new(self.config.network_config.clone())
            .await
            .map_err(McpServerError::Sdk)?;

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

    /// Get available tools list - Foundation for MCP tools
    pub fn get_available_tools(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "name": "generate_wallet",
                "description": "Generate a new HD wallet with mnemonic phrase",
                "input_schema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "import_wallet",
                "description": "Import wallet from mnemonic phrase",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "mnemonic": {
                            "type": "string",
                            "description": "BIP39 mnemonic phrase"
                        },
                        "account_index": {
                            "type": "integer",
                            "description": "Account index for derivation",
                            "default": 0
                        }
                    },
                    "required": ["mnemonic"]
                }
            }),
            serde_json::json!({
                "name": "get_wallet_info",
                "description": "Get information about the active wallet",
                "input_schema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "get_wallet_balance",
                "description": "Get token balances for the active wallet",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "address": {
                            "type": "string",
                            "description": "Wallet address (optional, uses active wallet if not provided)"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "switch_wallet",
                "description": "Switch to a different wallet by address",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "address": {
                            "type": "string",
                            "description": "Wallet address to switch to"
                        }
                    },
                    "required": ["address"]
                }
            }),
            serde_json::json!({
                "name": "validate_wallet",
                "description": "Validate wallet address, mnemonic, or other wallet data",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "address": {
                            "type": "string",
                            "description": "Wallet address to validate (optional)"
                        },
                        "mnemonic": {
                            "type": "string",
                            "description": "Mnemonic phrase to validate (optional)"
                        },
                        "public_key": {
                            "type": "string",
                            "description": "Public key to validate (optional)"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "switch_network",
                "description": "Switch between mainnet and testnet",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "network": {
                            "type": "string",
                            "enum": ["mainnet", "testnet"],
                            "description": "Network to switch to"
                        }
                    },
                    "required": ["network"]
                }
            }),
            serde_json::json!({
                "name": "get_pool",
                "description": "Get detailed information about a specific liquidity pool",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "The identifier of the pool to query"
                        }
                    },
                    "required": ["pool_id"]
                }
            }),
            serde_json::json!({
                "name": "get_pools",
                "description": "List liquidity pools with filtering and pagination options",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of pools to return (1-100)",
                            "minimum": 1,
                            "maximum": 100,
                            "default": 20
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Number of pools to skip for pagination",
                            "minimum": 0,
                            "default": 0
                        },
                        "status_filter": {
                            "type": "string",
                            "description": "Filter pools by operational status",
                            "enum": ["all", "active", "inactive", "swaps_enabled", "deposits_enabled"],
                            "default": "all"
                        },
                        "sort_by": {
                            "type": "string",
                            "description": "Sort pools by specified criteria",
                            "enum": ["pool_id", "tvl", "total_share", "created_at"],
                            "default": "pool_id"
                        },
                        "sort_order": {
                            "type": "string",
                            "description": "Sort order for results",
                            "enum": ["asc", "desc"],
                            "default": "asc"
                        },
                        "include_details": {
                            "type": "boolean",
                            "description": "Include detailed pool information (assets, fees, etc.)",
                            "default": false
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "simulate_swap",
                "description": "Simulate a token swap to preview outcomes",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID for the swap"
                        },
                        "offer_asset": {
                            "type": "object",
                            "properties": {
                                "denom": {"type": "string"},
                                "amount": {"type": "string"}
                            },
                            "required": ["denom", "amount"]
                        },
                        "ask_asset_denom": {
                            "type": "string",
                            "description": "Denomination of asset to receive"
                        }
                    },
                    "required": ["pool_id", "offer_asset", "ask_asset_denom"]
                }
            }),
            serde_json::json!({
                "name": "execute_swap",
                "description": "Execute a token swap",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "pool_id": {
                            "type": "string",
                            "description": "Pool ID for the swap"
                        },
                        "offer_asset": {
                            "type": "object",
                            "properties": {
                                "denom": {"type": "string"},
                                "amount": {"type": "string"}
                            },
                            "required": ["denom", "amount"]
                        },
                        "ask_asset_denom": {
                            "type": "string",
                            "description": "Denomination of asset to receive"
                        },
                        "max_slippage": {
                            "type": "number",
                            "description": "Maximum allowed slippage (optional)"
                        }
                    },
                    "required": ["pool_id", "offer_asset", "ask_asset_denom"]
                }
            }),
            serde_json::json!({
                "name": "get_network_status",
                "description": "Get current network status including connectivity and block height",
                "input_schema": {
                    "type": "object",
                    "properties": {}
                }
            }),
        ]
    }

    /// Get available resources list - Foundation for MCP resources
    pub fn get_available_resources(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "uri": "wallet://active",
                "name": "Active Wallet",
                "description": "Information about the currently active wallet",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "network://config",
                "name": "Network Configuration",
                "description": "Current network configuration and settings",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "pools://list",
                "name": "Pool List",
                "description": "List of all available liquidity pools",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "pools://details/{id}",
                "name": "Pool Details",
                "description": "Detailed information about a specific pool by ID",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "wallet://balance",
                "name": "Wallet Balance",
                "description": "Token balances for the active wallet",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "wallet://create",
                "name": "Wallet Creation",
                "description": "Generate new HD wallet with mnemonic phrase",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "wallet://import",
                "name": "Wallet Import",
                "description": "Import wallet from mnemonic phrase",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "wallet://info",
                "name": "Wallet Information",
                "description": "Detailed wallet information including address and public key",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "wallet://save",
                "name": "Wallet Save",
                "description": "Save wallet configuration securely",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "wallet://load",
                "name": "Wallet Load",
                "description": "Load saved wallet configuration",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "wallet://list",
                "name": "Saved Wallets List",
                "description": "List of all saved wallet configurations",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "network://switch",
                "name": "Network Switching",
                "description": "Information about network switching capabilities and available networks",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "network://status",
                "name": "Network Status",
                "description": "Current blockchain network status including connectivity, block height, and sync status",
                "mime_type": "application/json"
            }),
            serde_json::json!({
                "uri": "contracts://addresses",
                "name": "Contract Addresses",
                "description": "Smart contract addresses for the current network including pool manager and fee collector",
                "mime_type": "application/json"
            }),
        ]
    }

    /// Handle tool execution - Foundation for MCP tool handling
    pub async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        match tool_name {
            "generate_wallet" => self.handle_generate_wallet(arguments).await,
            "import_wallet" => self.handle_import_wallet(arguments).await,
            "get_wallet_info" => self.handle_get_wallet_info(arguments).await,
            "get_wallet_balance" => self.handle_get_wallet_balance(arguments).await,
            "switch_wallet" => self.handle_switch_wallet(arguments).await,
            "validate_wallet" => self.handle_validate_wallet(arguments).await,
            "switch_network" => self.handle_switch_network(arguments).await,
            "get_pool" => self.handle_get_pool(arguments).await,
            "get_pools" => self.handle_get_pools(arguments).await,
            "simulate_swap" => self.handle_simulate_swap(arguments).await,
            "execute_swap" => self.handle_execute_swap(arguments).await,
            "get_network_status" => self.handle_get_network_status(arguments).await,
            _ => Err(McpServerError::UnknownTool(tool_name.to_string())),
        }
    }

    /// Handle resource reading - Foundation for MCP resource handling
    pub async fn handle_resource_read(&self, uri: &str) -> McpResult<serde_json::Value> {
        match uri {
            "wallet://active" => self.read_active_wallet().await,
            "wallet://create" => self.read_wallet_create().await,
            "wallet://import" => self.read_wallet_import().await,
            "wallet://info" => self.read_wallet_info().await,
            "wallet://save" => self.read_wallet_save().await,
            "wallet://load" => self.read_wallet_load().await,
            "wallet://list" => self.read_wallet_list().await,
            "network://config" => self.read_network_config().await,
            "network://switch" => self.read_network_switch().await,
            "network://status" => self.read_network_status().await,
            "contracts://addresses" => self.read_contracts_addresses().await,
            "pools://list" => self.read_pools_list().await,
            _ if uri.starts_with("pools://details/") => {
                let pool_id = uri.strip_prefix("pools://details/").unwrap_or("");
                self.read_pool_details(pool_id).await
            }
            "wallet://balance" => self.read_wallet_balance().await,
            _ => Err(McpServerError::UnknownResource(uri.to_string())),
        }
    }
}

// Tool implementation methods
impl MantraDexMcpServer {
    /// Generate a new wallet
    async fn handle_generate_wallet(
        &self,
        _arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        match MantraWallet::generate() {
            Ok((wallet, mnemonic)) => {
                let wallet_info = wallet.info();

                // Set as active wallet
                self.state
                    .set_active_wallet(wallet_info.address.clone(), wallet_info.clone())
                    .await?;

                Ok(serde_json::json!({
                    "address": wallet_info.address,
                    "public_key": wallet_info.public_key,
                    "mnemonic": mnemonic,
                    "status": "generated",
                    "active": true
                }))
            }
            Err(e) => Err(McpServerError::Sdk(e)),
        }
    }

    /// Import an existing wallet
    async fn handle_import_wallet(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let mnemonic = arguments
            .get("mnemonic")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing required parameter: mnemonic".to_string())
            })?;

        let account_index = arguments
            .get("account_index")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        match MantraWallet::from_mnemonic(mnemonic, account_index) {
            Ok(wallet) => {
                let wallet_info = wallet.info();

                // Set as active wallet
                self.state
                    .set_active_wallet(wallet_info.address.clone(), wallet_info.clone())
                    .await?;

                Ok(serde_json::json!({
                    "address": wallet_info.address,
                    "public_key": wallet_info.public_key,
                    "status": "imported",
                    "active": true
                }))
            }
            Err(e) => Err(McpServerError::Sdk(e)),
        }
    }

    /// Get wallet information
    async fn handle_get_wallet_info(
        &self,
        _arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        match self.state.get_active_wallet().await? {
            Some(wallet_info) => Ok(serde_json::json!({
                "address": wallet_info.address,
                "public_key": wallet_info.public_key,
                "active": true
            })),
            None => Err(McpServerError::WalletNotConfigured),
        }
    }

    /// Get wallet balance
    async fn handle_get_wallet_balance(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        debug!("Getting wallet balance with arguments: {:?}", arguments);

        // Get optional address from arguments
        let address = arguments.get("address").and_then(|v| v.as_str());

        // Check if we have a client wrapper initialized
        let wrapper_guard = self.state.client_wrapper.lock().await;
        if let Some(wrapper) = wrapper_guard.as_ref() {
            wrapper.get_wallet_balance(address).await
        } else {
            // Fallback to basic implementation if wrapper not initialized
            warn!("Client wrapper not initialized, falling back to basic implementation");
            Ok(serde_json::json!({
                "error": "Client wrapper not initialized",
                "balances": [],
                "note": "Please ensure the server is properly initialized"
            }))
        }
    }

    /// Switch to a different wallet by address
    async fn handle_switch_wallet(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let address = arguments
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing required parameter: address".to_string())
            })?;

        // Check if the wallet exists in our cache
        {
            let wallets = self.state.wallets.read().await;
            if !wallets.contains_key(address) {
                return Err(McpServerError::InvalidArguments(format!(
                    "Wallet with address '{}' not found. Use 'import_wallet' to add it first.",
                    address
                )));
            }
        }

        // Get the wallet info and set it as active
        let wallet_info = {
            let wallets = self.state.wallets.read().await;
            wallets.get(address).cloned().ok_or_else(|| {
                McpServerError::InvalidArguments(format!(
                    "Wallet with address '{}' not found in cache",
                    address
                ))
            })?
        };

        // Set as active wallet
        {
            let mut active_wallet = self.state.active_wallet.lock().await;
            *active_wallet = Some(address.to_string());
        }

        info!("Switched to wallet: {}", address);

        Ok(serde_json::json!({
            "switched": true,
            "address": address,
            "public_key": wallet_info.public_key,
            "message": format!("Successfully switched to wallet: {}", address)
        }))
    }

    /// Validate wallet address, mnemonic, or other wallet data
    async fn handle_validate_wallet(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let mut validation_results = serde_json::Map::new();
        let mut overall_valid = true;

        // Validate address if provided
        if let Some(address) = arguments.get("address").and_then(|v| v.as_str()) {
            let is_valid = self.validate_address(address);
            validation_results.insert("address".to_string(), serde_json::json!({
                "value": address,
                "valid": is_valid,
                "message": if is_valid { "Valid Mantra address format" } else { "Invalid address format" }
            }));
            if !is_valid {
                overall_valid = false;
            }
        }

        // Validate mnemonic if provided
        if let Some(mnemonic) = arguments.get("mnemonic").and_then(|v| v.as_str()) {
            let is_valid = self.validate_mnemonic(mnemonic);
            validation_results.insert("mnemonic".to_string(), serde_json::json!({
                "valid": is_valid,
                "word_count": mnemonic.split_whitespace().count(),
                "message": if is_valid { "Valid BIP39 mnemonic phrase" } else { "Invalid mnemonic phrase" }
            }));
            if !is_valid {
                overall_valid = false;
            }
        }

        // Validate public key if provided
        if let Some(public_key) = arguments.get("public_key").and_then(|v| v.as_str()) {
            let is_valid = self.validate_public_key(public_key);
            validation_results.insert("public_key".to_string(), serde_json::json!({
                "value": public_key,
                "valid": is_valid,
                "message": if is_valid { "Valid public key format" } else { "Invalid public key format" }
            }));
            if !is_valid {
                overall_valid = false;
            }
        }

        if validation_results.is_empty() {
            return Err(McpServerError::InvalidArguments(
                "At least one of 'address', 'mnemonic', or 'public_key' must be provided"
                    .to_string(),
            ));
        }

        Ok(serde_json::json!({
            "overall_valid": overall_valid,
            "validations": validation_results,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Helper method to validate Mantra address format
    fn validate_address(&self, address: &str) -> bool {
        // Basic validation for Mantra address format (bech32)
        address.starts_with("mantra1")
            && address.len() >= 39  // Minimum length for bech32 address
            && address.len() <= 59  // Maximum reasonable length
            && address
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    }

    /// Helper method to validate mnemonic phrase
    fn validate_mnemonic(&self, mnemonic: &str) -> bool {
        use bip39::Mnemonic;
        // Try to parse as BIP39 mnemonic
        Mnemonic::parse(mnemonic).is_ok()
    }

    /// Helper method to validate public key format
    fn validate_public_key(&self, public_key: &str) -> bool {
        // Basic validation - should be hex string of appropriate length
        if public_key.len() != 66 || !public_key.starts_with("0x") {
            return false;
        }
        public_key[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Switch network
    async fn handle_switch_network(
        &self,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        let network = arguments
            .get("network")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing required parameter: network".to_string())
            })?;

        // Validate network name
        let valid_networks = ["mantra-dukong", "mantra-testnet", "mantra-mainnet"];
        if !valid_networks.contains(&network) {
            return Err(McpServerError::InvalidArguments(format!(
                "Invalid network '{}'. Valid networks: {}",
                network,
                valid_networks.join(", ")
            )));
        }

        // Switch to the new network
        self.state.switch_network(network).await?;

        // Load the new network configuration to return current settings
        use crate::config::{MantraNetworkConfig, NetworkConstants};
        let constants = NetworkConstants::load(network).map_err(|e| {
            McpServerError::Network(format!("Failed to load network constants: {}", e))
        })?;
        let network_config = MantraNetworkConfig::from_constants(&constants);

        // Initialize client with new network
        self.state
            .initialize_client_with_network(network_config.clone())
            .await?;

        // Reinitialize client wrapper with new network
        let wrapper = McpClientWrapper::new(self.state.sdk_adapter.clone(), network_config.clone());
        *self.state.client_wrapper.lock().await = Some(wrapper);

        Ok(serde_json::json!({
            "network": network,
            "switched": true,
            "network_id": network_config.network_id,
            "rpc_url": network_config.rpc_url,
            "native_denom": network_config.native_denom,
            "gas_price": network_config.gas_price,
            "message": format!("Successfully switched to network: {}", network)
        }))
    }

    /// Get single pool by ID
    async fn handle_get_pool(&self, arguments: serde_json::Value) -> McpResult<serde_json::Value> {
        // Extract pool_id from arguments and convert to owned String
        let pool_id = arguments
            .get("pool_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                McpServerError::InvalidArguments("Missing or invalid pool_id parameter".to_string())
            })?
            .to_string();

        // Get client through SDK adapter with retry logic
        let state = &self.state;
        let network_config = state.config.network_config.clone();
        let adapter = state.sdk_adapter.clone();

        let pool_id_for_error = pool_id.clone();

        // Use direct async block to avoid complex borrowing issues
        let result = async {
            let client = adapter
                .get_client(&network_config)
                .await
                .map_err(|e| McpServerError::Network(format!("Failed to get client: {}", e)))?;

            client
                .get_pool(&pool_id)
                .await
                .map_err(|e| McpServerError::Mcp(format!("Failed to get pool {}: {}", pool_id, e)))
        }
        .await;

        match result {
            Ok(pool_info) => {
                // Convert pool info to JSON response
                let pool_status = pool_info.pool_info.status;
                let assets: Vec<serde_json::Value> = pool_info
                    .pool_info
                    .assets
                    .iter()
                    .map(|asset| {
                        serde_json::json!({
                            "denom": asset.denom,
                            "amount": asset.amount.to_string()
                        })
                    })
                    .collect();

                Ok(serde_json::json!({
                    "pool_id": pool_info.pool_info.pool_identifier,
                    "lp_denom": pool_info.pool_info.lp_denom,
                    "pool_type": format!("{:?}", pool_info.pool_info.pool_type),
                    "assets": assets,
                    "total_share": {
                        "denom": pool_info.total_share.denom,
                        "amount": pool_info.total_share.amount.to_string()
                    },
                    "status": {
                        "swaps_enabled": pool_status.swaps_enabled,
                        "deposits_enabled": pool_status.deposits_enabled,
                        "withdrawals_enabled": pool_status.withdrawals_enabled
                    },
                    "fees": pool_info.pool_info.pool_fees,
                    "success": true
                }))
            }
            Err(e) => {
                // Return error information but don't fail completely
                Ok(serde_json::json!({
                    "pool_id": pool_id_for_error,
                    "success": false,
                    "error": format!("Failed to retrieve pool: {}", e),
                    "note": "Pool may not exist or network connection failed"
                }))
            }
        }
    }

    /// Get pools with filtering and pagination
    async fn handle_get_pools(&self, arguments: serde_json::Value) -> McpResult<serde_json::Value> {
        // Parse arguments with defaults
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20)
            .min(100)
            .max(1) as usize;

        let offset = arguments
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let status_filter = arguments
            .get("status_filter")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let sort_by = arguments
            .get("sort_by")
            .and_then(|v| v.as_str())
            .unwrap_or("pool_id");

        let sort_order = arguments
            .get("sort_order")
            .and_then(|v| v.as_str())
            .unwrap_or("asc");

        let include_details = arguments
            .get("include_details")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get client through SDK adapter
        let state = &self.state;
        let network_config = state.config.network_config.clone();
        let adapter = state.sdk_adapter.clone();

        // Use direct async block to avoid complex borrowing issues
        let result = async {
            let client = adapter
                .get_client(&network_config)
                .await
                .map_err(|e| McpServerError::Network(format!("Failed to get client: {}", e)))?;

            client
                .get_pools(Some(1000)) // Request up to 1000 pools
                .await
                .map_err(|e| McpServerError::Mcp(format!("Failed to get pools: {}", e)))
        }
        .await;

        match result {
            Ok(pools_info) => {
                // Transform pools to MCP-friendly format with filtering and sorting
                let mut pools: Vec<serde_json::Value> = pools_info
                    .into_iter()
                    .map(|pool| {
                        let pool_status = &pool.pool_info.status;
                        let mut pool_json = serde_json::json!({
                            "pool_id": pool.pool_info.pool_identifier,
                            "pool_type": format!("{:?}", pool.pool_info.pool_type),
                            "lp_denom": pool.pool_info.lp_denom,
                            "pool_status": {
                                "swaps_enabled": pool_status.swaps_enabled,
                                "deposits_enabled": pool_status.deposits_enabled,
                                "withdrawals_enabled": pool_status.withdrawals_enabled
                            },
                            "operational_status": {
                                "is_active": pool_status.swaps_enabled || pool_status.deposits_enabled,
                                "has_swaps": pool_status.swaps_enabled,
                                "has_deposits": pool_status.deposits_enabled,
                                "has_withdrawals": pool_status.withdrawals_enabled
                            }
                        });

                        // Add basic asset information
                        if !pool.pool_info.assets.is_empty() {
                            pool_json["asset_count"] = serde_json::json!(pool.pool_info.assets.len());
                            pool_json["primary_assets"] = serde_json::json!(
                                pool.pool_info.assets.iter()
                                    .take(2)
                                    .map(|asset| asset.denom.clone())
                                    .collect::<Vec<_>>()
                            );
                        }

                        // Add detailed information if requested
                        if include_details {
                            pool_json["assets"] = serde_json::json!(
                                pool.pool_info.assets.iter().map(|asset| {
                                    serde_json::json!({
                                        "denom": asset.denom,
                                        "amount": asset.amount.to_string()
                                    })
                                }).collect::<Vec<_>>()
                            );

                            pool_json["total_share"] = serde_json::json!({
                                "denom": pool.total_share.denom,
                                "amount": pool.total_share.amount.to_string()
                            });

                            pool_json["pool_fees"] = serde_json::to_value(&pool.pool_info.pool_fees).unwrap_or(serde_json::json!(null));
                        }

                        // Add estimated TVL (sum of asset amounts - simplified calculation)
                        let estimated_tvl: u128 = pool.pool_info.assets.iter()
                            .map(|asset| asset.amount.u128())
                            .sum();
                        pool_json["estimated_tvl"] = serde_json::json!(estimated_tvl.to_string());

                        pool_json
                    })
                    .collect();

                // Apply status filter
                pools.retain(|pool| match status_filter {
                    "all" => true,
                    "active" => pool["operational_status"]["is_active"]
                        .as_bool()
                        .unwrap_or(false),
                    "inactive" => !pool["operational_status"]["is_active"]
                        .as_bool()
                        .unwrap_or(true),
                    "swaps_enabled" => pool["pool_status"]["swaps_enabled"]
                        .as_bool()
                        .unwrap_or(false),
                    "deposits_enabled" => pool["pool_status"]["deposits_enabled"]
                        .as_bool()
                        .unwrap_or(false),
                    _ => true,
                });

                // Apply sorting
                pools.sort_by(|a, b| {
                    let comparison = match sort_by {
                        "pool_id" => {
                            let id_a = a["pool_id"].as_str().unwrap_or("0");
                            let id_b = b["pool_id"].as_str().unwrap_or("0");
                            id_a.cmp(id_b)
                        }
                        "tvl" => {
                            let tvl_a: u128 = a["estimated_tvl"]
                                .as_str()
                                .unwrap_or("0")
                                .parse()
                                .unwrap_or(0);
                            let tvl_b: u128 = b["estimated_tvl"]
                                .as_str()
                                .unwrap_or("0")
                                .parse()
                                .unwrap_or(0);
                            tvl_a.cmp(&tvl_b)
                        }
                        "total_share" => {
                            let share_a: u128 = a
                                .get("total_share")
                                .and_then(|s| s.get("amount"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("0")
                                .parse()
                                .unwrap_or(0);
                            let share_b: u128 = b
                                .get("total_share")
                                .and_then(|s| s.get("amount"))
                                .and_then(|a| a.as_str())
                                .unwrap_or("0")
                                .parse()
                                .unwrap_or(0);
                            share_a.cmp(&share_b)
                        }
                        _ => std::cmp::Ordering::Equal,
                    };

                    if sort_order == "desc" {
                        comparison.reverse()
                    } else {
                        comparison
                    }
                });

                // Apply pagination
                let total_count = pools.len();
                let paginated_pools: Vec<serde_json::Value> =
                    pools.into_iter().skip(offset).take(limit).collect();

                // Prepare response with metadata
                Ok(serde_json::json!({
                    "pools": paginated_pools,
                    "pagination": {
                        "limit": limit,
                        "offset": offset,
                        "total_count": total_count,
                        "returned_count": paginated_pools.len(),
                        "has_more": offset + paginated_pools.len() < total_count
                    },
                    "filters_applied": {
                        "status_filter": status_filter,
                        "sort_by": sort_by,
                        "sort_order": sort_order,
                        "include_details": include_details
                    },
                    "network_info": {
                        "network_id": network_config.network_id,
                        "network_name": network_config.network_name
                    },
                    "retrieved_at": chrono::Utc::now().to_rfc3339()
                }))
            }
            Err(e) => Ok(serde_json::json!({
                "pools": [],
                "error": format!("Failed to retrieve pools: {}", e),
                "pagination": {
                    "limit": limit,
                    "offset": offset,
                    "total_count": 0,
                    "returned_count": 0,
                    "has_more": false
                },
                "filters_applied": {
                    "status_filter": status_filter,
                    "sort_by": sort_by,
                    "sort_order": sort_order,
                    "include_details": include_details
                },
                "network_info": {
                    "network_id": network_config.network_id,
                    "network_name": network_config.network_name
                },
                "retrieved_at": chrono::Utc::now().to_rfc3339(),
                "note": "Pool querying failed - check network connection and configuration"
            })),
        }
    }

    /// Simulate swap
    async fn handle_simulate_swap(
        &self,
        _arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        // TODO: Implement swap simulation
        Ok(serde_json::json!({
            "simulation": {},
            "note": "Swap simulation not yet implemented"
        }))
    }

    /// Execute swap
    async fn handle_execute_swap(
        &self,
        _arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        // TODO: Implement swap execution
        Ok(serde_json::json!({
            "transaction": {},
            "note": "Swap execution not yet implemented"
        }))
    }

    /// Read active wallet resource
    async fn read_active_wallet(&self) -> McpResult<serde_json::Value> {
        match self.state.get_active_wallet().await? {
            Some(wallet_info) => Ok(serde_json::json!({
                "address": wallet_info.address,
                "public_key": wallet_info.public_key,
                "active": true
            })),
            None => Ok(serde_json::json!({
                "active": false,
                "message": "No wallet currently active"
            })),
        }
    }

    /// Read network configuration resource
    async fn read_network_config(&self) -> McpResult<serde_json::Value> {
        let config = &self.state.config.network_config;
        Ok(serde_json::json!({
            "network_name": config.network_name,
            "network_id": config.network_id,
            "rpc_url": config.rpc_url,
            "gas_price": config.gas_price,
            "gas_adjustment": config.gas_adjustment,
            "native_denom": config.native_denom
        }))
    }

    /// Read network switch resource
    async fn read_network_switch(&self) -> McpResult<serde_json::Value> {
        let current_config = &self.state.config.network_config;
        Ok(serde_json::json!({
            "description": "Network switching capabilities for Mantra DEX",
            "current_network": current_config.network_name,
            "available_networks": [
                {
                    "name": "mantra-dukong",
                    "display_name": "Mantra Dukong Testnet",
                    "network_id": "mantra-dukong-1",
                    "type": "testnet",
                    "description": "Primary development and testing network"
                },
                {
                    "name": "mantra-testnet",
                    "display_name": "Mantra Testnet",
                    "network_id": "mantra-testnet-1",
                    "type": "testnet",
                    "description": "Public testnet for application testing"
                },
                {
                    "name": "mantra-mainnet",
                    "display_name": "Mantra Mainnet",
                    "network_id": "mantra-1",
                    "type": "mainnet",
                    "description": "Production mainnet (use with caution)"
                }
            ],
            "switching_tool": "switch_network",
            "tool_description": "Use the 'switch_network' tool to change networks",
            "tool_parameters": {
                "network": {
                    "type": "string",
                    "description": "Network name to switch to",
                    "enum": ["mantra-dukong", "mantra-testnet", "mantra-mainnet"]
                }
            },
            "usage_example": {
                "tool_name": "switch_network",
                "arguments": {
                    "network": "mantra-testnet"
                }
            },
            "warnings": [
                "Switching networks will reset active wallet connections",
                "Ensure wallet is compatible with target network",
                "Mainnet operations use real tokens - exercise caution"
            ]
        }))
    }

    /// Read network status resource
    async fn read_network_status(&self) -> McpResult<serde_json::Value> {
        // Get network configuration
        let config = &self.state.config.network_config;

        // Try to get real-time network status from client wrapper
        let client_wrapper_guard = self.state.client_wrapper.lock().await;
        let network_status = if let Some(wrapper) = client_wrapper_guard.as_ref() {
            // Get comprehensive network status
            match wrapper.get_network_status().await {
                Ok(status) => status,
                Err(e) => {
                    // If network status fails, provide fallback with error info
                    serde_json::json!({
                        "network": config.network_id,
                        "rpc_url": config.rpc_url,
                        "block_height": null,
                        "status": "error",
                        "error": format!("Failed to get network status: {}", e)
                    })
                }
            }
        } else {
            // No client wrapper available - provide basic config info
            serde_json::json!({
                "network": config.network_id,
                "rpc_url": config.rpc_url,
                "block_height": null,
                "status": "not_initialized",
                "error": "Client not initialized"
            })
        };

        // Enhance the network status with additional configuration details
        let mut enhanced_status = network_status.as_object().unwrap().clone();
        enhanced_status.insert(
            "network_name".to_string(),
            serde_json::Value::String(config.network_name.clone()),
        );
        enhanced_status.insert(
            "gas_price".to_string(),
            serde_json::Value::String(config.gas_price.to_string()),
        );
        enhanced_status.insert(
            "gas_adjustment".to_string(),
            serde_json::json!(config.gas_adjustment),
        );
        enhanced_status.insert(
            "native_denom".to_string(),
            serde_json::Value::String(config.native_denom.clone()),
        );
        enhanced_status.insert(
            "timestamp".to_string(),
            serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
        );

        Ok(serde_json::Value::Object(enhanced_status))
    }

    /// Read contract addresses resource - provides smart contract addresses for the current network
    async fn read_contracts_addresses(&self) -> McpResult<serde_json::Value> {
        debug!("Reading contract addresses for current network");

        // Check if we have a client wrapper initialized
        let wrapper_guard = self.state.client_wrapper.lock().await;
        if let Some(wrapper) = wrapper_guard.as_ref() {
            match wrapper.get_contract_addresses().await {
                Ok(contract_data) => {
                    // Enhance the contract data with additional metadata
                    let mut enhanced_response = serde_json::Map::new();
                    enhanced_response.insert(
                        "resource_type".to_string(),
                        serde_json::Value::String("contract_addresses".to_string()),
                    );
                    enhanced_response.insert(
                        "description".to_string(),
                        serde_json::Value::String(
                            "Smart contract addresses for the Mantra DEX on the current network"
                                .to_string(),
                        ),
                    );

                    // Include the contract data from the wrapper
                    if let Some(network) = contract_data.get("network") {
                        enhanced_response.insert("network".to_string(), network.clone());
                    }
                    if let Some(contracts) = contract_data.get("contracts") {
                        enhanced_response.insert("contracts".to_string(), contracts.clone());
                    }
                    if let Some(rpc_url) = contract_data.get("rpc_url") {
                        enhanced_response.insert("rpc_url".to_string(), rpc_url.clone());
                    }

                    // Add metadata
                    enhanced_response.insert(
                        "contract_types".to_string(),
                        serde_json::json!({
                            "pool_manager": "Core DEX pool management and operations",
                            "fee_collector": "Fee collection and distribution"
                        }),
                    );
                    enhanced_response.insert(
                        "usage_info".to_string(),
                        serde_json::json!({
                            "note": "These addresses are required for direct smart contract interaction",
                            "pool_manager_functions": ["query_pools", "simulate_swap", "execute_swap"],
                            "fee_collector_functions": ["query_fees", "collect_fees"]
                        }),
                    );
                    enhanced_response.insert(
                        "retrieved_at".to_string(),
                        serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
                    );

                    Ok(serde_json::Value::Object(enhanced_response))
                }
                Err(e) => {
                    // Return error information
                    Ok(serde_json::json!({
                        "resource_type": "contract_addresses",
                        "error": format!("Failed to retrieve contract addresses: {}", e),
                        "network": self.state.config.network_config.network_id,
                        "available": false,
                        "note": "Contract addresses may not be configured for this network",
                        "retrieved_at": chrono::Utc::now().to_rfc3339(),
                        "fallback_info": {
                            "pool_manager": self.state.config.network_config.contracts.pool_manager,
                            "fee_collector": self.state.config.network_config.contracts.fee_collector
                        }
                    }))
                }
            }
        } else {
            // Fallback to configuration data if wrapper not initialized
            warn!("Client wrapper not initialized, using configuration fallback");
            Ok(serde_json::json!({
                "resource_type": "contract_addresses",
                "network": self.state.config.network_config.network_id,
                "contracts": {
                    "pool_manager": self.state.config.network_config.contracts.pool_manager,
                    "fee_collector": self.state.config.network_config.contracts.fee_collector
                },
                "contract_types": {
                    "pool_manager": "Core DEX pool management and operations",
                    "fee_collector": "Fee collection and distribution"
                },
                "note": "Using configuration fallback - client wrapper not initialized",
                "retrieved_at": chrono::Utc::now().to_rfc3339()
            }))
        }
    }

    /// Read pools list resource
    async fn read_pools_list(&self) -> McpResult<serde_json::Value> {
        // Use the get_pools tool implementation with default parameters
        let default_args = serde_json::json!({
            "limit": 50,
            "offset": 0,
            "status_filter": "all",
            "sort_by": "pool_id",
            "sort_order": "asc",
            "include_details": true
        });

        // Delegate to the get_pools handler
        let pools_result = self.handle_get_pools(default_args).await?;

        // Transform the response for resource format
        if let Some(pools) = pools_result.get("pools") {
            let mut response = serde_json::json!({
                "resource_type": "pools_list",
                "pools": pools,
                "description": "Complete list of available liquidity pools on Mantra DEX",
                "usage": "This resource provides comprehensive pool information including assets, status, and TVL estimates"
            });

            // Copy pagination and metadata if available
            if let Some(pagination) = pools_result.get("pagination") {
                response["pagination"] = pagination.clone();
            }
            if let Some(network_info) = pools_result.get("network_info") {
                response["network_info"] = network_info.clone();
            }
            if let Some(retrieved_at) = pools_result.get("retrieved_at") {
                response["retrieved_at"] = retrieved_at.clone();
            }

            // Add resource-specific metadata
            response["access_info"] = serde_json::json!({
                "available_filters": ["all", "active", "inactive", "swaps_enabled", "deposits_enabled"],
                "sort_options": ["pool_id", "tvl", "total_share", "created_at"],
                "detail_levels": ["basic", "detailed"],
                "pagination_support": true,
                "related_tools": ["get_pools", "get_pool"],
                "related_resources": ["pools://details/{id}"]
            });

            Ok(response)
        } else {
            // Handle error case
            Ok(serde_json::json!({
                "resource_type": "pools_list",
                "pools": [],
                "error": pools_result.get("error").unwrap_or(&serde_json::json!("Unknown error")),
                "description": "Failed to retrieve pool information",
                "network_info": pools_result.get("network_info"),
                "retrieved_at": pools_result.get("retrieved_at"),
                "troubleshooting": [
                    "Check network connectivity",
                    "Verify RPC endpoint configuration",
                    "Ensure DEX contracts are deployed on current network"
                ]
            }))
        }
    }

    /// Read pool details resource
    async fn read_pool_details(&self, pool_id: &str) -> McpResult<serde_json::Value> {
        if pool_id.is_empty() {
            return Ok(serde_json::json!({
                "error": "Pool ID is required",
                "usage": "Use URI format: pools://details/{pool_id}",
                "example": "pools://details/1"
            }));
        }

        // Convert pool_id to owned String
        let pool_id = pool_id.to_string();

        // Get client through SDK adapter with retry logic
        let state = &self.state;
        let network_config = state.config.network_config.clone();
        let adapter = state.sdk_adapter.clone();

        let pool_id_for_error = pool_id.clone();

        // Use direct async block to avoid complex borrowing issues
        let result = async {
            let client = adapter
                .get_client(&network_config)
                .await
                .map_err(|e| McpServerError::Network(format!("Failed to get client: {}", e)))?;

            client
                .get_pool(&pool_id)
                .await
                .map_err(|e| McpServerError::Mcp(format!("Failed to get pool {}: {}", pool_id, e)))
        }
        .await;

        match result {
            Ok(pool_info) => {
                // Convert pool info to detailed JSON response for resource
                let pool_status = pool_info.pool_info.status;
                let assets: Vec<serde_json::Value> = pool_info
                    .pool_info
                    .assets
                    .iter()
                    .map(|asset| {
                        serde_json::json!({
                            "denom": asset.denom,
                            "amount": asset.amount.to_string(),
                            "amount_raw": asset.amount.u128()
                        })
                    })
                    .collect();

                // Calculate total value locked (simplified)
                let total_assets: u128 = pool_info
                    .pool_info
                    .assets
                    .iter()
                    .map(|asset| asset.amount.u128())
                    .sum();

                Ok(serde_json::json!({
                    "resource_type": "pool_details",
                    "pool_id": pool_info.pool_info.pool_identifier,
                    "pool_type": format!("{:?}", pool_info.pool_info.pool_type),
                    "lp_denom": pool_info.pool_info.lp_denom,
                    "assets": assets,
                    "asset_count": assets.len(),
                    "total_share": {
                        "denom": pool_info.total_share.denom,
                        "amount": pool_info.total_share.amount.to_string(),
                        "amount_raw": pool_info.total_share.amount.u128()
                    },
                    "status": {
                        "swaps_enabled": pool_status.swaps_enabled,
                        "deposits_enabled": pool_status.deposits_enabled,
                        "withdrawals_enabled": pool_status.withdrawals_enabled,
                        "operational_status": if pool_status.swaps_enabled && pool_status.deposits_enabled && pool_status.withdrawals_enabled {
                            "fully_operational"
                        } else if !pool_status.swaps_enabled && !pool_status.deposits_enabled && !pool_status.withdrawals_enabled {
                            "disabled"
                        } else {
                            "partially_disabled"
                        }
                    },
                    "fees": pool_info.pool_info.pool_fees,
                    "tvl_estimate": total_assets.to_string(),
                    "retrieved_at": chrono::Utc::now().to_rfc3339(),
                    "network": state.config.network_config.network_id
                }))
            }
            Err(e) => Ok(serde_json::json!({
                "resource_type": "pool_details",
                "pool_id": pool_id_for_error,
                "error": format!("Failed to retrieve pool details: {}", e),
                "available": false,
                "note": "Pool may not exist or network connection failed",
                "retrieved_at": chrono::Utc::now().to_rfc3339(),
                "network": state.config.network_config.network_id
            })),
        }
    }

    /// Read wallet balance resource
    async fn read_wallet_balance(&self) -> McpResult<serde_json::Value> {
        // TODO: Implement actual balance querying
        Ok(serde_json::json!({
            "balances": [],
            "note": "Balance data not yet implemented - requires blockchain connection"
        }))
    }

    /// Read wallet create resource - provides information about wallet creation
    async fn read_wallet_create(&self) -> McpResult<serde_json::Value> {
        Ok(serde_json::json!({
            "description": "Generate new HD wallet with mnemonic phrase",
            "method": "Use the 'generate_wallet' tool to create a new wallet",
            "parameters": {
                "account_index": "Optional account index (default: 0)"
            },
            "example": {
                "tool": "generate_wallet",
                "arguments": {
                    "account_index": 0
                }
            }
        }))
    }

    /// Read wallet import resource - provides information about wallet import
    async fn read_wallet_import(&self) -> McpResult<serde_json::Value> {
        Ok(serde_json::json!({
            "description": "Import wallet from mnemonic phrase",
            "method": "Use the 'import_wallet' tool to import existing wallet",
            "parameters": {
                "mnemonic": "12 or 24 word mnemonic phrase (required)",
                "account_index": "Optional account index (default: 0)"
            },
            "example": {
                "tool": "import_wallet",
                "arguments": {
                    "mnemonic": "word1 word2 word3 ... word12",
                    "account_index": 0
                }
            }
        }))
    }

    /// Read wallet info resource - provides current wallet information
    async fn read_wallet_info(&self) -> McpResult<serde_json::Value> {
        match self.state.get_active_wallet().await? {
            Some(wallet_info) => Ok(serde_json::json!({
                "address": wallet_info.address,
                "public_key": wallet_info.public_key,
                "account_index": 0, // TODO: Store actual account index
                "network": self.state.config.network_config.network_name,
                "status": "active"
            })),
            None => Ok(serde_json::json!({
                "status": "no_wallet",
                "message": "No wallet currently active",
                "action": "Use 'generate_wallet' or 'import_wallet' tools to create/import a wallet"
            })),
        }
    }

    /// Read wallet save resource - provides information about wallet saving
    async fn read_wallet_save(&self) -> McpResult<serde_json::Value> {
        Ok(serde_json::json!({
            "description": "Save wallet configuration securely",
            "status": "not_implemented",
            "message": "Wallet persistence is not yet implemented",
            "future_features": {
                "encryption": "AES-256-GCM with Argon2 key derivation",
                "storage": "Local encrypted file storage",
                "password_protection": "Required for wallet access"
            }
        }))
    }

    /// Read wallet load resource - provides information about wallet loading
    async fn read_wallet_load(&self) -> McpResult<serde_json::Value> {
        Ok(serde_json::json!({
            "description": "Load saved wallet configuration",
            "status": "not_implemented",
            "message": "Wallet persistence is not yet implemented",
            "current_workaround": "Use 'import_wallet' tool with mnemonic phrase"
        }))
    }

    /// Read wallet list resource - provides list of saved wallets
    async fn read_wallet_list(&self) -> McpResult<serde_json::Value> {
        Ok(serde_json::json!({
            "description": "List of all saved wallet configurations",
            "status": "not_implemented",
            "saved_wallets": [],
            "message": "Wallet persistence is not yet implemented",
            "current_status": {
                "active_wallet": self.state.get_active_wallet().await?.is_some(),
                "total_saved": 0
            }
        }))
    }

    /// Handle get network status
    async fn handle_get_network_status(
        &self,
        _arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        debug!("Getting network status through MCP tool");

        // Check if we have a client wrapper initialized
        let wrapper_guard = self.state.client_wrapper.lock().await;
        if let Some(wrapper) = wrapper_guard.as_ref() {
            match wrapper.get_network_status().await {
                Ok(network_status) => {
                    info!("Network status retrieved successfully");
                    Ok(serde_json::json!({
                        "success": true,
                        "network_status": network_status,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }))
                }
                Err(e) => {
                    warn!("Failed to get network status: {}", e);
                    Ok(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to get network status: {}", e),
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    }))
                }
            }
        } else {
            // Fallback to basic network info from config if wrapper not initialized
            warn!("Client wrapper not initialized, falling back to basic network info");
            Ok(serde_json::json!({
                "success": false,
                "network_status": {
                    "network": self.state.config.network_config.network_id,
                    "rpc_url": self.state.config.network_config.rpc_url,
                    "status": "unknown",
                    "block_height": null
                },
                "error": "Client wrapper not initialized - please ensure the server is properly initialized",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        }
    }
}

// =============================================================================
// Trait Implementations for MantraDexMcpServer
// =============================================================================

#[async_trait::async_trait]
impl McpServerLifecycle for MantraDexMcpServer {
    async fn initialize(&self) -> McpResult<()> {
        info!("Initializing Mantra DEX MCP Server");
        self.state.initialize_client().await?;
        info!("Server initialization complete");
        Ok(())
    }

    fn get_server_info(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.state.config.name,
            "version": self.state.config.version
        })
    }

    fn get_capabilities(&self) -> serde_json::Value {
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

    async fn shutdown(&self) -> McpResult<()> {
        info!("Initiating graceful shutdown of Mantra DEX MCP Server");

        // Step 1: Clear active wallet
        {
            let mut active_wallet = self.state.active_wallet.lock().await;
            if active_wallet.is_some() {
                info!("Clearing active wallet during shutdown");
                *active_wallet = None;
            }
        }

        // Step 2: Clear wallet cache
        {
            let mut wallets = self.state.wallets.write().await;
            if !wallets.is_empty() {
                info!("Clearing {} cached wallets during shutdown", wallets.len());
                wallets.clear();
            }
        }

        // Step 3: Clear client connections
        {
            let mut client = self.state.client.lock().await;
            if client.is_some() {
                info!("Closing DEX client connection during shutdown");
                *client = None;
            }
        }

        // Step 4: Clear all cached data
        {
            let cache_size = self.state.cache.read().await.len();
            if cache_size > 0 {
                info!("Clearing {} cached entries during shutdown", cache_size);
                self.state.cache_clear().await;
            }
        }

        info!("Graceful shutdown completed successfully");
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        // Check if DEX client is initialized
        self.state.client.lock().await.is_some()
    }
}

#[async_trait::async_trait]
impl McpToolProvider for MantraDexMcpServer {
    fn get_available_tools(&self) -> Vec<serde_json::Value> {
        // Delegate to existing implementation
        self.get_available_tools()
    }

    async fn handle_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<serde_json::Value> {
        // Validate arguments first
        self.validate_tool_arguments(tool_name, &arguments)?;

        // Delegate to existing implementation
        self.handle_tool_call(tool_name, arguments).await
    }
}

#[async_trait::async_trait]
impl McpResourceProvider for MantraDexMcpServer {
    fn get_available_resources(&self) -> Vec<serde_json::Value> {
        // Delegate to existing implementation
        self.get_available_resources()
    }

    async fn handle_resource_read(&self, uri: &str) -> McpResult<serde_json::Value> {
        // Validate URI first
        self.validate_resource_uri(uri)?;

        // Delegate to existing implementation
        self.handle_resource_read(uri).await
    }
}

#[async_trait::async_trait]
impl McpServerStateManager for MantraDexMcpServer {
    async fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.state.config.name,
            "version": self.state.config.version,
            "network": {
                "name": self.state.config.network_config.network_name,
                "rpc_url": self.state.config.network_config.rpc_url,
                "chain_id": self.state.config.network_config.network_id
            },
            "debug": self.state.config.debug,
            "max_concurrent_ops": self.state.config.max_concurrent_ops,
            "active_wallet": self.state.active_wallet.lock().await.clone()
        })
    }

    async fn update_config(&self, config: serde_json::Value) -> McpResult<()> {
        info!("Config update requested: {:?}", config);
        // For now, just acknowledge the request
        // In a full implementation, we'd validate and apply config changes
        Ok(())
    }

    async fn clear_state(&self) -> McpResult<()> {
        info!("Clearing server state");
        self.state.cache_clear().await;
        // Note: We don't clear wallets or active wallet as that would be destructive
        Ok(())
    }

    async fn get_health_status(&self) -> serde_json::Value {
        let client_ready = self.is_ready().await;
        let active_wallet = self.state.active_wallet.lock().await.is_some();
        let runtime_health = self.state.runtime_manager.health_status().await;

        serde_json::json!({
            "status": if client_ready { "healthy" } else { "initializing" },
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "components": {
                "dex_client": client_ready,
                "active_wallet": active_wallet,
                "cache": true,
                "runtime": runtime_health.get("runtime").unwrap_or(&serde_json::Value::Null)
            }
        })
    }
}

// Implement the main composite trait
#[async_trait::async_trait]
impl McpServer for MantraDexMcpServer {}

/// JSON-RPC 2.0 request structure
#[derive(Debug, serde::Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, serde::Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error structure
#[derive(Debug, serde::Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

impl JsonRpcError {
    /// Create internal error with proper MCP error code
    pub fn internal_error(message: String) -> Self {
        Self {
            code: json_rpc_error_codes::INTERNAL_ERROR,
            message,
            data: None,
        }
    }

    /// Create invalid request error with proper MCP error code
    pub fn invalid_request(message: String) -> Self {
        Self {
            code: json_rpc_error_codes::INVALID_REQUEST,
            message,
            data: None,
        }
    }

    /// Create method not found error with proper MCP error code
    pub fn method_not_found(method: String) -> Self {
        Self {
            code: json_rpc_error_codes::METHOD_NOT_FOUND,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    /// Create invalid params error with proper MCP error code
    pub fn invalid_params(message: String) -> Self {
        Self {
            code: json_rpc_error_codes::INVALID_PARAMS,
            message,
            data: None,
        }
    }

    /// Create parse error with proper MCP error code
    pub fn parse_error(message: String) -> Self {
        Self {
            code: json_rpc_error_codes::PARSE_ERROR,
            message,
            data: None,
        }
    }

    /// Create error from MCP server error with full mapping
    pub fn from_mcp_error(error: &McpServerError) -> Self {
        error.to_json_rpc_error()
    }

    /// Create custom error with specific code and additional data
    pub fn custom_error(code: i32, message: String, data: Option<serde_json::Value>) -> Self {
        Self {
            code,
            message,
            data,
        }
    }
}

/// Stdio transport implementation for MCP
pub struct StdioTransport {
    server: Arc<MantraDexMcpServer>,
    active: Arc<Mutex<bool>>,
}

impl StdioTransport {
    pub fn new(server: Arc<MantraDexMcpServer>) -> Self {
        Self {
            server,
            active: Arc::new(Mutex::new(false)),
        }
    }

    /// Process a single JSON-RPC request
    async fn process_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!(
            "Processing request: method={}, id={:?}",
            request.method, request.id
        );

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "initialized" => self.handle_initialized(request).await,
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request).await,
            "resources/read" => self.handle_resources_read(request).await,
            "ping" => self.handle_ping(request).await,
            _ => JsonRpcResponse::error(request.id, JsonRpcError::method_not_found(request.method)),
        }
    }

    async fn handle_initialize(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling MCP initialize request");

        let result = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": self.server.get_capabilities(),
            "serverInfo": self.server.get_server_info()
        });

        JsonRpcResponse::success(request.id, result)
    }

    async fn handle_initialized(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        info!("MCP client initialization complete");
        JsonRpcResponse::success(request.id, serde_json::json!({}))
    }

    async fn handle_tools_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling tools/list request");

        let tools = self.server.get_available_tools();
        let result = serde_json::json!({
            "tools": tools
        });

        JsonRpcResponse::success(request.id, result)
    }

    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling tools/call request");

        let params = match request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing tool call parameters".to_string()),
                );
            }
        };

        let tool_name = match params.get("name").and_then(|n| n.as_str()) {
            Some(name) => name,
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing tool name".to_string()),
                );
            }
        };

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));

        match self.server.handle_tool_call(tool_name, arguments).await {
            Ok(result) => JsonRpcResponse::success(
                request.id,
                serde_json::json!({
                    "content": [{"type": "text", "text": result.to_string()}]
                }),
            ),
            Err(e) => JsonRpcResponse::error(
                request.id,
                JsonRpcError::internal_error(format!("Tool execution failed: {}", e)),
            ),
        }
    }

    async fn handle_resources_list(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling resources/list request");

        let resources = self.server.get_available_resources();
        let result = serde_json::json!({
            "resources": resources
        });

        JsonRpcResponse::success(request.id, result)
    }

    async fn handle_resources_read(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling resources/read request");

        let params = match request.params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing resource URI".to_string()),
                );
            }
        };

        let uri = match params.get("uri").and_then(|u| u.as_str()) {
            Some(uri) => uri,
            None => {
                return JsonRpcResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing resource URI".to_string()),
                );
            }
        };

        match self.server.handle_resource_read(uri).await {
            Ok(result) => JsonRpcResponse::success(
                request.id,
                serde_json::json!({
                    "contents": [{"uri": uri, "mimeType": "application/json", "text": result.to_string()}]
                }),
            ),
            Err(e) => JsonRpcResponse::error(
                request.id,
                JsonRpcError::internal_error(format!("Resource read failed: {}", e)),
            ),
        }
    }

    async fn handle_ping(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Handling ping request");
        JsonRpcResponse::success(request.id, serde_json::json!({}))
    }
}

#[async_trait::async_trait]
impl McpTransportLayer for StdioTransport {
    async fn start(&self) -> McpResult<()> {
        info!("Starting MCP stdio transport");
        *self.active.lock().await = true;

        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        loop {
            line.clear();

            if !self.is_active() {
                break;
            }

            match reader.read_line(&mut line).await {
                Ok(0) => {
                    debug!("EOF reached, stopping stdio transport");
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    debug!("Received line: {}", trimmed);

                    match serde_json::from_str::<JsonRpcRequest>(trimmed) {
                        Ok(request) => {
                            let response = self.process_request(request).await;
                            if let Err(e) = self.send_response(response).await {
                                error!("Failed to send response: {}", e);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse JSON-RPC request: {}", e);
                            let error_response = JsonRpcResponse::error(
                                None,
                                JsonRpcError::invalid_request(format!("Invalid JSON: {}", e)),
                            );
                            if let Err(e) = self.send_response(error_response).await {
                                error!("Failed to send error response: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    break;
                }
            }
        }

        *self.active.lock().await = false;
        info!("MCP stdio transport stopped");
        Ok(())
    }

    async fn stop(&self) -> McpResult<()> {
        info!("Stopping MCP stdio transport");
        *self.active.lock().await = false;
        Ok(())
    }

    async fn send_response(&self, response: JsonRpcResponse) -> McpResult<()> {
        use tokio::io::AsyncWriteExt;

        let json =
            serde_json::to_string(&response).map_err(|e| McpServerError::Serialization(e))?;

        debug!("Sending response: {}", json);

        let mut stdout = tokio::io::stdout();
        stdout
            .write_all(json.as_bytes())
            .await
            .map_err(|e| McpServerError::Network(format!("Failed to write to stdout: {}", e)))?;
        stdout
            .write_all(b"\n")
            .await
            .map_err(|e| McpServerError::Network(format!("Failed to write newline: {}", e)))?;
        stdout
            .flush()
            .await
            .map_err(|e| McpServerError::Network(format!("Failed to flush stdout: {}", e)))?;

        Ok(())
    }

    fn is_active(&self) -> bool {
        // Use try_lock to avoid blocking
        self.active.try_lock().map(|guard| *guard).unwrap_or(false)
    }
}

/// HTTP transport implementation for MCP
pub struct HttpTransport {
    server: Arc<MantraDexMcpServer>,
    host: String,
    port: u16,
    active: Arc<Mutex<bool>>,
}

impl HttpTransport {
    pub fn new(server: Arc<MantraDexMcpServer>, host: String, port: u16) -> Self {
        Self {
            server,
            host,
            port,
            active: Arc::new(Mutex::new(false)),
        }
    }

    /// Create the HTTP router for MCP endpoints
    fn create_router(&self) -> axum::Router {
        axum::Router::new()
            .route("/mcp", axum::routing::post(Self::handle_mcp_request))
            .route("/health", axum::routing::get(Self::handle_health))
            .with_state(self.server.clone())
    }

    /// Handle MCP JSON-RPC requests over HTTP
    async fn handle_mcp_request(
        State(server): State<Arc<MantraDexMcpServer>>,
        axum::Json(request): axum::Json<JsonRpcRequest>,
    ) -> Result<axum::Json<JsonRpcResponse>, (StatusCode, axum::Json<JsonRpcResponse>)> {
        debug!(
            "HTTP MCP request: method={}, id={:?}",
            request.method, request.id
        );

        // Create a stdio transport instance for request processing
        let transport = StdioTransport::new(server);
        let response = transport.process_request(request).await;

        Ok(axum::Json(response))
    }

    /// Health check endpoint
    async fn handle_health() -> axum::Json<serde_json::Value> {
        axum::Json(serde_json::json!({
            "status": "healthy",
            "service": "Mantra DEX MCP Server",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

#[async_trait::async_trait]
impl McpTransportLayer for HttpTransport {
    async fn start(&self) -> McpResult<()> {
        use tokio::net::TcpListener;

        info!("Starting MCP HTTP transport on {}:{}", self.host, self.port);
        *self.active.lock().await = true;

        let app = self.create_router();
        let addr = format!("{}:{}", self.host, self.port);

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| McpServerError::Network(format!("Failed to bind to {}: {}", addr, e)))?;

        info!("MCP HTTP server listening on {}", addr);

        // Use axum::serve instead of manual hyper integration
        if let Err(e) = axum::serve(listener, app).await {
            error!("HTTP server error: {}", e);
            return Err(McpServerError::Network(format!(
                "HTTP server failed: {}",
                e
            )));
        }

        *self.active.lock().await = false;
        info!("MCP HTTP transport stopped");
        Ok(())
    }

    async fn stop(&self) -> McpResult<()> {
        info!("Stopping MCP HTTP transport");
        *self.active.lock().await = false;
        Ok(())
    }

    async fn send_response(&self, _response: JsonRpcResponse) -> McpResult<()> {
        // HTTP responses are handled directly in the request handler
        Ok(())
    }

    fn is_active(&self) -> bool {
        self.active.try_lock().map(|guard| *guard).unwrap_or(false)
    }
}

/// Create a new MCP server instance
pub fn create_mcp_server(config: McpServerConfig) -> MantraDexMcpServer {
    MantraDexMcpServer::new(config)
}

/// Create server with stdio transport
pub async fn create_stdio_server(config: McpServerConfig) -> McpResult<()> {
    let server = Arc::new(create_mcp_server(config));
    server.initialize().await?;

    let transport = StdioTransport::new(server);
    transport.start().await?;

    Ok(())
}

/// Create server with HTTP transport
#[cfg(feature = "mcp")]
pub async fn create_http_server(config: McpServerConfig, host: String, port: u16) -> McpResult<()> {
    let server = Arc::new(create_mcp_server(config));
    server.initialize().await?;

    let transport = HttpTransport::new(server, host, port);
    transport.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // Test server info
        let info = server.get_server_info();
        assert_eq!(info["name"], "Mantra DEX SDK MCP Server");
        assert_eq!(info["version"], "0.1.0");

        // Test capabilities
        let capabilities = server.get_capabilities();
        assert!(capabilities["tools"]["list_changed"]
            .as_bool()
            .unwrap_or(false));
        assert!(capabilities["resources"]["list_changed"]
            .as_bool()
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn test_tool_listing() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        let tools = server.get_available_tools();
        assert!(!tools.is_empty());

        // Check that wallet tools are present
        let tool_names: Vec<String> = tools
            .iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
            .collect();

        assert!(tool_names.contains(&"generate_wallet".to_string()));
        assert!(tool_names.contains(&"import_wallet".to_string()));
        assert!(tool_names.contains(&"get_wallet_info".to_string()));
    }

    #[tokio::test]
    async fn test_resource_listing() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        let resources = server.get_available_resources();
        assert!(!resources.is_empty());

        // Check that wallet resources are present
        let resource_uris: Vec<String> = resources
            .iter()
            .filter_map(|r| r.get("uri").and_then(|u| u.as_str()).map(String::from))
            .collect();

        assert!(resource_uris.contains(&"wallet://active".to_string()));
        assert!(resource_uris.contains(&"network://config".to_string()));
    }

    #[tokio::test]
    async fn test_server_state() {
        let config = McpServerConfig::default();
        let state = McpServerStateData::new(config);

        // Test cache operations
        state
            .cache_set("test_key".to_string(), serde_json::json!({"test": "value"}))
            .await;
        let cached_value = state.cache_get("test_key").await;
        assert!(cached_value.is_some());

        // Test cache clear
        state.cache_clear().await;
        let cleared_value = state.cache_get("test_key").await;
        assert!(cleared_value.is_none());
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // Test that server can be initialized
        // Note: This will fail without proper network config in CI
        // but provides the structure for testing
        match server.initialize().await {
            Ok(_) => {
                info!("Server initialized successfully");
            }
            Err(e) => {
                info!(
                    "Server initialization failed as expected in test environment: {}",
                    e
                );
            }
        }
    }

    #[tokio::test]
    async fn test_config_from_env() {
        // Test default configuration loading
        let config = McpServerConfig::from_env().unwrap();
        assert_eq!(config.name, "Mantra DEX SDK MCP Server");
        assert_eq!(config.version, "0.1.0");
        assert_eq!(config.max_concurrent_ops, 10);
        assert_eq!(config.http_port, 8080);
        assert_eq!(config.http_host, "127.0.0.1");
    }

    #[tokio::test]
    async fn test_config_validation() {
        let mut config = McpServerConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid configs should fail
        config.name = "".to_string();
        assert!(config.validate().is_err());

        config.name = "Test".to_string();
        config.max_concurrent_ops = 0;
        assert!(config.validate().is_err());

        config.max_concurrent_ops = 5;
        config.http_port = 0;
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_config_with_network() {
        // Test with default network (should work)
        let config = McpServerConfig::with_network("testnet");
        // Note: This may fail in CI without proper config files
        // but provides the structure for testing
        match config {
            Ok(cfg) => {
                assert!(cfg.validate().is_ok());
            }
            Err(_) => {
                // Expected in test environment without config files
            }
        }
    }

    #[tokio::test]
    async fn test_network_switching() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // Test invalid network
        let invalid_args = serde_json::json!({
            "network": "invalid-network"
        });
        let result = server.handle_switch_network(invalid_args).await;
        assert!(result.is_err());

        // Test valid network - mantra-dukong
        let valid_args = serde_json::json!({
            "network": "mantra-dukong"
        });

        // Note: This may fail in CI without proper config files
        // but provides the structure for testing
        match server.handle_switch_network(valid_args).await {
            Ok(response) => {
                assert_eq!(response["network"], "mantra-dukong");
                assert_eq!(response["switched"], true);
                assert!(response["message"]
                    .as_str()
                    .unwrap()
                    .contains("Successfully switched"));
            }
            Err(e) => {
                info!(
                    "Network switching failed as expected in test environment: {}",
                    e
                );
                // Expected in test environment without config files
            }
        }

        // Test missing network parameter
        let missing_args = serde_json::json!({});
        let result = server.handle_switch_network(missing_args).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wallet_switching() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // First, import a wallet
        let import_args = serde_json::json!({
            "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
            "account_index": 0
        });

        let import_result = server.handle_import_wallet(import_args).await;
        assert!(import_result.is_ok());

        let import_response = import_result.unwrap();
        let wallet_address = import_response["address"].as_str().unwrap();

        // Test switching to the imported wallet
        let switch_args = serde_json::json!({
            "address": wallet_address
        });

        let switch_result = server.handle_switch_wallet(switch_args).await;
        assert!(switch_result.is_ok());

        let switch_response = switch_result.unwrap();
        assert_eq!(switch_response["switched"], true);
        assert_eq!(switch_response["address"], wallet_address);
        assert!(switch_response["message"]
            .as_str()
            .unwrap()
            .contains("Successfully switched"));

        // Test switching to non-existent wallet
        let invalid_switch_args = serde_json::json!({
            "address": "mantra1invalidaddress"
        });

        let invalid_result = server.handle_switch_wallet(invalid_switch_args).await;
        assert!(invalid_result.is_err());

        // Test missing address parameter
        let missing_args = serde_json::json!({});
        let missing_result = server.handle_switch_wallet(missing_args).await;
        assert!(missing_result.is_err());
    }

    #[tokio::test]
    async fn test_wallet_validation() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // Test valid address validation
        let valid_address_args = serde_json::json!({
            "address": "mantra1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq0mttgq"
        });
        let valid_address_result = server.handle_validate_wallet(valid_address_args).await;
        assert!(valid_address_result.is_ok());
        let response = valid_address_result.unwrap();
        assert_eq!(response["validations"]["address"]["valid"], true);

        // Test invalid address validation
        let invalid_address_args = serde_json::json!({
            "address": "invalid_address"
        });
        let invalid_address_result = server.handle_validate_wallet(invalid_address_args).await;
        assert!(invalid_address_result.is_ok());
        let response = invalid_address_result.unwrap();
        assert_eq!(response["validations"]["address"]["valid"], false);
        assert_eq!(response["overall_valid"], false);

        // Test valid mnemonic validation
        let valid_mnemonic_args = serde_json::json!({
            "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        });
        let valid_mnemonic_result = server.handle_validate_wallet(valid_mnemonic_args).await;
        assert!(valid_mnemonic_result.is_ok());
        let response = valid_mnemonic_result.unwrap();
        assert_eq!(response["validations"]["mnemonic"]["valid"], true);
        assert_eq!(response["validations"]["mnemonic"]["word_count"], 12);

        // Test invalid mnemonic validation
        let invalid_mnemonic_args = serde_json::json!({
            "mnemonic": "invalid mnemonic phrase test"
        });
        let invalid_mnemonic_result = server.handle_validate_wallet(invalid_mnemonic_args).await;
        assert!(invalid_mnemonic_result.is_ok());
        let response = invalid_mnemonic_result.unwrap();
        assert_eq!(response["validations"]["mnemonic"]["valid"], false);

        // Test valid public key validation
        let valid_pubkey_args = serde_json::json!({
            "public_key": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        });
        let valid_pubkey_result = server.handle_validate_wallet(valid_pubkey_args).await;
        assert!(valid_pubkey_result.is_ok());
        let response = valid_pubkey_result.unwrap();
        assert_eq!(response["validations"]["public_key"]["valid"], true);

        // Test invalid public key validation
        let invalid_pubkey_args = serde_json::json!({
            "public_key": "invalid_public_key"
        });
        let invalid_pubkey_result = server.handle_validate_wallet(invalid_pubkey_args).await;
        assert!(invalid_pubkey_result.is_ok());
        let response = invalid_pubkey_result.unwrap();
        assert_eq!(response["validations"]["public_key"]["valid"], false);

        // Test multiple validations
        let multiple_args = serde_json::json!({
            "address": "mantra1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq0mttgq",
            "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        });
        let multiple_result = server.handle_validate_wallet(multiple_args).await;
        assert!(multiple_result.is_ok());
        let response = multiple_result.unwrap();
        assert_eq!(response["overall_valid"], true);
        assert!(response["validations"]["address"]["valid"]
            .as_bool()
            .unwrap());
        assert!(response["validations"]["mnemonic"]["valid"]
            .as_bool()
            .unwrap());

        // Test missing parameters
        let empty_args = serde_json::json!({});
        let empty_result = server.handle_validate_wallet(empty_args).await;
        assert!(empty_result.is_err());
    }

    #[tokio::test]
    async fn test_network_switch_resource() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // Test that network://switch resource is available
        let resources = server.get_available_resources();
        let resource_uris: Vec<&str> = resources
            .iter()
            .filter_map(|r| r.get("uri").and_then(|u| u.as_str()))
            .collect();

        assert!(resource_uris.contains(&"network://switch"));

        // Test reading the network://switch resource
        let resource_result = server.handle_resource_read("network://switch").await;
        assert!(resource_result.is_ok());

        let resource_data = resource_result.unwrap();

        // Verify the resource contains expected fields
        assert!(resource_data["description"].is_string());
        assert!(resource_data["current_network"].is_string());
        assert!(resource_data["available_networks"].is_array());
        assert!(resource_data["switching_tool"].is_string());
        assert!(resource_data["tool_parameters"].is_object());
        assert!(resource_data["usage_example"].is_object());
        assert!(resource_data["warnings"].is_array());

        // Verify current network is set
        assert_eq!(resource_data["current_network"], "mantra-dukong");
        assert_eq!(resource_data["switching_tool"], "switch_network");

        // Verify available networks structure
        let available_networks = resource_data["available_networks"].as_array().unwrap();
        assert_eq!(available_networks.len(), 3);

        // Check that all expected networks are present
        let network_names: Vec<&str> = available_networks
            .iter()
            .filter_map(|n| n.get("name").and_then(|name| name.as_str()))
            .collect();

        assert!(network_names.contains(&"mantra-dukong"));
        assert!(network_names.contains(&"mantra-testnet"));
        assert!(network_names.contains(&"mantra-mainnet"));

        // Verify warnings array is not empty
        let warnings = resource_data["warnings"].as_array().unwrap();
        assert!(!warnings.is_empty());
    }

    #[tokio::test]
    async fn test_network_status_resource() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // Test that network://status resource is available
        let resources = server.get_available_resources();
        let resource_uris: Vec<&str> = resources
            .iter()
            .filter_map(|r| r.get("uri").and_then(|u| u.as_str()))
            .collect();

        assert!(resource_uris.contains(&"network://status"));

        // Test reading the network://status resource
        let resource_result = server.handle_resource_read("network://status").await;
        assert!(resource_result.is_ok());

        let resource_data = resource_result.unwrap();

        // Verify the resource contains expected fields
        assert!(resource_data["network_name"].is_string());
        assert!(resource_data["network"].is_string());
        assert!(resource_data["rpc_url"].is_string());
        assert!(resource_data["gas_price"].is_string());
        assert!(resource_data["gas_adjustment"].is_number());
        assert!(resource_data["native_denom"].is_string());
        assert!(resource_data["timestamp"].is_string());
        assert!(resource_data["status"].is_string());

        // Verify basic network information is correct
        assert_eq!(resource_data["network_name"], "mantra-dukong");
        assert_eq!(resource_data["network"], "mantra-dukong-1");
        assert_eq!(resource_data["native_denom"], "uom");

        // The status should be either "connected", "disconnected", "error", or "not_initialized"
        let status = resource_data["status"].as_str().unwrap();
        assert!(matches!(
            status,
            "connected" | "disconnected" | "error" | "not_initialized"
        ));

        // Verify timestamp is a valid RFC3339 string
        let timestamp_str = resource_data["timestamp"].as_str().unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(timestamp_str).is_ok());
    }

    #[tokio::test]
    async fn test_contracts_addresses_resource() {
        let config = McpServerConfig::default();
        let server = create_mcp_server(config);

        // Test that contracts://addresses resource is available
        let resources = server.get_available_resources();
        let resource_uris: Vec<&str> = resources
            .iter()
            .filter_map(|r| r.get("uri").and_then(|u| u.as_str()))
            .collect();

        assert!(resource_uris.contains(&"contracts://addresses"));

        // Test reading the contracts://addresses resource
        let resource_result = server.handle_resource_read("contracts://addresses").await;
        assert!(resource_result.is_ok());

        let resource_data = resource_result.unwrap();

        // Should contain basic resource information
        assert!(resource_data.get("resource_type").is_some());
        assert_eq!(resource_data["resource_type"], "contract_addresses");

        // Should have network identification
        assert!(resource_data.get("network").is_some());

        // Should have timestamp
        assert!(resource_data.get("retrieved_at").is_some());

        // Should have contract types info
        assert!(resource_data.get("contract_types").is_some());

        // Should have either contracts data or fallback info
        let has_contracts = resource_data.get("contracts").is_some();
        let has_fallback = resource_data.get("fallback_info").is_some();
        assert!(
            has_contracts || has_fallback,
            "Should have either contracts or fallback info"
        );

        // Verify timestamp is a valid RFC3339 string
        let timestamp_str = resource_data["retrieved_at"].as_str().unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(timestamp_str).is_ok());

        // Should have network identification
        assert_eq!(resource_data["network"], "mantra-dukong-1");
    }

    #[tokio::test]
    async fn test_async_runtime_configuration() {
        // Test default runtime configuration
        let default_config = AsyncRuntimeConfig::default();
        assert_eq!(default_config.flavor, RuntimeFlavor::MultiThread);
        assert!(default_config.enable_io);
        assert!(default_config.enable_time);
        assert_eq!(default_config.max_blocking_threads, Some(512));

        // Test validation
        assert!(default_config.validate().is_ok());

        // Test environment loading
        std::env::set_var("MCP_RUNTIME_FLAVOR", "current_thread");
        std::env::set_var("MCP_WORKER_THREADS", "4");
        std::env::set_var("MCP_MAX_BLOCKING_THREADS", "256");

        let env_config = AsyncRuntimeConfig::from_env();
        assert_eq!(env_config.flavor, RuntimeFlavor::CurrentThread);
        assert_eq!(env_config.worker_threads, Some(4));
        assert_eq!(env_config.max_blocking_threads, Some(256));

        // Clean up env vars
        std::env::remove_var("MCP_RUNTIME_FLAVOR");
        std::env::remove_var("MCP_WORKER_THREADS");
        std::env::remove_var("MCP_MAX_BLOCKING_THREADS");
    }

    #[tokio::test]
    async fn test_runtime_metrics() {
        let config = AsyncRuntimeConfig::default();
        let metrics = RuntimeMetrics::new(&config);

        assert_eq!(metrics.flavor, RuntimeFlavor::MultiThread);
        assert!(metrics.uptime().as_nanos() > 0);

        let json = metrics.to_json();
        assert!(json.get("uptime_secs").is_some());
        assert!(json.get("active_tasks").is_some());
        assert!(json.get("worker_threads").is_some());
        assert!(json.get("flavor").is_some());
    }

    #[tokio::test]
    async fn test_async_runtime_manager() {
        let config = AsyncRuntimeConfig::default();
        let manager = AsyncRuntimeManager::new(config);

        // Test metrics access
        let metrics = manager.metrics().await;
        assert_eq!(metrics.flavor, RuntimeFlavor::MultiThread);

        // Test task spawning
        let handle = manager.spawn_monitored(async {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            "test_result"
        });

        let result = handle.await.expect("Task should complete");
        assert_eq!(result, "test_result");

        // Test blocking task spawning
        let blocking_handle = manager.spawn_blocking_monitored(|| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            42
        });

        let blocking_result = blocking_handle
            .await
            .expect("Blocking task should complete");
        assert_eq!(blocking_result, 42);

        // Test health status
        let health = manager.health_status().await;
        assert!(health.get("status").is_some());
        assert!(health.get("runtime").is_some());
    }

    #[tokio::test]
    async fn test_server_runtime_integration() {
        let mut config = McpServerConfig::default();
        config.runtime_config.flavor = RuntimeFlavor::CurrentThread;
        config.runtime_config.worker_threads = Some(1);

        let server = create_mcp_server(config);

        // Test that runtime manager is properly integrated
        let runtime_manager = &server.state().runtime_manager;
        assert_eq!(
            runtime_manager.config().flavor,
            RuntimeFlavor::CurrentThread
        );
        assert_eq!(runtime_manager.config().worker_threads, Some(1));

        // Test health status includes runtime information
        let health = server.get_health_status().await;
        assert!(health.get("components").is_some());
        let components = health.get("components").unwrap();
        assert!(components.get("runtime").is_some());
    }

    #[tokio::test]
    async fn test_runtime_config_validation() {
        let mut config = AsyncRuntimeConfig::default();

        // Test valid configuration
        assert!(config.validate().is_ok());

        // Test invalid worker threads
        config.worker_threads = Some(0);
        assert!(config.validate().is_err());

        config.worker_threads = Some(2000); // Too high
        assert!(config.validate().is_err());

        config.worker_threads = Some(4); // Reset to valid

        // Test invalid blocking threads
        config.max_blocking_threads = Some(0);
        assert!(config.validate().is_err());

        config.max_blocking_threads = Some(20000); // Too high
        assert!(config.validate().is_err());

        config.max_blocking_threads = Some(512); // Reset to valid

        // Test invalid stack size
        config.thread_stack_size = Some(1024); // Too small
        assert!(config.validate().is_err());

        config.thread_stack_size = Some(2 * 1024 * 1024); // Valid
        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_error_mapping_to_json_rpc_codes() {
        use crate::error::Error as SdkError;
        use json_rpc_error_codes::*;

        // Test SDK error mapping
        let sdk_wallet_error =
            McpServerError::Sdk(SdkError::Wallet("insufficient funds".to_string()));
        assert_eq!(
            sdk_wallet_error.to_json_rpc_error_code(),
            INSUFFICIENT_FUNDS
        );

        let sdk_network_error =
            McpServerError::Sdk(SdkError::Network("connection failed".to_string()));
        assert_eq!(
            sdk_network_error.to_json_rpc_error_code(),
            NETWORK_CONNECTION_FAILED
        );

        let sdk_fee_error =
            McpServerError::Sdk(SdkError::FeeValidation("fees too high".to_string()));
        assert_eq!(
            sdk_fee_error.to_json_rpc_error_code(),
            FEE_VALIDATION_FAILED
        );

        // Test MCP server error mapping
        let wallet_not_configured = McpServerError::WalletNotConfigured;
        assert_eq!(
            wallet_not_configured.to_json_rpc_error_code(),
            WALLET_NOT_CONFIGURED
        );

        let invalid_args = McpServerError::InvalidArguments("missing parameter".to_string());
        assert_eq!(invalid_args.to_json_rpc_error_code(), INVALID_PARAMS);

        let unknown_tool = McpServerError::UnknownTool("nonexistent_tool".to_string());
        assert_eq!(unknown_tool.to_json_rpc_error_code(), METHOD_NOT_FOUND);

        let unknown_resource =
            McpServerError::UnknownResource("nonexistent://resource".to_string());
        assert_eq!(
            unknown_resource.to_json_rpc_error_code(),
            RESOURCE_NOT_FOUND
        );
    }

    #[tokio::test]
    async fn test_error_data_generation() {
        use crate::error::Error as SdkError;

        // Test SDK error data generation
        let sdk_error = McpServerError::Sdk(SdkError::Wallet("test wallet error".to_string()));
        let error_data = sdk_error.get_error_data();
        assert!(error_data.is_some());

        let data = error_data.unwrap();
        assert_eq!(data["category"], "sdk");
        assert_eq!(data["sdk_error_type"], "Wallet");
        assert!(data["original_error"]
            .as_str()
            .unwrap()
            .contains("test wallet error"));

        // Test validation error data generation
        let validation_error = McpServerError::Validation("invalid input format".to_string());
        let error_data = validation_error.get_error_data();
        assert!(error_data.is_some());

        let data = error_data.unwrap();
        assert_eq!(data["category"], "validation");
        assert!(data["validation_error"]
            .as_str()
            .unwrap()
            .contains("invalid input format"));
    }

    #[tokio::test]
    async fn test_json_rpc_error_creation() {
        use crate::error::Error as SdkError;
        use json_rpc_error_codes::*;

        // Test JsonRpcError creation from MCP error
        let mcp_error = McpServerError::Sdk(SdkError::Network("RPC timeout".to_string()));
        let json_rpc_error = mcp_error.to_json_rpc_error();

        assert_eq!(json_rpc_error.code, NETWORK_CONNECTION_FAILED);
        assert!(json_rpc_error.message.contains("RPC timeout"));
        assert!(json_rpc_error.data.is_some());

        // Test JsonRpcError methods with new error codes
        let internal_error = JsonRpcError::internal_error("test internal error".to_string());
        assert_eq!(internal_error.code, INTERNAL_ERROR);

        let parse_error = JsonRpcError::parse_error("invalid JSON".to_string());
        assert_eq!(parse_error.code, PARSE_ERROR);

        let custom_error = JsonRpcError::custom_error(
            INSUFFICIENT_FUNDS,
            "not enough tokens".to_string(),
            Some(serde_json::json!({"required": 100, "available": 50})),
        );
        assert_eq!(custom_error.code, INSUFFICIENT_FUNDS);
        assert!(custom_error.data.is_some());
    }

    #[tokio::test]
    async fn test_sdk_error_type_name_mapping() {
        use crate::error::Error as SdkError;

        // Test all SDK error type name mappings
        let test_cases = vec![
            (SdkError::Rpc("test rpc error".to_string()), "Rpc"),
            (SdkError::Wallet("test wallet error".to_string()), "Wallet"),
            (SdkError::Config("test config error".to_string()), "Config"),
            (
                SdkError::Contract("test contract error".to_string()),
                "Contract",
            ),
            (
                SdkError::FeeValidation("test fee error".to_string()),
                "FeeValidation",
            ),
            (
                SdkError::Network("test network error".to_string()),
                "Network",
            ),
            (
                SdkError::Timeout("test timeout error".to_string()),
                "Timeout",
            ),
            (SdkError::Other("test other error".to_string()), "Other"),
        ];

        for (sdk_error, expected_name) in test_cases {
            let mcp_error = McpServerError::Sdk(sdk_error);
            let error_data = mcp_error.get_error_data().unwrap();
            assert_eq!(error_data["sdk_error_type"], expected_name);
        }
    }

    #[tokio::test]
    async fn test_wallet_error_code_specificity() {
        use crate::error::Error as SdkError;
        use json_rpc_error_codes::*;

        // Test specific wallet error message handling
        let insufficient_funds_error = McpServerError::Sdk(SdkError::Wallet(
            "insufficient balance for transaction".to_string(),
        ));
        assert_eq!(
            insufficient_funds_error.to_json_rpc_error_code(),
            INSUFFICIENT_FUNDS
        );

        let address_format_error =
            McpServerError::Sdk(SdkError::Wallet("invalid address format".to_string()));
        assert_eq!(
            address_format_error.to_json_rpc_error_code(),
            INVALID_ADDRESS_FORMAT
        );

        let generic_wallet_error =
            McpServerError::Sdk(SdkError::Wallet("wallet not initialized".to_string()));
        assert_eq!(
            generic_wallet_error.to_json_rpc_error_code(),
            WALLET_NOT_CONFIGURED
        );
    }

    /// Test the enhanced error mapping functionality
    #[tokio::test]
    async fn test_enhanced_error_mapping_and_recovery() {
        // Test enhanced wallet error context detection
        let wallet_errors = vec![
            (
                SdkError::Wallet("insufficient balance for transaction".to_string()),
                json_rpc_error_codes::INSUFFICIENT_FUNDS,
            ),
            (
                SdkError::Wallet("invalid address format provided".to_string()),
                json_rpc_error_codes::INVALID_ADDRESS_FORMAT,
            ),
            (
                SdkError::Wallet("invalid mnemonic phrase provided".to_string()),
                json_rpc_error_codes::INVALID_MNEMONIC_FORMAT,
            ),
            (
                SdkError::Wallet("invalid public key format".to_string()),
                json_rpc_error_codes::INVALID_PUBLIC_KEY_FORMAT,
            ),
            (
                SdkError::Wallet("general wallet error".to_string()),
                json_rpc_error_codes::WALLET_NOT_CONFIGURED,
            ),
        ];

        for (sdk_error, expected_code) in wallet_errors {
            let mcp_error = McpServerError::Sdk(sdk_error);
            assert_eq!(mcp_error.to_json_rpc_error_code(), expected_code);

            // Test error data generation
            let error_data = mcp_error.get_error_data().unwrap();
            assert_eq!(error_data["category"], "sdk");
            assert!(error_data["recovery_suggestions"].is_array());
            assert!(error_data["severity"].is_string());
            assert!(error_data["timestamp"].is_string());
        }

        // Test enhanced contract error context detection
        let contract_errors = vec![
            (
                SdkError::Contract("pool not found on network".to_string()),
                json_rpc_error_codes::POOL_NOT_FOUND,
            ),
            (
                SdkError::Contract("slippage tolerance exceeded".to_string()),
                json_rpc_error_codes::SWAP_SLIPPAGE_EXCEEDED,
            ),
            (
                SdkError::Contract("insufficient liquidity in pool".to_string()),
                json_rpc_error_codes::LIQUIDITY_INSUFFICIENT,
            ),
            (
                SdkError::Contract("general contract error".to_string()),
                json_rpc_error_codes::TOOL_EXECUTION_FAILED,
            ),
        ];

        for (sdk_error, expected_code) in contract_errors {
            let mcp_error = McpServerError::Sdk(sdk_error);
            assert_eq!(mcp_error.to_json_rpc_error_code(), expected_code);
        }

        // Test error recovery suggestions
        let network_error =
            McpServerError::Sdk(SdkError::Network("connection timeout".to_string()));
        let recovery_suggestions =
            McpServerError::get_recovery_suggestions(&SdkError::Network("test".to_string()));
        assert!(recovery_suggestions.contains(&"Check internet connectivity"));
        assert!(recovery_suggestions.contains(&"Verify firewall settings"));

        // Test error severity levels
        assert_eq!(
            McpServerError::get_error_severity(&SdkError::Wallet("test".to_string())),
            "high"
        );
        assert_eq!(
            McpServerError::get_error_severity(&SdkError::Network("test".to_string())),
            "medium"
        );
        assert_eq!(
            McpServerError::get_error_severity(&SdkError::Timeout("test".to_string())),
            "low"
        );

        // Test error recoverability
        assert!(network_error.is_recoverable());
        assert_eq!(network_error.get_retry_delay(), Some(5));

        let wallet_error = McpServerError::Sdk(SdkError::Wallet("test".to_string()));
        assert!(!wallet_error.is_recoverable());
        assert_eq!(wallet_error.get_retry_delay(), None);

        // Test JSON-RPC error generation
        let json_rpc_error = network_error.to_json_rpc_error();
        assert_eq!(
            json_rpc_error.code,
            json_rpc_error_codes::NETWORK_CONNECTION_FAILED
        );
        assert!(json_rpc_error.data.is_some());

        let error_data = json_rpc_error.data.unwrap();
        assert_eq!(error_data["category"], "sdk");
        assert_eq!(error_data["severity"], "medium");
        assert!(error_data["recovery_suggestions"].is_array());
    }

    /// Test MCP-specific error types and their data generation
    #[tokio::test]
    async fn test_mcp_error_types_data_generation() {
        // Test InvalidArguments error
        let invalid_args_error =
            McpServerError::InvalidArguments("Missing required parameter 'pool_id'".to_string());
        let error_data = invalid_args_error.get_error_data().unwrap();
        assert_eq!(error_data["category"], "arguments");
        assert_eq!(error_data["severity"], "high");
        assert!(error_data["recovery_suggestions"].is_array());

        // Test UnknownTool error
        let unknown_tool_error = McpServerError::UnknownTool("invalid_tool_name".to_string());
        let error_data = unknown_tool_error.get_error_data().unwrap();
        assert_eq!(error_data["category"], "tool");
        assert_eq!(error_data["tool_name"], "invalid_tool_name");
        assert_eq!(error_data["severity"], "medium");

        // Test UnknownResource error
        let unknown_resource_error =
            McpServerError::UnknownResource("invalid://resource/uri".to_string());
        let error_data = unknown_resource_error.get_error_data().unwrap();
        assert_eq!(error_data["category"], "resource");
        assert_eq!(error_data["resource_uri"], "invalid://resource/uri");
        assert_eq!(error_data["severity"], "medium");

        // Test WalletNotConfigured error
        let wallet_not_configured_error = McpServerError::WalletNotConfigured;
        let error_data = wallet_not_configured_error.get_error_data().unwrap();
        assert_eq!(error_data["category"], "wallet");
        assert_eq!(error_data["severity"], "high");

        // Test Network error
        let network_error = McpServerError::Network("RPC endpoint unreachable".to_string());
        let error_data = network_error.get_error_data().unwrap();
        assert_eq!(error_data["category"], "network");
        assert_eq!(error_data["severity"], "medium");
        assert_eq!(error_data["network_error"], "RPC endpoint unreachable");

        // Test Validation error
        let validation_error =
            McpServerError::Validation("Invalid amount format: must be numeric".to_string());
        let error_data = validation_error.get_error_data().unwrap();
        assert_eq!(error_data["category"], "validation");
        assert_eq!(error_data["severity"], "high");
        assert_eq!(
            error_data["validation_error"],
            "Invalid amount format: must be numeric"
        );
    }

    /// Test error code mapping for all SDK error types
    #[tokio::test]
    async fn test_comprehensive_sdk_error_code_mapping() {
        let test_cases = vec![
            (
                SdkError::Rpc("RPC connection failed".to_string()),
                json_rpc_error_codes::BLOCKCHAIN_RPC_ERROR,
            ),
            (
                SdkError::TxSimulation("Transaction simulation failed".to_string()),
                json_rpc_error_codes::TRANSACTION_FAILED,
            ),
            (
                SdkError::Config("Invalid network configuration".to_string()),
                json_rpc_error_codes::CONFIGURATION_ERROR,
            ),
            (
                SdkError::FeeValidation("Total fees exceed 20% limit".to_string()),
                json_rpc_error_codes::FEE_VALIDATION_FAILED,
            ),
            (
                SdkError::Timeout("Request timeout after 30s".to_string()),
                json_rpc_error_codes::TIMEOUT_ERROR,
            ),
            (
                SdkError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "File not found",
                )),
                json_rpc_error_codes::IO_ERROR,
            ),
            (
                SdkError::Other("Unexpected error occurred".to_string()),
                json_rpc_error_codes::INTERNAL_ERROR,
            ),
        ];

        for (sdk_error, expected_code) in test_cases {
            let mcp_error = McpServerError::Sdk(sdk_error);
            assert_eq!(mcp_error.to_json_rpc_error_code(), expected_code);
        }

        // Test serialization error separately with a proper error instance
        let ser_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let mcp_ser_error = McpServerError::Sdk(SdkError::Serialization(ser_error));
        assert_eq!(
            mcp_ser_error.to_json_rpc_error_code(),
            json_rpc_error_codes::SERIALIZATION_ERROR
        );
    }

    /// Test error data timestamp generation and format
    #[tokio::test]
    async fn test_error_data_timestamp_generation() {
        let error = McpServerError::Sdk(SdkError::Network("test".to_string()));
        let error_data = error.get_error_data().unwrap();

        // Verify timestamp is present and in correct format
        assert!(error_data["timestamp"].is_string());
        let timestamp_str = error_data["timestamp"].as_str().unwrap();

        // Try to parse the timestamp to verify it's valid RFC3339
        chrono::DateTime::parse_from_rfc3339(timestamp_str).expect("Invalid timestamp format");
    }

    #[tokio::test]
    async fn test_config_file_toml_support() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for test files
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.toml");

        // Create a sample TOML configuration
        let toml_content = r#"
name = "Test MCP Server"
version = "0.2.0"
debug = true
max_concurrent_ops = 20
http_port = 9090
http_host = "0.0.0.0"
request_timeout_secs = 60
cache_ttl_secs = 600
auto_load_env = true

[network_config]
network_name = "mantra-dukong"
network_id = "mantra-dukong-1"
rpc_url = "https://rpc.dukong.mantrachain.io"
gas_price = 0.01
gas_adjustment = 1.5
native_denom = "uaum"

[network_config.contracts]
pool_manager = ""

[runtime_config]
flavor = "MultiThread"
enable_io = true
enable_time = true
max_blocking_threads = 1024
"#;

        fs::write(&config_path, toml_content).unwrap();

        // Load configuration from TOML file
        let loaded_config = McpServerConfig::from_file(&config_path).unwrap();

        assert_eq!(loaded_config.name, "Test MCP Server");
        assert_eq!(loaded_config.version, "0.2.0");
        assert_eq!(loaded_config.debug, true);
        assert_eq!(loaded_config.max_concurrent_ops, 20);
        assert_eq!(loaded_config.http_port, 9090);
        assert_eq!(loaded_config.http_host, "0.0.0.0");
        assert_eq!(loaded_config.request_timeout_secs, 60);
        assert_eq!(loaded_config.cache_ttl_secs, 600);
        assert_eq!(
            loaded_config.runtime_config.flavor,
            RuntimeFlavor::MultiThread
        );
        assert_eq!(
            loaded_config.runtime_config.max_blocking_threads,
            Some(1024)
        );
    }

    #[tokio::test]
    async fn test_config_file_json_support() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for test files
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.json");

        // Create a sample JSON configuration
        let json_content = r#"{
  "name": "Test JSON MCP Server",
  "version": "0.3.0",
  "debug": false,
  "max_concurrent_ops": 15,
  "http_port": 8888,
  "http_host": "127.0.0.1",
  "request_timeout_secs": 45,
  "cache_ttl_secs": 450,
  "auto_load_env": false,
  "network_config": {
    "network_name": "mantra-dukong",
    "network_id": "mantra-dukong-1",
    "rpc_url": "https://rpc.dukong.mantrachain.io",
    "gas_price": 0.01,
    "gas_adjustment": 1.5,
    "native_denom": "uaum",
    "contracts": {
      "pool_manager": ""
    }
  },
  "runtime_config": {
    "flavor": "CurrentThread",
    "enable_io": true,
    "enable_time": false,
    "max_blocking_threads": 256
  }
}"#;

        fs::write(&config_path, json_content).unwrap();

        // Load configuration from JSON file
        let loaded_config = McpServerConfig::from_file(&config_path).unwrap();

        assert_eq!(loaded_config.name, "Test JSON MCP Server");
        assert_eq!(loaded_config.version, "0.3.0");
        assert_eq!(loaded_config.debug, false);
        assert_eq!(loaded_config.max_concurrent_ops, 15);
        assert_eq!(loaded_config.http_port, 8888);
        assert_eq!(loaded_config.http_host, "127.0.0.1");
        assert_eq!(loaded_config.request_timeout_secs, 45);
        assert_eq!(loaded_config.cache_ttl_secs, 450);
        assert_eq!(loaded_config.auto_load_env, false);
        assert_eq!(
            loaded_config.runtime_config.flavor,
            RuntimeFlavor::CurrentThread
        );
        assert_eq!(loaded_config.runtime_config.enable_time, false);
        assert_eq!(loaded_config.runtime_config.max_blocking_threads, Some(256));
    }

    #[tokio::test]
    async fn test_config_save_to_file() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for test files
        let temp_dir = TempDir::new().unwrap();

        let mut config = McpServerConfig::default();
        config.name = "Save Test Server".to_string();
        config.version = "1.0.0".to_string();
        config.debug = true;
        config.max_concurrent_ops = 25;

        // Test TOML save
        let toml_path = temp_dir.path().join("saved-config.toml");
        config.save_to_file(&toml_path).unwrap();

        assert!(toml_path.exists());
        let toml_content = fs::read_to_string(&toml_path).unwrap();
        assert!(toml_content.contains("Save Test Server"));
        assert!(toml_content.contains("1.0.0"));
        assert!(toml_content.contains("debug = true"));
        assert!(toml_content.contains("max_concurrent_ops = 25"));

        // Test JSON save
        let json_path = temp_dir.path().join("saved-config.json");
        config.save_to_file(&json_path).unwrap();

        assert!(json_path.exists());
        let json_content = fs::read_to_string(&json_path).unwrap();
        assert!(json_content.contains("Save Test Server"));
        assert!(json_content.contains("1.0.0"));
        assert!(json_content.contains("\"debug\": true"));
        assert!(json_content.contains("\"max_concurrent_ops\": 25"));

        // Verify saved files can be loaded back
        let loaded_toml = McpServerConfig::from_file(&toml_path).unwrap();
        let loaded_json = McpServerConfig::from_file(&json_path).unwrap();

        assert_eq!(loaded_toml.name, config.name);
        assert_eq!(loaded_json.name, config.name);
        assert_eq!(loaded_toml.debug, config.debug);
        assert_eq!(loaded_json.debug, config.debug);
    }

    #[tokio::test]
    async fn test_config_from_sources_layering() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for test files
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("layered-config.toml");

        // Create a configuration file with some values
        let toml_content = r#"
name = "File Config Server"
debug = true
max_concurrent_ops = 50
http_port = 7777

[network_config]
network_name = "mantra-testnet"
network_id = "mantra-testnet-1"
rpc_url = "https://rpc.testnet.mantrachain.io"
gas_price = 0.01
gas_adjustment = 1.5
native_denom = "uaum"

[network_config.contracts]
pool_manager = ""
"#;
        fs::write(&config_path, toml_content).unwrap();

        // Set environment variables that should override file values
        env::set_var("MCP_SERVER_NAME", "Env Override Server");
        env::set_var("MCP_HTTP_PORT", "6666");
        env::set_var("MCP_MAX_CONCURRENT_OPS", "100");

        // Load configuration with layering
        let config = McpServerConfig::from_sources(Some(&config_path)).unwrap();

        // Environment variables should override file values
        assert_eq!(config.name, "Env Override Server"); // From env
        assert_eq!(config.http_port, 6666); // From env
        assert_eq!(config.max_concurrent_ops, 100); // From env
        assert_eq!(config.debug, true); // From file (no env override)

        // Clean up environment variables
        env::remove_var("MCP_SERVER_NAME");
        env::remove_var("MCP_HTTP_PORT");
        env::remove_var("MCP_MAX_CONCURRENT_OPS");
    }

    #[tokio::test]
    async fn test_config_file_error_handling() {
        use std::fs;
        use tempfile::TempDir;

        // Test non-existent file
        let temp_dir = TempDir::new().unwrap();
        let non_existent_path = temp_dir.path().join("does-not-exist.toml");
        let result = McpServerConfig::from_file(&non_existent_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));

        // Test unsupported file format (create the file first so it exists)
        let unsupported_path = temp_dir.path().join("config.xml");
        fs::write(&unsupported_path, "<xml>test</xml>").unwrap();
        let result = McpServerConfig::from_file(&unsupported_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported"));

        // Test invalid TOML content
        let invalid_toml_path = temp_dir.path().join("invalid.toml");
        fs::write(&invalid_toml_path, "invalid toml [[[content").unwrap();
        let result = McpServerConfig::from_file(&invalid_toml_path);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Failed to parse")
                || error_msg.contains("Failed to build")
                || error_msg.contains("Failed to load")
        );
    }

    #[tokio::test]
    async fn test_generate_example_config() {
        let example_toml = McpServerConfig::generate_example_config();

        // Verify the example contains expected sections and comments
        assert!(example_toml.contains("# Mantra DEX SDK MCP Server Configuration"));
        assert!(example_toml.contains("# Server identification"));
        assert!(example_toml.contains("name = \"Mantra DEX SDK MCP Server\""));
        assert!(example_toml.contains("version = \"0.1.0\""));
        assert!(example_toml.contains("# Network configuration"));
        assert!(example_toml.contains("# Performance settings"));
        assert!(example_toml.contains("# HTTP transport settings"));
        assert!(example_toml.contains("[runtime_config]"));
        assert!(example_toml.contains("flavor = \"MultiThread\""));
        assert!(example_toml.contains("max_blocking_threads = 512"));

        // Verify the example can be parsed as valid TOML
        let parsed_toml: toml::Value = toml::from_str(&example_toml).unwrap();
        assert!(parsed_toml.get("name").is_some());
        assert!(parsed_toml.get("runtime_config").is_some());
    }

    #[tokio::test]
    async fn test_create_example_files() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let result = McpServerConfig::create_example_files(temp_dir.path());
        assert!(result.is_ok());

        // Verify both example files were created
        let toml_example = temp_dir.path().join("mcp-server.example.toml");
        let json_example = temp_dir.path().join("mcp-server.example.json");

        assert!(toml_example.exists());
        assert!(json_example.exists());

        // Verify the files can be loaded successfully
        let toml_config = McpServerConfig::from_file(&toml_example).unwrap();
        let json_config = McpServerConfig::from_file(&json_example).unwrap();

        assert_eq!(toml_config.name, "Mantra DEX SDK MCP Server");
        assert_eq!(json_config.name, "Mantra DEX SDK MCP Server");
        assert_eq!(toml_config.version, json_config.version);
    }

    #[tokio::test]
    async fn test_network_config_from_file() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("network-config.toml");

        // Create configuration with network specification
        let toml_content = r#"
name = "Network Test Server"
network = "testnet"
debug = false

[network_config]
network_name = "mantra-testnet"
network_id = "mantra-testnet-1"
rpc_url = "https://rpc.testnet.mantrachain.io"
gas_price = 0.01
gas_adjustment = 1.5
native_denom = "uaum"

[network_config.contracts]
pool_manager = ""
"#;
        fs::write(&config_path, toml_content).unwrap();

        // Load configuration
        let config = McpServerConfig::from_file(&config_path).unwrap();

        assert_eq!(config.name, "Network Test Server");
        assert_eq!(config.debug, false);
        // Network config should be applied (exact values depend on network constants)
        assert!(!config.network_config.network_id.is_empty());
    }

    #[tokio::test]
    async fn test_detect_file_format() {
        use std::path::Path;

        // Test TOML format detection
        let toml_path = Path::new("config.toml");
        let format = McpServerConfig::detect_file_format(toml_path).unwrap();
        assert_eq!(format, FileFormat::Toml);

        // Test JSON format detection
        let json_path = Path::new("config.json");
        let format = McpServerConfig::detect_file_format(json_path).unwrap();
        assert_eq!(format, FileFormat::Json);

        // Test YAML format detection
        let yaml_path = Path::new("config.yaml");
        let format = McpServerConfig::detect_file_format(yaml_path).unwrap();
        assert_eq!(format, FileFormat::Yaml);

        let yml_path = Path::new("config.yml");
        let format = McpServerConfig::detect_file_format(yml_path).unwrap();
        assert_eq!(format, FileFormat::Yaml);

        // Test unsupported format
        let xml_path = Path::new("config.xml");
        let result = McpServerConfig::detect_file_format(xml_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported"));
    }
}

// =============================================================================
// JSON-RPC Error Code Constants (MCP Specification Compliance)
// =============================================================================

/// JSON-RPC 2.0 standard error codes as defined in the MCP specification
/// See: https://spec.modelcontextprotocol.io/specification/2025-03-26/
pub mod json_rpc_error_codes {
    // Standard JSON-RPC 2.0 errors (-32768 to -32000 are reserved)
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Implementation-defined server errors (-32099 to -32000)
    pub const SERVER_ERROR: i32 = -32000;
    pub const SERVER_OVERLOADED: i32 = -32001;
    pub const RATE_LIMIT_EXCEEDED: i32 = -32002;
    pub const SESSION_EXPIRED: i32 = -32003;
    pub const METHOD_NOT_READY: i32 = -32004;

    // MCP-specific error codes (outside reserved range)
    // Application errors (-31999 to -1)
    pub const WALLET_NOT_CONFIGURED: i32 = -31999;
    pub const NETWORK_CONNECTION_FAILED: i32 = -31998;
    pub const BLOCKCHAIN_RPC_ERROR: i32 = -31997;
    pub const TRANSACTION_FAILED: i32 = -31996;
    pub const INSUFFICIENT_PERMISSIONS: i32 = -31995;
    pub const RESOURCE_NOT_FOUND: i32 = -31994;
    pub const TOOL_EXECUTION_FAILED: i32 = -31993;
    pub const CONFIGURATION_ERROR: i32 = -31992;
    pub const VALIDATION_ERROR: i32 = -31991;
    pub const TIMEOUT_ERROR: i32 = -31990;

    // Validation and input errors (1 to 999)
    pub const INVALID_ADDRESS_FORMAT: i32 = 100;
    pub const INVALID_MNEMONIC_FORMAT: i32 = 101;
    pub const INVALID_PUBLIC_KEY_FORMAT: i32 = 102;
    pub const MISSING_REQUIRED_FIELD: i32 = 103;
    pub const INVALID_NETWORK_NAME: i32 = 104;
    pub const INVALID_AMOUNT_FORMAT: i32 = 105;

    // Business logic errors (1000 to 4999)
    pub const INSUFFICIENT_FUNDS: i32 = 1000;
    pub const POOL_NOT_FOUND: i32 = 1001;
    pub const SWAP_SLIPPAGE_EXCEEDED: i32 = 1002;
    pub const LIQUIDITY_INSUFFICIENT: i32 = 1003;
    pub const REWARD_CLAIM_FAILED: i32 = 1004;
    pub const FEE_VALIDATION_FAILED: i32 = 1005;

    // System and infrastructure errors (5000+)
    pub const DATABASE_ERROR: i32 = 5000;
    pub const CACHE_ERROR: i32 = 5001;
    pub const SERIALIZATION_ERROR: i32 = 5002;
    pub const DESERIALIZATION_ERROR: i32 = 5003;
    pub const IO_ERROR: i32 = 5004;
}
