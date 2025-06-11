//! Application State Management
//!
//! This module manages the global application state for the TUI, including
//! screen navigation, data caching, and state transitions.

#[cfg(feature = "tui")]
use crate::tui::components::modals::{ErrorType, ModalState};
#[cfg(feature = "tui")]
use crate::tui::events::Event;
#[cfg(feature = "tui")]
use crate::tui::utils::async_ops::SyncConfig;
#[cfg(feature = "tui")]
use crate::tui::utils::focus_manager::FocusManager;
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
#[cfg(feature = "tui")]
use std::sync::Arc;
#[cfg(feature = "tui")]
use std::time::Duration;

#[cfg(feature = "tui")]
use tokio::sync::mpsc;

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

/// Navigation mode for keyboard handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationMode {
    /// Navigating between main screen tabs
    ScreenLevel,
    /// Navigating within the current screen
    WithinScreen,
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

/// Enhanced loading state for comprehensive async operations
#[derive(Debug, Clone)]
pub enum LoadingState {
    Idle,
    Loading {
        message: String,
        progress: Option<f64>, // 0.0 to 100.0, None for indeterminate
        can_cancel: bool,
        operation_id: Option<String>, // For cancellation tracking
    },
    Success {
        message: String,
        details: Option<Vec<String>>,
    },
    Error {
        message: String,
        error_type: ErrorType,
        details: Option<Vec<String>>,
        retry_action: Option<String>,
    },
}

impl LoadingState {
    /// Create a simple loading state
    pub fn loading(message: String) -> Self {
        Self::Loading {
            message,
            progress: None,
            can_cancel: false,
            operation_id: None,
        }
    }

    /// Create a loading state with progress
    pub fn loading_with_progress(message: String, progress: f64, can_cancel: bool) -> Self {
        Self::Loading {
            message,
            progress: Some(progress),
            can_cancel,
            operation_id: None,
        }
    }

    /// Create a loading state with operation ID for cancellation
    pub fn loading_with_id(message: String, operation_id: String, can_cancel: bool) -> Self {
        Self::Loading {
            message,
            progress: None,
            can_cancel,
            operation_id: Some(operation_id),
        }
    }

    /// Create a success state
    pub fn success(message: String) -> Self {
        Self::Success {
            message,
            details: None,
        }
    }

    /// Create a success state with details
    pub fn success_with_details(message: String, details: Vec<String>) -> Self {
        Self::Success {
            message,
            details: Some(details),
        }
    }

    /// Create a comprehensive error state
    pub fn error(message: String, error_type: ErrorType) -> Self {
        Self::Error {
            message,
            error_type,
            details: None,
            retry_action: None,
        }
    }

    /// Create an error state with retry action
    pub fn error_with_retry(message: String, error_type: ErrorType, retry_action: String) -> Self {
        Self::Error {
            message,
            error_type,
            details: None,
            retry_action: Some(retry_action),
        }
    }

    /// Create an error state with detailed information
    pub fn error_with_details(
        message: String,
        error_type: ErrorType,
        details: Vec<String>,
        retry_action: Option<String>,
    ) -> Self {
        Self::Error {
            message,
            error_type,
            details: Some(details),
            retry_action,
        }
    }

    /// Check if the state represents an active loading operation
    pub fn is_loading(&self) -> bool {
        matches!(self, LoadingState::Loading { .. })
    }

    /// Check if the state represents an error
    pub fn is_error(&self) -> bool {
        matches!(self, LoadingState::Error { .. })
    }

    /// Check if the loading operation can be cancelled
    pub fn can_cancel(&self) -> bool {
        match self {
            LoadingState::Loading { can_cancel, .. } => *can_cancel,
            _ => false,
        }
    }

    /// Get the current progress if available
    pub fn progress(&self) -> Option<f64> {
        match self {
            LoadingState::Loading { progress, .. } => *progress,
            _ => None,
        }
    }

    /// Get the operation ID if available
    pub fn operation_id(&self) -> Option<&String> {
        match self {
            LoadingState::Loading { operation_id, .. } => operation_id.as_ref(),
            _ => None,
        }
    }
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

/// Global application state
pub struct AppState {
    /// Current active screen
    pub current_screen: Screen,
    /// Navigation mode (screen-level vs within-screen)
    pub navigation_mode: NavigationMode,
    /// Enhanced loading state for async operations
    pub loading_state: LoadingState,
    /// Error messages to display (deprecated in favor of modal_state)
    pub error_message: Option<String>,
    /// Status message to display
    pub status_message: Option<String>,
    /// Modal state for comprehensive dialogs
    pub modal_state: Option<ModalState>,
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
    /// Rewards screen state
    pub rewards_state: crate::tui::screens::rewards::RewardsState,
    /// Admin screen state
    pub admin_state: crate::tui::screens::admin::AdminState,
    /// Settings screen state
    pub settings_state: crate::tui::screens::settings::SettingsState,
    /// Transaction screen state
    pub transaction_state: crate::tui::screens::transaction::TransactionState,
    /// Network information
    pub network_info: NetworkInfo,
    /// Pending async operations for tracking
    pub pending_operations: HashMap<String, PendingOperation>,
    /// Focus management for keyboard navigation
    pub focus_manager: FocusManager,
    /// Wallet setup wizard state
    pub wizard_state: crate::tui::screens::wizard::WizardState,
}

/// Pending operation tracking for comprehensive loading states
#[derive(Debug, Clone)]
pub struct PendingOperation {
    pub operation_type: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub can_cancel: bool,
    pub cancel_token: Option<String>,
}

/// Enhanced network information with detailed connection state
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub chain_id: Option<String>,
    pub node_version: Option<String>,
    pub is_syncing: bool,
    pub last_sync_time: Option<chrono::DateTime<chrono::Utc>>,
    pub connection_state: crate::tui::utils::async_ops::NetworkState,
    pub last_block_height: Option<u64>,
    pub connection_latency: Option<Duration>,
    pub retry_count: u32,
}

impl Default for NetworkInfo {
    fn default() -> Self {
        Self {
            chain_id: None,
            node_version: None,
            is_syncing: false,
            last_sync_time: None,
            connection_state: crate::tui::utils::async_ops::NetworkState::Connected,
            last_block_height: None,
            connection_latency: None,
            retry_count: 0,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_screen: Screen::Dashboard,
            navigation_mode: NavigationMode::ScreenLevel,
            loading_state: LoadingState::Idle,
            error_message: None,
            status_message: None,
            modal_state: None,
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
            rewards_state: crate::tui::screens::rewards::RewardsState::default(),
            admin_state: crate::tui::screens::admin::AdminState::default(),
            settings_state: crate::tui::screens::settings::SettingsState::default(),
            transaction_state: crate::tui::screens::transaction::TransactionState::default(),
            network_info: NetworkInfo::default(),
            pending_operations: HashMap::new(),
            focus_manager: FocusManager::new(),
            wizard_state: {
                let mut wizard = crate::tui::screens::wizard::WizardState::new();
                // Show wizard on first run if no wallet is configured
                wizard.show_wizard = true; // Always show for now; in real app, check wallet config
                wizard
            },
        }
    }
}

/// Main application structure
pub struct App {
    /// Application state
    pub state: AppState,
    /// DEX client wrapped in Arc for sharing
    pub client: Arc<MantraDexClient>,
    /// Configuration
    pub config: MantraNetworkConfig,
    /// Event sender for background task communication
    event_sender: Option<mpsc::UnboundedSender<Event>>,
    /// Enhanced background task coordinator
    background_coordinator: Option<crate::tui::utils::async_ops::BackgroundTaskCoordinator>,
}

impl App {
    /// Create a new application instance
    pub fn new(client: MantraDexClient, config: MantraNetworkConfig) -> Self {
        Self {
            state: AppState::default(),
            client: Arc::new(client),
            config,
            event_sender: None,
            background_coordinator: None,
        }
    }

    /// Initialize background tasks for data synchronization with enhanced coordination
    pub fn initialize_background_tasks(&mut self, event_sender: mpsc::UnboundedSender<Event>) {
        // Create enhanced background task coordinator
        let client_arc = Arc::clone(&self.client);
        let mut coordinator = crate::tui::utils::async_ops::BackgroundTaskCoordinator::new(
            event_sender.clone(),
            client_arc,
            None, // Use default config for now
        );

        // Set wallet address if available
        if let Some(wallet_address) = &self.state.wallet_address {
            coordinator.set_wallet_address(wallet_address.clone());
        }

        // Start background coordination
        coordinator.start();

        self.background_coordinator = Some(coordinator);
        self.event_sender = Some(event_sender);
    }

    /// Stop background tasks with proper cleanup
    pub fn stop_background_tasks(&mut self) {
        if let Some(mut coordinator) = self.background_coordinator.take() {
            coordinator.stop();
        }
    }

    /// Update sync configuration
    pub fn update_sync_config(&mut self, config: crate::tui::utils::async_ops::SyncConfig) {
        if let Some(coordinator) = &mut self.background_coordinator {
            coordinator.update_config(config);
        }
    }

    /// Check if real-time synchronization is active
    pub fn is_real_time_sync_active(&self) -> bool {
        self.background_coordinator
            .as_ref()
            .map(|c| c.is_active())
            .unwrap_or(false)
    }

    /// Get current network state
    pub async fn get_network_state(&self) -> crate::tui::utils::async_ops::NetworkState {
        if let Some(coordinator) = &self.background_coordinator {
            coordinator.get_network_state().await
        } else {
            crate::tui::utils::async_ops::NetworkState::Connected
        }
    }

    /// Execute async operation with comprehensive error handling
    pub async fn execute_async_operation<F, Fut, T>(
        &mut self,
        operation_name: &str,
        operation: F,
    ) -> Result<T, Error>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        // Set loading state
        self.set_loading_with_progress(format!("Executing {}...", operation_name), Some(0.1), true);

        let start_time = std::time::Instant::now();

        // Check network state first
        let network_state = self.get_network_state().await;
        match network_state {
            crate::tui::utils::async_ops::NetworkState::Disconnected => {
                let error_msg =
                    "Network is disconnected. Please check your connection.".to_string();
                self.set_error_with_retry(
                    error_msg.clone(),
                    ErrorType::Network,
                    "retry_connection".to_string(),
                );
                return Err(Error::Network(error_msg));
            }
            crate::tui::utils::async_ops::NetworkState::Error(ref err) => {
                let error_msg = format!("Network error: {}", err);
                self.set_error_with_retry(
                    error_msg.clone(),
                    ErrorType::Network,
                    "retry_connection".to_string(),
                );
                return Err(Error::Network(error_msg));
            }
            _ => {}
        }

        // Update progress
        self.update_loading_progress(25.0, Some(format!("Executing {}...", operation_name)));

        // Execute the operation with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30), // 30 second timeout
            operation(),
        )
        .await;

        match result {
            Ok(Ok(value)) => {
                let duration = start_time.elapsed();
                self.set_success(format!(
                    "{} completed successfully in {:.2}s",
                    operation_name,
                    duration.as_secs_f64()
                ));
                Ok(value)
            }
            Ok(Err(e)) => {
                let categorized_error = self.categorize_error(&e);
                let error_msg = format!("{} failed: {}", operation_name, e);

                match categorized_error {
                    ErrorType::Network => {
                        self.set_error_with_retry(
                            error_msg,
                            categorized_error,
                            "retry_operation".to_string(),
                        );
                    }
                    ErrorType::Validation => {
                        self.set_error_with_type(error_msg, categorized_error);
                    }
                    _ => {
                        self.set_error_with_retry(
                            error_msg,
                            categorized_error,
                            "retry_operation".to_string(),
                        );
                    }
                }
                Err(e)
            }
            Err(_) => {
                let error_msg = format!("{} timed out after 30 seconds", operation_name);
                self.set_error_with_retry(
                    error_msg.clone(),
                    ErrorType::Timeout,
                    "retry_operation".to_string(),
                );
                Err(Error::Rpc(error_msg))
            }
        }
    }

    /// Enhanced error handling with comprehensive categorization
    fn categorize_error(&self, error: &Error) -> ErrorType {
        let error_string = error.to_string().to_lowercase();

        if error_string.contains("connection")
            || error_string.contains("network")
            || error_string.contains("timeout")
        {
            ErrorType::Network
        } else if error_string.contains("unauthorized") || error_string.contains("forbidden") {
            ErrorType::Authentication
        } else if error_string.contains("validation") || error_string.contains("invalid") {
            ErrorType::Validation
        } else if error_string.contains("insufficient") || error_string.contains("balance") {
            ErrorType::InsufficientFunds
        } else if error_string.contains("contract") || error_string.contains("execute") {
            ErrorType::Contract
        } else {
            ErrorType::Unknown
        }
    }

    /// Handle async blockchain operations with comprehensive status updates
    pub async fn handle_event(&mut self, event: Event) -> Result<bool, Error> {
        // Handle network state changes
        if let Event::Custom(ref custom_event) = event {
            if custom_event.starts_with("network_state_changed:") {
                let parts: Vec<&str> = custom_event.split(':').collect();
                if parts.len() >= 3 {
                    let new_state = parts[2];
                    match new_state {
                        "connected" => {
                            self.state.network_info.connection_state =
                                crate::tui::utils::async_ops::NetworkState::Connected;
                            self.set_status("Network connection restored".to_string());
                        }
                        "disconnected" => {
                            self.state.network_info.connection_state =
                                crate::tui::utils::async_ops::NetworkState::Disconnected;
                            self.set_error_with_type(
                                "Network disconnected. Some features may be unavailable."
                                    .to_string(),
                                ErrorType::Network,
                            );
                        }
                        "error" => {
                            self.state.network_info.connection_state =
                                crate::tui::utils::async_ops::NetworkState::Error(
                                    "Network error".to_string(),
                                );
                            self.set_error_with_retry(
                                "Network error detected. Attempting to reconnect...".to_string(),
                                ErrorType::Network,
                                "retry_connection".to_string(),
                            );
                        }
                        _ => {}
                    }
                }
                return Ok(false);
            }
        }

        // Handle blockchain progress events
        if let Event::BlockchainProgress {
            operation,
            status,
            progress,
        } = &event
        {
            self.update_loading_progress(
                progress.unwrap_or(0.0) as f64 * 100.0,
                Some(format!("{}: {}", operation, status)),
            );
            return Ok(false);
        }

        // Handle blockchain success events
        if let Event::BlockchainSuccess {
            operation,
            result,
            transaction_hash,
        } = &event
        {
            let mut success_details = vec![result.clone()];
            if let Some(tx_hash) = transaction_hash {
                success_details.push(format!("Transaction: {}", tx_hash));
            }

            self.state.loading_state = LoadingState::success_with_details(
                format!("{} completed successfully", operation),
                success_details,
            );
            return Ok(false);
        }

        // Handle blockchain error events
        if let Event::BlockchainError { operation, error } = &event {
            let error_type = if error.to_lowercase().contains("network") {
                ErrorType::Network
            } else if error.to_lowercase().contains("contract") {
                ErrorType::Contract
            } else {
                ErrorType::Unknown
            };

            self.state.loading_state = LoadingState::error_with_details(
                format!("{} failed", operation),
                error_type,
                vec![error.clone()],
                Some("retry_operation".to_string()),
            );
            return Ok(false);
        }

        // Handle data refresh events with enhanced error reporting
        if let Event::DataRefresh {
            data_type,
            success,
            error,
        } = &event
        {
            match self
                .handle_data_refresh(data_type.clone(), *success, error.clone())
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    self.set_error_with_type(
                        format!("Failed to refresh {}: {}", data_type, e),
                        ErrorType::Unknown,
                    );
                }
            }
            return Ok(false);
        }

        // Handle blockchain action events with comprehensive async processing
        match &event {
            Event::ExecuteSwap {
                from_asset,
                to_asset,
                amount,
                pool_id,
                slippage_tolerance,
            } => {
                let operation_name = "swap";
                let from_asset = from_asset.clone();
                let to_asset = to_asset.clone();
                let amount = amount.clone();
                let pool_id = *pool_id;
                let slippage_tolerance = slippage_tolerance.clone();

                // Execute swap with comprehensive error handling
                let result = self
                    .execute_async_operation(operation_name, || async {
                        // TODO: Implement actual swap execution
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        Ok(())
                    })
                    .await;

                if let Err(e) = result {
                    eprintln!("Swap failed: {}", e);
                }
                return Ok(false);
            }
            Event::ProvideLiquidity {
                pool_id,
                asset_1_amount,
                asset_2_amount,
                slippage_tolerance,
            } => {
                let operation_name = "provide_liquidity";
                let pool_id = *pool_id;
                let asset_1_amount = asset_1_amount.clone();
                let asset_2_amount = asset_2_amount.clone();
                let slippage_tolerance = slippage_tolerance.clone();

                let result = self
                    .execute_async_operation(operation_name, || async {
                        // TODO: Implement actual liquidity provision
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        Ok(())
                    })
                    .await;

                if let Err(e) = result {
                    eprintln!("Liquidity provision failed: {}", e);
                }
                return Ok(false);
            }
            Event::ClaimRewards {
                pool_id,
                epochs,
                claim_all,
            } => {
                let operation_name = "claim_rewards";
                let pool_id = *pool_id;
                let epochs = epochs.clone();
                let claim_all = *claim_all;

                let result = self
                    .execute_async_operation(operation_name, || async {
                        // TODO: Implement actual rewards claiming
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        Ok(())
                    })
                    .await;

                if let Err(e) = result {
                    eprintln!("Rewards claiming failed: {}", e);
                }
                return Ok(false);
            }
            _ => {}
        }

        // Handle focus management events
        if let Some(focused_component) = self.state.focus_manager.handle_event(&event) {
            // Focus changed, you can add logic here to update component states
            self.update_component_focus(&focused_component);
        }

        // Handle wizard events first if wizard is active
        if self.state.wizard_state.show_wizard {
            return self.handle_wizard_event(event).await;
        }

        // Handle standard navigation events
        match event {
            Event::Quit => {
                self.state.should_quit = true;
                return Ok(true);
            }
            Event::Tab => {
                match self.state.navigation_mode {
                    NavigationMode::ScreenLevel => {
                        // Navigate to next screen tab
                        self.next_tab();
                    }
                    NavigationMode::WithinScreen => {
                        // Handle within-screen tab navigation
                        if let Some(focused_component) =
                            self.state.focus_manager.handle_event(&Event::FocusNext)
                        {
                            self.update_component_focus(&focused_component);
                        }
                    }
                }
            }
            Event::BackTab => {
                match self.state.navigation_mode {
                    NavigationMode::ScreenLevel => {
                        // Navigate to previous screen tab
                        self.previous_tab();
                    }
                    NavigationMode::WithinScreen => {
                        // Handle within-screen reverse tab navigation
                        if let Some(focused_component) =
                            self.state.focus_manager.handle_event(&Event::FocusPrevious)
                        {
                            self.update_component_focus(&focused_component);
                        }
                    }
                }
            }
            Event::Escape => {
                // Handle escape key - close modals or return to screen-level navigation
                if self.state.modal_state.is_some() {
                    self.state.modal_state = None;
                } else if self.state.navigation_mode == NavigationMode::WithinScreen {
                    // Exit within-screen navigation mode
                    self.state.navigation_mode = NavigationMode::ScreenLevel;
                    self.state.focus_manager.clear_focus();
                } else {
                    self.state.focus_manager.return_to_previous();
                }
            }
            Event::Help => {
                self.show_help();
            }
            Event::Refresh => {
                self.refresh_current_screen_data().await?;
            }
            Event::Enter => {
                // Handle enter key based on navigation mode
                if let Some(modal) = &self.state.modal_state {
                    if modal.is_confirmed() {
                        self.handle_confirmation();
                        self.state.modal_state = None;
                    }
                } else if self.state.navigation_mode == NavigationMode::ScreenLevel {
                    // Enter within-screen navigation mode
                    self.state.navigation_mode = NavigationMode::WithinScreen;
                    self.initialize_focus_for_screen(self.state.current_screen);
                } else {
                    // Handle enter within screen
                    self.handle_enter_key().await?;
                }
            }
            // Handle focus management events only when in within-screen mode
            Event::MoveFocus(direction) => {
                if let Some(crate::tui::events::FocusableComponent::Dropdown(id)) = self.state.focus_manager.current_focus() {
                    let swap_state = crate::tui::screens::swap::get_swap_screen_state();

                    match (id.as_str(), direction) {
                        ("swap_pool", crate::tui::events::FocusDirection::Up) => swap_state.pool_dropdown.move_up(),
                        ("swap_pool", crate::tui::events::FocusDirection::Down) => swap_state.pool_dropdown.move_down(),
                        ("swap_to_asset", crate::tui::events::FocusDirection::Up) => swap_state.to_token_dropdown.move_up(),
                        ("swap_to_asset", crate::tui::events::FocusDirection::Down) => swap_state.to_token_dropdown.move_down(),
                        _ => {}
                    }

                    // Prevent focus manager from switching components
                    return Ok(());
                }
            }
            Event::ContextAction => {
                // Handle space bar for context-sensitive actions
                self.handle_context_action().await?;
            }
            Event::F(1) => {
                self.show_help();
            }
            Event::F(5) => {
                self.refresh_current_screen_data().await?;
            }
            _ => {
                // Handle modal events if modal is open
                if self.state.modal_state.is_some() {
                    if self.handle_modal_event(&event) {
                        return Ok(false);
                    }
                }

                // Handle number key navigation for tab switching (1-8) when in ScreenLevel mode
                if let Event::Char(c) = &event {
                    if self.state.navigation_mode == NavigationMode::ScreenLevel {
                        if let Some(screen) =
                            crate::tui::components::navigation::number_key_to_screen(*c)
                        {
                            self.navigate_to(screen);
                            return Ok(false);
                        }
                    }
                }

                // Handle screen-specific events
                self.handle_screen_specific_event(event).await?;
            }
        }

        Ok(false)
    }

    /// Update component focus state when focus changes
    fn update_component_focus(
        &mut self,
        focused_component: &crate::tui::events::FocusableComponent,
    ) {
        use crate::tui::events::FocusableComponent;

        // Bridge FocusManager focus changes to per-screen component state so that
        // individual screen modules can visually highlight the focused widget.
        match self.state.current_screen {
            Screen::Swap => {
                let swap_state = crate::tui::screens::swap::get_swap_screen_state();
                // Clear previous internal focus first
                swap_state.reset_focus();

                match focused_component {
                    FocusableComponent::TextInput(id) => {
                        match id.as_str() {
                            "swap_amount" => swap_state.input_focus = crate::tui::screens::swap::SwapInputFocus::FromAmount,
                            "swap_slippage" => swap_state.input_focus = crate::tui::screens::swap::SwapInputFocus::Slippage,
                            _ => {}
                        }
                    }
                    FocusableComponent::Dropdown(id) => {
                        match id.as_str() {
                            "swap_pool" => swap_state.input_focus = crate::tui::screens::swap::SwapInputFocus::Pool,
                            "swap_to_asset" => swap_state.input_focus = crate::tui::screens::swap::SwapInputFocus::ToToken,
                            _ => {}
                        }
                    }
                    FocusableComponent::Button(id) => {
                        if id == "swap_execute" {
                            swap_state.input_focus = crate::tui::screens::swap::SwapInputFocus::Execute;
                        }
                    }
                    _ => {}
                }

                // Ensure internal state knows which widget is focused so that render_* helpers style correctly
                swap_state.apply_focus();
            }
            _ => {}
        }
    }

    /// Initialize focus manager for the current screen
    pub fn initialize_focus_for_screen(&mut self, screen: Screen) {
        use crate::tui::utils::focus_manager::component_ids::*;

        let components = match screen {
            Screen::Dashboard => vec![dashboard_refresh_button(), dashboard_transactions_table()],
            Screen::Pools => vec![pools_search_input(), pools_table()],
            Screen::Swap => vec![
                swap_from_asset_input(),
                swap_pool_dropdown(),
                swap_to_asset_dropdown(),
                swap_amount_input(),
                swap_slippage_input(),
                swap_execute_button(),
            ],
            Screen::Liquidity => vec![
                liquidity_pool_dropdown(),
                liquidity_amount1_input(),
                liquidity_amount2_input(),
                liquidity_provide_button(),
                liquidity_withdraw_button(),
            ],
            Screen::Rewards => vec![
                rewards_epoch_input(),
                rewards_claim_all_button(),
                rewards_history_table(),
            ],
            Screen::Admin => vec![
                admin_asset1_input(),
                admin_asset2_input(),
                admin_fee_input(),
                admin_create_pool_button(),
            ],
            Screen::Settings => vec![
                settings_network_dropdown(),
                settings_rpc_input(),
                settings_wallet_input(),
                settings_save_button(),
                settings_reset_button(),
            ],
            _ => vec![],
        };

        self.state.focus_manager.set_tab_order(components);
        // Set focus to first component
        self.state.focus_manager.focus_first();
    }

    /// Handle refresh for current screen
    async fn refresh_current_screen_data(&mut self) -> Result<(), Error> {
        match self.state.current_screen {
            Screen::Dashboard => {
                // Refresh dashboard data
                self.refresh_dashboard_data().await?;
            }
            Screen::Pools => {
                // Refresh pool data
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(Event::DataRefresh {
                        data_type: "pools".to_string(),
                        success: true,
                        error: None,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle enter key based on current focus
    async fn handle_enter_key(&mut self) -> Result<(), Error> {
        let focused = self.state.focus_manager.current_focus().cloned();
        if let Some(focused) = focused {
            match focused {
                crate::tui::events::FocusableComponent::Button(button_id) => {
                    self.handle_button_activation(&button_id).await?;
                }
                crate::tui::events::FocusableComponent::Dropdown(dropdown_id) => {
                    // Toggle dropdown
                    self.handle_dropdown_toggle(&dropdown_id);
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle context action (space bar)
    async fn handle_context_action(&mut self) -> Result<(), Error> {
        let focused = self.state.focus_manager.current_focus().cloned();
        if let Some(focused) = focused {
            match focused {
                crate::tui::events::FocusableComponent::Checkbox(checkbox_id) => {
                    self.handle_checkbox_toggle(&checkbox_id);
                }
                crate::tui::events::FocusableComponent::Button(button_id) => {
                    self.handle_button_activation(&button_id).await?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle button activation
    async fn handle_button_activation(&mut self, button_id: &str) -> Result<(), Error> {
        match button_id {
            "swap_execute" => {
                // Execute swap
                let swap_state = self.state.swap_state.clone();
                if let (Some(from_asset), Some(to_asset)) =
                    (&swap_state.from_asset, &swap_state.to_asset)
                {
                    if let Some(sender) = &self.event_sender {
                        let _ = sender.send(Event::ExecuteSwap {
                            from_asset: from_asset.clone(),
                            to_asset: to_asset.clone(),
                            amount: swap_state.amount,
                            pool_id: swap_state.selected_pool_id.and_then(|id| id.parse().ok()),
                            slippage_tolerance: Some(swap_state.slippage),
                        });
                    }
                }
            }
            "liquidity_provide" => {
                // Provide liquidity
                let liquidity_state = self.state.liquidity_state.clone();
                if let Some(pool_id_str) = &liquidity_state.selected_pool_id {
                    if let Ok(pool_id) = pool_id_str.parse::<u64>() {
                        if let Some(sender) = &self.event_sender {
                            let _ = sender.send(Event::ProvideLiquidity {
                                pool_id,
                                asset_1_amount: liquidity_state.first_asset_amount,
                                asset_2_amount: liquidity_state.second_asset_amount,
                                slippage_tolerance: Some(liquidity_state.slippage_amount),
                            });
                        }
                    }
                }
            }
            "rewards_claim_all" => {
                // Claim all rewards
                if let Some(sender) = &self.event_sender {
                    let _ = sender.send(Event::ClaimRewards {
                        pool_id: None,
                        epochs: None,
                        claim_all: true,
                    });
                }
            }
            "dashboard_refresh" => {
                self.refresh_current_screen_data().await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle dropdown toggle
    fn handle_dropdown_toggle(&mut self, _dropdown_id: &str) {
        if self.state.current_screen == Screen::Swap {
            let swap_state = crate::tui::screens::swap::get_swap_screen_state();

            match _dropdown_id {
                "swap_pool" => {
                    swap_state.pool_dropdown.toggle();
                }
                "swap_to_asset" => {
                    swap_state.to_token_dropdown.toggle();
                }
                _ => {}
            }
        }
    }

    /// Handle checkbox toggle
    fn handle_checkbox_toggle(&mut self, _checkbox_id: &str) {
        // Implementation depends on specific checkbox state management
        // This is a placeholder
    }

    /// Handle screen-specific events
    async fn handle_screen_specific_event(&mut self, event: Event) -> Result<(), Error> {
        match self.state.current_screen {
            Screen::Swap => {
                self.handle_swap_screen_event(event).await?;
            }
            Screen::Liquidity => {
                self.handle_liquidity_screen_event(event).await?;
            }
            Screen::Settings => {
                self.handle_settings_screen_event(event).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle swap screen specific events
    async fn handle_swap_screen_event(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::Char(c) => {
                // Handle character input for focused text fields
                let focused = self.state.focus_manager.current_focus().cloned();
                if let Some(focused) = focused {
                    match focused {
                        crate::tui::events::FocusableComponent::TextInput(field_id) => {
                            match field_id.as_str() {
                                "swap_amount" => {
                                    // Update UI component
                                    let swap_state = crate::tui::screens::swap::get_swap_screen_state();
                                    swap_state.handle_input(tui_input::InputRequest::InsertChar(c));
                                    self.state.swap_state.amount = swap_state.from_amount_input.value().to_string();
                                }
                                "swap_slippage" => {
                                    let swap_state = crate::tui::screens::swap::get_swap_screen_state();
                                    swap_state.handle_input(tui_input::InputRequest::InsertChar(c));
                                    self.state.swap_state.slippage = swap_state.slippage_input.value().to_string();
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Backspace => {
                // Handle backspace for focused text fields
                let focused = self.state.focus_manager.current_focus().cloned();
                if let Some(focused) = focused {
                    match focused {
                        crate::tui::events::FocusableComponent::TextInput(field_id) => {
                            match field_id.as_str() {
                                "swap_amount" => {
                                    let swap_state = crate::tui::screens::swap::get_swap_screen_state();
                                    swap_state.handle_input(tui_input::InputRequest::Backspace);
                                    self.state.swap_state.amount = swap_state.from_amount_input.value().to_string();
                                }
                                "swap_slippage" => {
                                    let swap_state = crate::tui::screens::swap::get_swap_screen_state();
                                    swap_state.handle_input(tui_input::InputRequest::Backspace);
                                    self.state.swap_state.slippage = swap_state.slippage_input.value().to_string();
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::MoveFocus(direction) => {
                // When a dropdown is focused, repurpose Up/Down keys to navigate within the list instead of shifting focus.
                if let Some(crate::tui::events::FocusableComponent::Dropdown(id)) = self.state.focus_manager.current_focus() {
                    let swap_state = crate::tui::screens::swap::get_swap_screen_state();

                    match (id.as_str(), direction) {
                        ("swap_pool", crate::tui::events::FocusDirection::Up) => swap_state.pool_dropdown.move_up(),
                        ("swap_pool", crate::tui::events::FocusDirection::Down) => swap_state.pool_dropdown.move_down(),
                        ("swap_to_asset", crate::tui::events::FocusDirection::Up) => swap_state.to_token_dropdown.move_up(),
                        ("swap_to_asset", crate::tui::events::FocusDirection::Down) => swap_state.to_token_dropdown.move_down(),
                        _ => {}
                    }

                    // Prevent focus manager from switching components
                    return Ok(());
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle liquidity screen specific events
    async fn handle_liquidity_screen_event(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::Char(c) => {
                let focused = self.state.focus_manager.current_focus().cloned();
                if let Some(focused) = focused {
                    match focused {
                        crate::tui::events::FocusableComponent::TextInput(field_id) => {
                            match field_id.as_str() {
                                "liquidity_amount1" => {
                                    self.state.liquidity_state.first_asset_amount.push(c);
                                }
                                "liquidity_amount2" => {
                                    self.state.liquidity_state.second_asset_amount.push(c);
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Backspace => {
                let focused = self.state.focus_manager.current_focus().cloned();
                if let Some(focused) = focused {
                    match focused {
                        crate::tui::events::FocusableComponent::TextInput(field_id) => {
                            match field_id.as_str() {
                                "liquidity_amount1" => {
                                    self.state.liquidity_state.first_asset_amount.pop();
                                }
                                "liquidity_amount2" => {
                                    self.state.liquidity_state.second_asset_amount.pop();
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle settings screen specific events
    async fn handle_settings_screen_event(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::Char(c) => {
                self.handle_settings_input(c).await?;
            }
            Event::MoveFocus(direction) => {
                // Handle focus movement within settings screen
                match direction {
                    crate::tui::events::FocusDirection::Next => {
                        self.state.focus_manager.focus_next();
                    }
                    crate::tui::events::FocusDirection::Previous => {
                        self.state.focus_manager.focus_previous();
                    }
                    crate::tui::events::FocusDirection::Up => {
                        // Navigate to previous section
                        self.state.settings_state.previous_section();
                    }
                    crate::tui::events::FocusDirection::Down => {
                        // Navigate to next section
                        self.state.settings_state.next_section();
                    }
                    _ => {}
                }
            }
            Event::Enter => {
                // Handle enter key in settings
                if let Some(focused) = self.state.focus_manager.current_focus() {
                    match focused {
                        crate::tui::events::FocusableComponent::Button(button_id) => {
                            match button_id.as_str() {
                                "settings_save" => {
                                    self.handle_settings_action().await?;
                                }
                                "settings_reset" => {
                                    self.state.settings_state.reset_to_defaults();
                                }
                                _ => {}
                            }
                        }
                        crate::tui::events::FocusableComponent::Dropdown(dropdown_id) => {
                            // Toggle dropdown or handle selection
                            match dropdown_id.as_str() {
                                "settings_network" => {
                                    self.state.settings_state.toggle_network_environment();
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Escape => {
                // Handle escape key - close confirmation modal or go back
                if self.state.settings_state.show_confirmation {
                    self.state.settings_state.show_confirmation = false;
                } else if self.state.settings_state.message.is_some() {
                    self.state.settings_state.clear_message();
                }
            }
            Event::Backspace => {
                // Handle backspace for text input fields
                if let Some(focused) = self.state.focus_manager.current_focus() {
                    if let crate::tui::events::FocusableComponent::TextInput(field_id) = focused {
                        match field_id.as_str() {
                            "settings_rpc" => {
                                let _ = self.state.settings_state.handle_backspace();
                            }
                            "settings_wallet" => {
                                let _ = self.state.settings_state.handle_backspace();
                            }
                            _ => {}
                        }
                    }
                }
            }
            Event::ContextAction => {
                // Handle space bar context actions
                match self.state.settings_state.current_section {
                    crate::tui::screens::settings::SettingsSection::Network => {
                        self.state.settings_state.toggle_network_environment();
                    }
                    crate::tui::screens::settings::SettingsSection::Display => {
                        self.state.settings_state.toggle_theme();
                    }
                    crate::tui::screens::settings::SettingsSection::Wallet => {
                        self.state.settings_state.toggle_import_mode();
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Set an error with comprehensive error handling
    pub fn set_error(&mut self, message: String) {
        self.set_error_with_type(message, ErrorType::Unknown)
    }

    /// Set an error with specific error type
    pub fn set_error_with_type(&mut self, message: String, error_type: ErrorType) {
        self.state.error_message = Some(message.clone());
        self.state.loading_state = LoadingState::error(message.clone(), error_type.clone());

        // Show comprehensive error modal
        self.state.modal_state = Some(ModalState::error(
            "Error".to_string(),
            message,
            error_type,
            None,
            None,
        ));
    }

    /// Set an error with retry capability
    pub fn set_error_with_retry(
        &mut self,
        message: String,
        error_type: ErrorType,
        retry_action: String,
    ) {
        self.state.error_message = Some(message.clone());
        self.state.loading_state = LoadingState::error_with_retry(
            message.clone(),
            error_type.clone(),
            retry_action.clone(),
        );

        // Show error modal with retry option
        self.state.modal_state = Some(ModalState::error(
            "Error".to_string(),
            message,
            error_type,
            None,
            Some(retry_action),
        ));
    }

    /// Set a status message
    pub fn set_status(&mut self, message: String) {
        self.state.status_message = Some(message);
    }

    /// Clear error and status messages
    pub fn clear_messages(&mut self) {
        self.state.error_message = None;
        self.state.status_message = None;
        self.state.modal_state = None;
        if matches!(
            self.state.loading_state,
            LoadingState::Error { .. } | LoadingState::Success { .. }
        ) {
            self.state.loading_state = LoadingState::Idle;
        }
    }

    /// Set loading state with comprehensive progress tracking
    pub fn set_loading(&mut self, message: String) {
        self.set_loading_with_progress(message, None, false)
    }

    /// Set loading state with progress and cancellation support
    pub fn set_loading_with_progress(
        &mut self,
        message: String,
        progress: Option<f64>,
        can_cancel: bool,
    ) {
        self.state.loading_state = LoadingState::Loading {
            message: message.clone(),
            progress,
            can_cancel,
            operation_id: None,
        };

        // Show loading modal for long operations
        if can_cancel || progress.is_some() {
            self.state.modal_state = Some(ModalState::loading(
                "Processing".to_string(),
                message,
                progress,
                can_cancel,
            ));
        }
    }

    /// Update loading progress
    pub fn update_loading_progress(&mut self, progress: f64, message: Option<String>) {
        if let LoadingState::Loading {
            message: ref mut m,
            progress: ref mut p,
            ..
        } = self.state.loading_state
        {
            *p = Some(progress);
            if let Some(ref new_message) = message {
                *m = new_message.clone();
            }
        }

        // Update modal if visible
        if let Some(ref mut modal) = self.state.modal_state {
            modal.update_progress(Some(progress));
            if let Some(new_message) = message {
                modal.update_loading_message(new_message);
            }
        }
    }

    /// Set success state
    pub fn set_success(&mut self, message: String) {
        self.state.loading_state = LoadingState::success(message.clone());
        self.state.status_message = Some(message);
        // Clear any modal on success
        self.state.modal_state = None;
    }

    /// Show confirmation dialog
    pub fn show_confirmation(
        &mut self,
        title: String,
        message: String,
        confirm_text: Option<String>,
        cancel_text: Option<String>,
    ) {
        self.state.modal_state = Some(ModalState::confirmation(
            title,
            message,
            confirm_text,
            cancel_text,
        ));
    }

    /// Show help modal
    pub fn show_help(&mut self) {
        self.state.modal_state = Some(crate::tui::components::modals::create_comprehensive_help());
    }

    /// Show validation error
    pub fn show_validation_error(
        &mut self,
        field_name: String,
        error_message: String,
        suggestions: Vec<String>,
    ) {
        self.state.modal_state = Some(ModalState::validation_error(
            "Validation Error".to_string(),
            field_name,
            error_message,
            suggestions,
        ));
    }

    /// Handle modal events (navigation, confirmation, etc.)
    pub fn handle_modal_event(&mut self, event: &Event) -> bool {
        if let Some(ref mut modal) = self.state.modal_state {
            match event {
                Event::Up => {
                    modal.scroll_up();
                    return true;
                }
                Event::Down => {
                    modal.scroll_down();
                    return true;
                }
                Event::Left => {
                    modal.select_previous();
                    return true;
                }
                Event::Right => {
                    modal.select_next();
                    return true;
                }
                Event::Enter => {
                    // Handle confirmation or retry
                    let should_retry = modal.is_retry_selected();
                    let is_confirmed = modal.is_confirmed();

                    // Clear modal first
                    self.state.modal_state = None;

                    if should_retry {
                        // Implement retry logic based on the last failed operation
                        self.retry_last_operation();
                    } else if is_confirmed {
                        // Handle confirmation actions
                        self.handle_confirmation();
                    }
                    return true;
                }
                Event::Escape => {
                    self.state.modal_state = None;
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Retry the last failed operation
    fn retry_last_operation(&mut self) {
        // This would implement retry logic based on the operation type
        // For now, just refresh the current screen
        self.set_status("Retrying operation...".to_string());
    }

    /// Handle confirmation actions
    fn handle_confirmation(&mut self) {
        // This would implement confirmation-specific logic
        self.set_status("Action confirmed".to_string());
    }

    /// Navigate to a specific screen
    pub fn navigate_to(&mut self, screen: Screen) {
        self.state.current_screen = screen;
        self.state.navigation_mode = NavigationMode::ScreenLevel;
        self.clear_messages();
        // Don't initialize focus here - it will be done when user presses Enter
    }

    /// Navigate to the next tab
    pub fn next_tab(&mut self) {
        let screens = Screen::all();
        self.state.current_tab = (self.state.current_tab + 1) % screens.len();
        self.state.current_screen = screens[self.state.current_tab];
        self.state.navigation_mode = NavigationMode::ScreenLevel;
        self.clear_messages();
        // Don't initialize focus here - it will be done when user presses Enter
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
        self.state.navigation_mode = NavigationMode::ScreenLevel;
        self.clear_messages();
        // Don't initialize focus here - it will be done when user presses Enter
    }

    /// Handle background data refresh events
    async fn handle_data_refresh(
        &mut self,
        data_type: String,
        success: bool,
        error: Option<String>,
    ) -> Result<(), Error> {
        if !success {
            if let Some(err) = error {
                self.set_error(format!("Failed to refresh {}: {}", data_type, err));
            }
            return Ok(());
        }

        // Perform actual data refresh based on type
        match data_type.as_str() {
            "balances" => {
                if let Some(address) = &self.state.wallet_address.clone() {
                    // Refresh balances
                    if let Ok(balances) = self.client.get_balances().await {
                        for balance in balances {
                            self.state
                                .balances
                                .insert(balance.denom, balance.amount.to_string());
                        }
                    }

                    // Note: Wallet address updated for future background tasks
                }
            }
            "pools" => {
                // Refresh pool data
                if let Ok(pools) = self.client.get_pools(None).await {
                    for pool in pools {
                        let pool_id = pool.pool_info.pool_identifier.clone();
                        let cache_entry = PoolCacheEntry {
                            pool_info: pool,
                            cached_at: chrono::Utc::now(),
                        };
                        self.state.pool_cache.insert(pool_id, cache_entry);
                    }
                }
            }
            "transactions" => {
                // Refresh transaction status for pending transactions
                let pending_txs: Vec<String> = self
                    .state
                    .recent_transactions
                    .iter()
                    .filter(|tx| tx.status == TransactionStatus::Pending)
                    .map(|tx| tx.hash.clone())
                    .collect();

                // TODO: Implement transaction status checking when SDK supports it
                // For now, we'll mark pending transactions as unknown status
                for tx_hash in pending_txs {
                    if let Some(tx_info) = self
                        .state
                        .recent_transactions
                        .iter_mut()
                        .find(|tx| tx.hash == tx_hash)
                    {
                        // Mark as unknown for now - would check actual status in real implementation
                        tx_info.status = TransactionStatus::Unknown;
                    }
                }
            }
            "network_info" => {
                // Refresh network information
                if let Ok(height) = self.client.get_last_block_height().await {
                    self.state.block_height = Some(height);
                }

                // Update sync status
                self.state.network_info.last_sync_time = Some(chrono::Utc::now());
                self.state.network_info.is_syncing = false;
            }
            "prices" => {
                // Refresh price data - this would be implemented when price APIs are available
                // For now, just update the last sync time
                self.state.network_info.last_sync_time = Some(chrono::Utc::now());
            }
            _ => {
                // Unknown data type, log but don't error
                eprintln!("Unknown data refresh type: {}", data_type);
            }
        }

        Ok(())
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
            let _ = self.refresh_current_screen_data().await;
        }
    }

    /// Refresh settings data
    async fn refresh_dashboard_data(&mut self) -> Result<(), Error> {
        // Refresh balances, network info, and other dashboard data
        self.set_loading("Refreshing dashboard data...".to_string());

        let mut errors = Vec::new();

        // Refresh balances if wallet is connected
        if let Some(address) = &self.state.wallet_address.clone() {
            match self.client.get_balances().await {
                Ok(balances) => {
                    // Clear existing balances
                    self.state.balances.clear();
                    // Update with new balances
                    for balance in balances {
                        self.state
                            .balances
                            .insert(balance.denom, balance.amount.to_string());
                    }
                }
                Err(e) => {
                    errors.push(format!("Failed to fetch balances: {}", e));
                }
            }
        }

        // Refresh network info
        match self.client.get_last_block_height().await {
            Ok(height) => {
                self.state.block_height = Some(height);
                self.state.network_info.last_sync_time = Some(chrono::Utc::now());
                self.state.network_info.is_syncing = false;
            }
            Err(e) => {
                errors.push(format!("Failed to fetch block height: {}", e));
            }
        }

        // Refresh pool data (limited to avoid overwhelming)
        match self.client.get_pools(Some(20)).await {
            Ok(pools) => {
                for pool in pools {
                    let pool_id = pool.pool_info.pool_identifier.clone();
                    let cache_entry = PoolCacheEntry {
                        pool_info: pool,
                        cached_at: chrono::Utc::now(),
                    };
                    self.state.pool_cache.insert(pool_id, cache_entry);
                }
            }
            Err(e) => {
                errors.push(format!("Failed to fetch pools: {}", e));
            }
        }

        // Update background coordinator with wallet address for future syncing
        if let Some(address) = &self.state.wallet_address {
            if let Some(coordinator) = &mut self.background_coordinator {
                coordinator.set_wallet_address(address.clone());
            }
        }

        if errors.is_empty() {
            self.set_success("Dashboard refreshed successfully".to_string());
        } else {
            // Show partial success with warnings
            let error_summary = if errors.len() == 1 {
                errors[0].clone()
            } else {
                format!("Multiple errors occurred: {}", errors.join("; "))
            };
            self.set_error_with_type(
                format!(
                    "Dashboard refresh completed with warnings: {}",
                    error_summary
                ),
                crate::tui::components::modals::ErrorType::Network,
            );
        }

        Ok(())
    }

    async fn refresh_settings_data(&mut self) -> Result<(), Error> {
        // Initialize settings state with current config if needed
        if self.state.settings_state.current_config.mnemonic.is_none() {
            // Load current config into settings state
            let current_config = crate::config::Config {
                network: self.config.clone(),
                mnemonic: None, // We don't store mnemonic in memory for security
                tokens: std::collections::HashMap::new(),
            };
            self.state.settings_state =
                crate::tui::screens::settings::SettingsState::new(current_config);
        }
        Ok(())
    }

    /// Handle settings actions
    async fn handle_settings_action(&mut self) -> Result<(), Error> {
        // If showing confirmation, handle confirmation
        if self.state.settings_state.show_confirmation {
            // Save settings
            match self.state.settings_state.save_settings() {
                Ok(new_config) => {
                    // Update application config
                    self.config = new_config.network;
                    self.state.settings_state.show_confirmation = false;
                    self.set_success("Settings saved successfully!".to_string());
                }
                Err(e) => {
                    self.set_error(format!("Failed to save settings: {}", e));
                    self.state.settings_state.show_confirmation = false;
                }
            }
        } else if self.state.settings_state.has_changes {
            // Show confirmation dialog
            self.state.settings_state.show_confirmation = true;
        } else {
            // No changes, just refresh
            self.refresh_settings_data().await?;
        }
        Ok(())
    }

    /// Handle character input for settings screen
    async fn handle_settings_input(&mut self, c: char) -> Result<(), Error> {
        // Clear any messages first
        self.state.settings_state.clear_message();

        match c {
            // Ctrl+S - Save settings
            '\x13' => {
                if self.state.settings_state.has_changes {
                    self.state.settings_state.show_confirmation = true;
                } else {
                    self.state.settings_state.message =
                        Some(("No changes to save".to_string(), false));
                }
            }
            // Ctrl+R - Reset settings
            '\x12' => {
                self.state.settings_state.reset_to_defaults();
            }
            // Section navigation
            '\t' => {
                self.state.settings_state.next_section();
            }
            // Environment/theme toggles
            'e' => {
                if self.state.settings_state.current_section
                    == crate::tui::screens::settings::SettingsSection::Network
                {
                    self.state.settings_state.toggle_network_environment();
                }
            }
            't' => {
                if self.state.settings_state.current_section
                    == crate::tui::screens::settings::SettingsSection::Display
                {
                    self.state.settings_state.toggle_theme();
                }
            }
            'a' => {
                if self.state.settings_state.current_section
                    == crate::tui::screens::settings::SettingsSection::Wallet
                {
                    self.state.settings_state.toggle_import_mode();
                }
            }
            'm' => {
                if self.state.settings_state.current_section
                    == crate::tui::screens::settings::SettingsSection::Wallet
                {
                    self.state.settings_state.toggle_mnemonic_visibility();
                }
            }
            // Escape key handling
            '\x1b' => {
                if self.state.settings_state.show_confirmation {
                    self.state.settings_state.show_confirmation = false;
                } else if self.state.settings_state.message.is_some() {
                    self.state.settings_state.clear_message();
                }
            }
            // Backspace
            '\x08' | '\x7f' => {
                let _ = self.state.settings_state.handle_backspace();
            }
            // Regular character input
            _ => {
                if c.is_ascii_graphic() || c == ' ' {
                    let _ = self.state.settings_state.handle_char_input(c);
                }
            }
        }
        Ok(())
    }

    /// Handle wizard-specific events
    async fn handle_wizard_event(&mut self, event: Event) -> Result<bool, Error> {
        match event {
            Event::Quit => {
                // If we're in mnemonic input mode, treat 'q' as a regular character
                if self.state.wizard_state.current_step
                    == crate::tui::screens::wizard::WizardStep::WalletSetup
                    && self.state.wizard_state.import_existing
                {
                    self.state.wizard_state.mnemonic_input.push('q');
                } else {
                    self.state.should_quit = true;
                    return Ok(true);
                }
            }
            Event::Escape => {
                // Go back a step or exit wizard
                if self.state.wizard_state.current_step
                    == crate::tui::screens::wizard::WizardStep::Welcome
                {
                    // Skip wizard (not recommended)
                    self.state.wizard_state.finish_wizard();
                } else {
                    self.state.wizard_state.previous_step();
                }
            }
            Event::Enter => {
                // Proceed to next step or finish wizard
                if self.state.wizard_state.can_proceed() {
                    if self.state.wizard_state.current_step
                        == crate::tui::screens::wizard::WizardStep::Complete
                    {
                        // Finish wizard and apply settings
                        self.apply_wizard_settings().await?;
                        self.state.wizard_state.finish_wizard();
                    } else {
                        self.state.wizard_state.next_step();
                    }
                }
            }
            Event::Tab => {
                // Navigate between options in current step
                match self.state.wizard_state.current_step {
                    crate::tui::screens::wizard::WizardStep::NetworkSelection => {
                        self.state.wizard_state.toggle_network();
                    }
                    crate::tui::screens::wizard::WizardStep::WalletSetup => {
                        self.state.wizard_state.toggle_wallet_mode();
                    }
                    _ => {}
                }
            }
            Event::BackTab => {
                // Navigate between options in current step (reverse)
                match self.state.wizard_state.current_step {
                    crate::tui::screens::wizard::WizardStep::NetworkSelection => {
                        self.state.wizard_state.toggle_network();
                    }
                    crate::tui::screens::wizard::WizardStep::WalletSetup => {
                        self.state.wizard_state.toggle_wallet_mode();
                    }
                    _ => {}
                }
            }
            Event::Char(c) => {
                // Handle character input for mnemonic or other text fields
                match self.state.wizard_state.current_step {
                    crate::tui::screens::wizard::WizardStep::WalletSetup => {
                        if self.state.wizard_state.import_existing {
                            self.state.wizard_state.mnemonic_input.push(c);
                        }
                    }
                    crate::tui::screens::wizard::WizardStep::SecurityWarning => {
                        if c == 'y' || c == 'Y' {
                            self.state.wizard_state.security_acknowledged = true;
                        } else if c == 'n' || c == 'N' {
                            self.state.wizard_state.security_acknowledged = false;
                        }
                    }
                    _ => {}
                }
            }
            Event::Backspace => {
                // Handle backspace for text input
                match self.state.wizard_state.current_step {
                    crate::tui::screens::wizard::WizardStep::WalletSetup => {
                        if self.state.wizard_state.import_existing {
                            self.state.wizard_state.mnemonic_input.pop();
                        }
                    }
                    _ => {}
                }
            }
            // Treat context action (space bar) as a space character when typing mnemonic
            Event::ContextAction => {
                if self.state.wizard_state.current_step
                    == crate::tui::screens::wizard::WizardStep::WalletSetup
                    && self.state.wizard_state.import_existing
                {
                    self.state.wizard_state.mnemonic_input.push(' ');
                }
            }
            // Handle paste events (bracketed paste) for mnemonic field
            Event::Paste(text) => {
                if self.state.wizard_state.current_step
                    == crate::tui::screens::wizard::WizardStep::WalletSetup
                    && self.state.wizard_state.import_existing
                {
                    self.state.wizard_state.mnemonic_input.push_str(&text);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    /// Apply wizard settings to the app configuration
    async fn apply_wizard_settings(&mut self) -> Result<(), Error> {
        // Apply network settings
        match self.state.wizard_state.selected_network {
            crate::tui::screens::wizard::NetworkEnvironment::Mainnet => {
                self.set_status("Connected to Mainnet".to_string());
            }
            crate::tui::screens::wizard::NetworkEnvironment::Testnet => {
                self.set_status("Connected to Testnet".to_string());
            }
        }

        // Apply wallet settings
        if self.state.wizard_state.import_existing
            && !self.state.wizard_state.mnemonic_input.is_empty()
        {
            // Import wallet from mnemonic
            match crate::wallet::MantraWallet::from_mnemonic(
                &self.state.wizard_state.mnemonic_input,
                0,
            ) {
                Ok(wallet) => {
                    match wallet.address() {
                        Ok(address) => {
                            self.set_wallet_address(address.to_string());
                            // Reconfigure the client so all future calls have the wallet attached
                            self.configure_client_wallet(wallet).await?;
                            self.set_status("Wallet imported successfully".to_string());
                        }
                        Err(e) => {
                            self.set_error(format!("Failed to derive wallet address: {}", e));
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    self.set_error(format!("Failed to import wallet: {}", e));
                    return Err(e);
                }
            }
        } else if !self.state.wizard_state.import_existing {
            // Create new wallet
            if let Some(mnemonic) = &self.state.wizard_state.generated_mnemonic {
                match crate::wallet::MantraWallet::from_mnemonic(mnemonic, 0) {
                    Ok(wallet) => {
                        match wallet.address() {
                            Ok(address) => {
                                self.set_wallet_address(address.to_string());
                                // Reconfigure the client with the newly generated wallet
                                self.configure_client_wallet(wallet).await?;
                                self.set_status("New wallet created successfully".to_string());
                            }
                            Err(e) => {
                                self.set_error(format!("Failed to derive wallet address: {}", e));
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        self.set_error(format!("Failed to create wallet: {}", e));
                        return Err(e);
                    }
                }
            }
        }

        // Trigger dashboard refresh to reflect new wallet and network state
        if let Err(e) = self.refresh_dashboard_data().await {
            eprintln!("Warning: Failed to refresh dashboard data: {}", e);
        }

        Ok(())
    }

    /// Update the underlying client with a newly provided wallet and restart background tasks
    async fn configure_client_wallet(&mut self, wallet: crate::wallet::MantraWallet) -> Result<(), Error> {
        // Stop any currently running background sync tasks so they don't keep using the stale client
        self.stop_background_tasks();

        // Re-create a fresh client instance that includes the wallet
        let mut new_client = MantraDexClient::new(self.config.clone()).await?;
        new_client = new_client.with_wallet(wallet);

        // Replace the old Arc so all subsequent operations use the updated client
        self.client = std::sync::Arc::new(new_client);

        // Restart background tasks so they pick up the new client instance
        if let Some(sender) = self.event_sender.clone() {
            self.initialize_background_tasks(sender);
        }

        Ok(())
    }
}
