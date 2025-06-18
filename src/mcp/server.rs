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
const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
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
    info!("Created MCP server with STDIO transport");
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
