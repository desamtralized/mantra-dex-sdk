pub mod client;
pub mod config;
pub mod error;
pub mod wallet;

// TUI module - optional via "tui" feature
#[cfg(feature = "tui")]
pub mod tui;

// MCP module - optional via "mcp" feature
#[cfg(feature = "mcp")]
pub mod mcp;

// Re-export mantra-dex-std for user convenience
pub use mantra_dex_std;

pub use client::MantraDexClient;
pub use config::{MantraNetworkConfig, NetworkConstants};
pub use error::Error;
pub use wallet::MantraWallet;

// Re-export TUI entry point when feature is enabled
#[cfg(feature = "tui")]
pub use tui::run_tui;

// Re-export MCP server types when feature is enabled
#[cfg(feature = "mcp")]
pub use mcp::{
    create_http_server, create_mcp_server, create_stdio_server, ConnectionPoolConfig,
    MantraDexMcpServer, McpResult, McpSdkAdapter, McpServerConfig, McpServerError, MCP_SERVER_NAME,
    MCP_SERVER_VERSION,
};

// Re-export common types from mantra-dex-std
pub use cosmwasm_std::{Coin, Decimal, Uint128};
pub use mantra_dex_std::{
    fee::PoolFee,
    pool_manager::{FeatureToggle, PoolInfo, PoolType, SwapOperation},
};
