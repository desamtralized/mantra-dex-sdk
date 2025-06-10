//! Application State Management
//!
//! This module manages the global application state for the TUI, including
//! screen navigation, data caching, and state transitions.

#[cfg(feature = "tui")]
use crate::tui::events::Event;
#[cfg(feature = "tui")]
use crate::{Error, MantraDexClient, MantraNetworkConfig};
#[cfg(feature = "tui")]
use cosmrs::proto::cosmos::base::abci::v1beta1::TxResponse;
#[cfg(feature = "tui")]
use cosmwasm_std::{Coin, Uint128};
#[cfg(feature = "tui")]
use mantra_dex_std::pool_manager::{PoolInfoResponse, SimulationResponse};
#[cfg(feature = "tui")]
use std::collections::HashMap;

/// Available screens in the TUI application
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Dashboard,
    Pools,
    Swap,
    MultiHop,
    Liquidity,
    Rewards,
    Admin,
    Settings,
    TransactionDetails,
}

impl Screen {
    /// Get the display name for the screen
    pub fn display_name(&self) -> &'static str {
        match self {
            Screen::Dashboard => "Dashboard",
            Screen::Pools => "Pools",
            Screen::Swap => "Swap",
            Screen::MultiHop => "Multi-hop",
            Screen::Liquidity => "Liquidity",
            Screen::Rewards => "Rewards",
            Screen::Admin => "Admin",
            Screen::Settings => "Settings",
            Screen::TransactionDetails => "Transaction",
        }
    }

    /// Get all available screens for navigation
    pub fn all() -> Vec<Screen> {
        vec![
            Screen::Dashboard,
            Screen::Pools,
            Screen::Swap,
            Screen::MultiHop,
            Screen::Liquidity,
            Screen::Rewards,
            Screen::Admin,
            Screen::Settings,
        ]
    }
}

/// Loading state for async operations
#[derive(Debug, Clone)]
pub enum LoadingState {
    Idle,
    Loading(String), // Loading with description
    Success(String), // Success with message
    Error(String),   // Error with message
}

/// Transaction details for tracking
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub hash: String,
    pub status: TransactionStatus,
    pub operation_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub gas_used: Option<i64>,
    pub gas_wanted: Option<i64>,
}

/// Transaction status enum
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Success,
    Failed,
    Unknown,
}

/// Pool cache entry for efficient lookup
#[derive(Debug, Clone)]
pub struct PoolCacheEntry {
    pub pool_info: PoolInfoResponse,
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

/// Swap operation state for the swap screen
#[derive(Debug, Clone)]
pub struct SwapState {
    pub from_asset: Option<String>,
    pub to_asset: Option<String>,
    pub amount: String,
    pub slippage: String,
    pub simulation_result: Option<SimulationResponse>,
    pub selected_pool_id: Option<String>,
}

/// Current liquidity operation state
#[derive(Debug, Clone)]
pub struct LiquidityState {
    pub operation_mode: LiquidityOperationMode,
    pub selected_pool_id: Option<String>,
    pub first_asset_amount: String,
    pub second_asset_amount: String,
    pub withdraw_amount: String,
    pub slippage_amount: String,
    pub slippage_swap: String,
    pub expected_lp_tokens: Option<Uint128>,
    pub expected_assets: Option<(Uint128, Uint128, String, String)>,
}

/// Liquidity operation modes
#[derive(Debug, Clone, PartialEq)]
pub enum LiquidityOperationMode {
    Provide,
    Withdraw,
    ViewPositions,
}

impl Default for SwapState {
    fn default() -> Self {
        Self {
            from_asset: None,
            to_asset: None,
            amount: String::new(),
            slippage: "1.0".to_string(), // Default 1% slippage
            simulation_result: None,
            selected_pool_id: None,
        }
    }
}

impl Default for LiquidityState {
    fn default() -> Self {
        Self {
            operation_mode: LiquidityOperationMode::Provide,
            selected_pool_id: None,
            first_asset_amount: String::new(),
            second_asset_amount: String::new(),
            withdraw_amount: String::new(),
            slippage_amount: "1.0".to_string(),
            slippage_swap: "1.0".to_string(),
            expected_lp_tokens: None,
            expected_assets: None,
        }
    }
}

/// Application state structure
#[derive(Debug)]
pub struct AppState {
    /// Current active screen
    pub current_screen: Screen,
    /// Loading state for async operations
    pub loading_state: LoadingState,
    /// Error messages to display
    pub error_message: Option<String>,
    /// Status message to display
    pub status_message: Option<String>,
    /// Selected pool ID (if any)
    pub selected_pool_id: Option<u64>,
    /// User token balances cache
    pub balances: HashMap<String, String>,
    /// Recent transaction details with full info
    pub recent_transactions: Vec<TransactionInfo>,
    /// Network block height
    pub block_height: Option<u64>,
    /// Connected wallet address
    pub wallet_address: Option<String>,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Current tab selection for navigation
    pub current_tab: usize,
    /// Cached pool information
    pub pool_cache: HashMap<String, PoolCacheEntry>,
    /// Current swap operation state
    pub swap_state: SwapState,
    /// Current liquidity operation state
    pub liquidity_state: LiquidityState,
    /// Current epoch information
    pub current_epoch: Option<u64>,
    /// Claimable rewards amount
    pub claimable_rewards: HashMap<String, Uint128>,
    /// Network information
    pub network_info: NetworkInfo,
}

/// Network information for display
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub chain_id: Option<String>,
    pub node_version: Option<String>,
    pub is_syncing: bool,
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for NetworkInfo {
    fn default() -> Self {
        Self {
            chain_id: None,
            node_version: None,
            is_syncing: false,
            last_sync_time: None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_screen: Screen::Dashboard,
            loading_state: LoadingState::Idle,
            error_message: None,
            status_message: None,
            selected_pool_id: None,
            balances: HashMap::new(),
            recent_transactions: Vec::new(),
            block_height: None,
            wallet_address: None,
            should_quit: false,
            current_tab: 0,
            pool_cache: HashMap::new(),
            swap_state: SwapState::default(),
            liquidity_state: LiquidityState::default(),
            current_epoch: None,
            claimable_rewards: HashMap::new(),
            network_info: NetworkInfo::default(),
        }
    }
}

/// Main application structure
pub struct App {
    /// Application state
    pub state: AppState,
    /// DEX client
    pub client: MantraDexClient,
    /// Configuration
    pub config: MantraNetworkConfig,
}

impl App {
    /// Create a new application instance
    pub fn new(client: MantraDexClient, config: MantraNetworkConfig) -> Self {
        Self {
            state: AppState::default(),
            client,
            config,
        }
    }

    /// Set an error message
    pub fn set_error(&mut self, message: String) {
        self.state.error_message = Some(message.clone());
        self.state.loading_state = LoadingState::Error(message);
    }

    /// Set a status message
    pub fn set_status(&mut self, message: String) {
        self.state.status_message = Some(message);
    }

    /// Clear error and status messages
    pub fn clear_messages(&mut self) {
        self.state.error_message = None;
        self.state.status_message = None;
        if matches!(
            self.state.loading_state,
            LoadingState::Error(_) | LoadingState::Success(_)
        ) {
            self.state.loading_state = LoadingState::Idle;
        }
    }

    /// Set loading state
    pub fn set_loading(&mut self, message: String) {
        self.state.loading_state = LoadingState::Loading(message);
    }

    /// Set success state
    pub fn set_success(&mut self, message: String) {
        self.state.loading_state = LoadingState::Success(message.clone());
        self.state.status_message = Some(message);
    }

    /// Navigate to a specific screen
    pub fn navigate_to(&mut self, screen: Screen) {
        self.state.current_screen = screen;
        self.clear_messages();
    }

    /// Navigate to the next tab
    pub fn next_tab(&mut self) {
        let screens = Screen::all();
        self.state.current_tab = (self.state.current_tab + 1) % screens.len();
        self.state.current_screen = screens[self.state.current_tab];
        self.clear_messages();
    }

    /// Navigate to the previous tab
    pub fn previous_tab(&mut self) {
        let screens = Screen::all();
        if self.state.current_tab == 0 {
            self.state.current_tab = screens.len() - 1;
        } else {
            self.state.current_tab -= 1;
        }
        self.state.current_screen = screens[self.state.current_tab];
        self.clear_messages();
    }

    /// Handle application events
    pub async fn handle_event(&mut self, event: Event) -> Result<bool, Error> {
        match event {
            Event::Quit => {
                self.state.should_quit = true;
                return Ok(true);
            }
            Event::Tab => {
                self.next_tab();
            }
            Event::BackTab => {
                self.previous_tab();
            }
            Event::Enter => {
                // Handle enter key based on current screen
                match self.state.current_screen {
                    Screen::Dashboard => {
                        // Refresh dashboard data
                        self.refresh_dashboard_data().await?;
                    }
                    Screen::Pools => {
                        // Refresh pool data
                        self.refresh_pool_data().await?;
                    }
                    Screen::Swap => {
                        // Execute swap simulation or actual swap
                        self.handle_swap_action().await?;
                    }
                    Screen::Liquidity => {
                        // Handle liquidity operations
                        self.handle_liquidity_action().await?;
                    }
                    _ => {
                        // Screen-specific enter handling will be implemented in screen modules
                    }
                }
            }
            Event::Escape => {
                // Clear any error states or go back to dashboard
                self.clear_messages();
                if self.state.current_screen != Screen::Dashboard {
                    self.navigate_to(Screen::Dashboard);
                }
            }
            Event::Refresh => {
                self.refresh_current_screen_data().await?;
            }
            _ => {
                // Other events will be handled by specific screens
            }
        }

        Ok(self.state.should_quit)
    }

    /// Refresh data for the current screen
    async fn refresh_current_screen_data(&mut self) -> Result<(), Error> {
        match self.state.current_screen {
            Screen::Dashboard => self.refresh_dashboard_data().await,
            Screen::Pools => self.refresh_pool_data().await,
            Screen::Swap => self.refresh_swap_data().await,
            Screen::Liquidity => self.refresh_liquidity_data().await,
            Screen::Rewards => self.refresh_rewards_data().await,
            _ => Ok(()),
        }
    }

    /// Refresh dashboard data
    async fn refresh_dashboard_data(&mut self) -> Result<(), Error> {
        self.set_loading("Refreshing dashboard data...".to_string());

        // Update block height
        if let Ok(height) = self.client.get_last_block_height().await {
            self.state.block_height = Some(height);
        }

        // Update balances
        if let Ok(balances) = self.client.get_balances().await {
            for balance in balances {
                self.state
                    .balances
                    .insert(balance.denom, balance.amount.to_string());
            }
        }

        // Update current epoch
        if let Ok(epoch) = self.client.get_current_epoch().await {
            self.state.current_epoch = Some(epoch);
        }

        self.set_success("Dashboard data refreshed".to_string());
        Ok(())
    }

    /// Refresh pool data
    async fn refresh_pool_data(&mut self) -> Result<(), Error> {
        self.set_loading("Refreshing pool data...".to_string());

        match self.client.get_pools(Some(20)).await {
            Ok(pools) => {
                let now = chrono::Utc::now();
                for pool in pools {
                    let pool_id = pool.pool_info.pool_identifier.clone();
                    self.state.pool_cache.insert(
                        pool_id,
                        PoolCacheEntry {
                            pool_info: pool,
                            cached_at: now,
                        },
                    );
                }
                self.set_success("Pool data refreshed".to_string());
            }
            Err(e) => {
                self.set_error(format!("Failed to refresh pool data: {}", e));
            }
        }

        Ok(())
    }

    /// Refresh swap-related data
    async fn refresh_swap_data(&mut self) -> Result<(), Error> {
        self.set_loading("Refreshing swap data...".to_string());

        // Refresh simulation if we have swap parameters
        if let (Some(pool_id), Some(from_asset)) = (
            &self.state.swap_state.selected_pool_id,
            &self.state.swap_state.from_asset,
        ) {
            if let Ok(amount) = self.state.swap_state.amount.parse::<u128>() {
                let offer_asset = Coin {
                    denom: from_asset.clone(),
                    amount: Uint128::new(amount),
                };

                if let Some(to_asset) = &self.state.swap_state.to_asset {
                    match self
                        .client
                        .simulate_swap(pool_id, offer_asset, to_asset)
                        .await
                    {
                        Ok(simulation) => {
                            self.state.swap_state.simulation_result = Some(simulation);
                            self.set_success("Swap simulation updated".to_string());
                        }
                        Err(e) => {
                            self.set_error(format!("Simulation failed: {}", e));
                        }
                    }
                }
            }
        } else {
            self.set_success("Swap data current".to_string());
        }

        Ok(())
    }

    /// Refresh liquidity data
    async fn refresh_liquidity_data(&mut self) -> Result<(), Error> {
        self.set_loading("Refreshing liquidity data...".to_string());

        // Refresh pool data first since liquidity operations depend on it
        self.refresh_pool_data().await?;

        // Refresh positions if wallet is connected
        if let Some(_address) = &self.state.wallet_address {
            // TODO: Query user's liquidity positions
            // This would involve querying the user's LP token balances
            // and calculating position values
            self.set_success("Liquidity data refreshed".to_string());
        } else {
            self.set_success("Liquidity data refreshed (no wallet connected)".to_string());
        }

        Ok(())
    }

    /// Refresh rewards data
    async fn refresh_rewards_data(&mut self) -> Result<(), Error> {
        self.set_loading("Refreshing rewards data...".to_string());

        if let Some(address) = &self.state.wallet_address {
            match self.client.query_all_rewards(address).await {
                Ok(_rewards_data) => {
                    // Parse rewards data and update claimable_rewards
                    // This would depend on the specific format returned by the query
                    self.set_success("Rewards data refreshed".to_string());
                }
                Err(e) => {
                    self.set_error(format!("Failed to refresh rewards: {}", e));
                }
            }
        } else {
            self.set_error("No wallet address available for rewards query".to_string());
        }

        Ok(())
    }

    /// Handle swap action (simulation or execution)
    async fn handle_swap_action(&mut self) -> Result<(), Error> {
        // This method would be expanded to handle swap execution
        // For now, just trigger simulation refresh
        self.refresh_swap_data().await
    }

    /// Handle liquidity action (provide/withdraw liquidity)
    async fn handle_liquidity_action(&mut self) -> Result<(), Error> {
        // This method would be expanded to handle liquidity operations
        // For now, just trigger liquidity data refresh
        self.refresh_liquidity_data().await
    }

    /// Update wallet address
    pub fn set_wallet_address(&mut self, address: String) {
        self.state.wallet_address = Some(address);
    }

    /// Update block height
    pub fn set_block_height(&mut self, height: u64) {
        self.state.block_height = Some(height);
        self.state.network_info.last_sync_time = Some(chrono::Utc::now());
    }

    /// Update token balance
    pub fn update_balance(&mut self, token: String, balance: String) {
        self.state.balances.insert(token, balance);
    }

    /// Add a recent transaction with full details
    pub fn add_transaction(&mut self, tx_info: TransactionInfo) {
        self.state.recent_transactions.insert(0, tx_info);
        // Keep only the last 50 transactions
        if self.state.recent_transactions.len() > 50 {
            self.state.recent_transactions.truncate(50);
        }
    }

    /// Add a recent transaction (legacy method for backward compatibility)
    pub fn add_recent_transaction(&mut self, tx_hash: String) {
        let tx_info = TransactionInfo {
            hash: tx_hash,
            status: TransactionStatus::Unknown,
            operation_type: "Unknown".to_string(),
            timestamp: chrono::Utc::now(),
            gas_used: None,
            gas_wanted: None,
        };
        self.add_transaction(tx_info);
    }

    /// Select a pool
    pub fn select_pool(&mut self, pool_id: u64) {
        self.state.selected_pool_id = Some(pool_id);
    }

    /// Clear pool selection
    pub fn clear_pool_selection(&mut self) {
        self.state.selected_pool_id = None;
    }

    /// Update swap state
    pub fn update_swap_state(&mut self, swap_state: SwapState) {
        self.state.swap_state = swap_state;
    }

    /// Get cached pool information
    pub fn get_cached_pool(&self, pool_id: &str) -> Option<&PoolInfoResponse> {
        self.state
            .pool_cache
            .get(pool_id)
            .map(|entry| &entry.pool_info)
    }

    /// Check if pool cache is stale (older than 5 minutes)
    pub fn is_pool_cache_stale(&self, pool_id: &str) -> bool {
        match self.state.pool_cache.get(pool_id) {
            Some(entry) => {
                let age = chrono::Utc::now() - entry.cached_at;
                age.num_minutes() > 5
            }
            None => true,
        }
    }

    /// Update network information
    pub fn update_network_info(&mut self, chain_id: Option<String>, is_syncing: bool) {
        self.state.network_info.chain_id = chain_id;
        self.state.network_info.is_syncing = is_syncing;
        self.state.network_info.last_sync_time = Some(chrono::Utc::now());
    }

    /// Process transaction response and update state
    pub async fn process_transaction_response(
        &mut self,
        tx_response: TxResponse,
        operation_type: String,
    ) {
        let status = if tx_response.code == 0 {
            TransactionStatus::Success
        } else {
            TransactionStatus::Failed
        };

        let tx_info = TransactionInfo {
            hash: tx_response.txhash.clone(),
            status: status.clone(),
            operation_type: operation_type.clone(),
            timestamp: chrono::Utc::now(),
            gas_used: Some(tx_response.gas_used),
            gas_wanted: Some(tx_response.gas_wanted),
        };

        self.add_transaction(tx_info);

        match status {
            TransactionStatus::Success => {
                self.set_success(format!(
                    "{} successful: {}",
                    operation_type, tx_response.txhash
                ));
            }
            TransactionStatus::Failed => {
                self.set_error(format!(
                    "{} failed: {}",
                    operation_type, tx_response.raw_log
                ));
            }
            _ => {}
        }

        // Trigger data refresh after successful operations
        if status == TransactionStatus::Success {
            let _ = self.refresh_dashboard_data().await;
        }
    }
}
