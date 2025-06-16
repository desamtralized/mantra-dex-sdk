//! Liquidity Screen Implementation
//!
//! This module provides the liquidity management interface for the MANTRA DEX SDK TUI,
//! allowing users to provide liquidity to pools, withdraw liquidity, view current positions,
//! and analyze position performance with PnL calculations.

use crate::tui::{
    app::{App, LoadingState},
    components::{
        forms::{InputType, TextInput},
        header::render_header,
        modals::{render_modal, ModalState},
        navigation::render_navigation,
        simple_list::{ListEvent, SimpleList, SimpleListOption},
        status_bar::render_status_bar,
        tables::format_large_number,
    },
};
use cosmwasm_std::Uint128;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};
use tui_input::InputRequest;

/// Liquidity screen operation modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidityMode {
    Provide,
    Withdraw,
    Positions,
}

/// Input focus states for the liquidity screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiquidityInputFocus {
    Mode,
    FirstAssetAmount,
    SecondAssetAmount,
    PoolSelection,
    SlippageAmount,
    SlippageSwap,
    WithdrawAmount,
    Execute,
}

/// Current liquidity position information
#[derive(Debug, Clone)]
pub struct LiquidityPosition {
    pub pool_id: String,
    pub asset_pair: String,
    pub lp_token_amount: Uint128,
    pub estimated_value_usd: f64,
    pub initial_value_usd: f64,
    pub pnl_percentage: f64,
    pub pnl_usd: f64,
    pub share_percentage: f64,
    pub first_asset_amount: Uint128,
    pub second_asset_amount: Uint128,
    pub first_asset_denom: String,
    pub second_asset_denom: String,
}

/// Liquidity screen state
#[derive(Debug, Clone)]
pub struct LiquidityScreenState {
    /// Current operation mode
    pub mode: LiquidityMode,
    /// Current input focus
    pub input_focus: LiquidityInputFocus,
    /// First asset amount input (for providing liquidity)
    pub first_asset_input: TextInput,
    /// Second asset amount input (for providing liquidity)
    pub second_asset_input: TextInput,
    /// Pool selection dropdown
    pub pool_dropdown: SimpleList,
    /// Slippage tolerance for liquidity operations
    pub slippage_amount_input: TextInput,
    /// Slippage tolerance for swap operations
    pub slippage_swap_input: TextInput,
    /// LP token amount input (for withdrawing liquidity)
    pub withdraw_amount_input: TextInput,
    /// Whether confirmation modal is shown
    pub show_confirmation: bool,
    /// Modal state for confirmations
    pub modal_state: Option<ModalState>,
    /// Available pools for liquidity operations
    pub available_pools: Vec<(String, String)>, // (pool_id, display_name)
    /// Current liquidity positions
    pub positions: Vec<LiquidityPosition>,
    /// Selected position index for details
    pub selected_position: Option<usize>,
    /// Expected LP tokens from providing liquidity
    pub expected_lp_tokens: Option<Uint128>,
    /// Expected assets from withdrawing liquidity
    pub expected_assets: Option<(Uint128, Uint128, String, String)>, // (amount1, amount2, denom1, denom2)
}

impl Default for LiquidityScreenState {
    fn default() -> Self {
        let first_asset_input = TextInput::new("First Asset Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let second_asset_input = TextInput::new("Second Asset Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let pool_dropdown = SimpleList::new("Select Pool");

        let slippage_amount_input = TextInput::new("Liquidity Slippage (%)")
            .with_type(InputType::Amount)
            .with_value("1.0")
            .with_placeholder("1.0");

        let slippage_swap_input = TextInput::new("Swap Slippage (%)")
            .with_type(InputType::Amount)
            .with_value("1.0")
            .with_placeholder("1.0");

        let withdraw_amount_input = TextInput::new("LP Token Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let mut instance = Self {
            mode: LiquidityMode::Provide,
            input_focus: LiquidityInputFocus::FirstAssetAmount,
            first_asset_input,
            second_asset_input,
            pool_dropdown,
            slippage_amount_input,
            slippage_swap_input,
            withdraw_amount_input,
            show_confirmation: false,
            modal_state: None,
            available_pools: Vec::new(),
            positions: Vec::new(),
            selected_position: None,
            expected_lp_tokens: None,
            expected_assets: None,
        };

        // Apply initial focus
        instance.apply_focus();
        instance
    }
}

impl LiquidityScreenState {
    /// Update available pools
    pub fn update_available_pools(&mut self, pools: Vec<(String, String)>) {
        crate::tui::utils::logger::log_info(&format!(
            "Updating available pools for liquidity screen: {} pools found",
            pools.len()
        ));

        self.available_pools = pools.clone();

        // Update pool list while preserving focus state
        let was_active = self.pool_dropdown.is_active;
        let mut dropdown = SimpleList::new("Select Pool");
        let options: Vec<SimpleListOption> = pools
            .iter()
            .map(|(pool_id, display_name)| {
                SimpleListOption::new(display_name.clone(), pool_id.clone())
            })
            .collect();
        dropdown = dropdown.with_options(options);
        dropdown.set_active(was_active);
        self.pool_dropdown = dropdown;

        crate::tui::utils::logger::log_info("Liquidity pool dropdown updated successfully");
    }

    /// Update current positions
    pub fn update_positions(&mut self, positions: Vec<LiquidityPosition>) {
        self.positions = positions;
        // Reset selection if it's out of bounds
        if let Some(selected) = self.selected_position {
            if selected >= self.positions.len() {
                self.selected_position = None;
            }
        }
    }

    /// Switch operation mode
    pub fn set_mode(&mut self, mode: LiquidityMode) {
        if self.mode != mode {
            self.mode = mode;
            self.clear_focus();

            // Always stay in Mode focus when switching tabs to maintain navigation
            self.input_focus = LiquidityInputFocus::Mode;

            crate::tui::utils::logger::log_info(&format!(
                "Liquidity mode switched to {:?}, maintaining Mode focus for navigation",
                mode
            ));
        }
    }

    /// Initialize focus state for the liquidity screen (called when entering the screen)
    pub fn initialize_focus(&mut self) {
        self.clear_focus();
        // Always start with Mode focus so users can see tab switching instructions
        self.input_focus = LiquidityInputFocus::Mode;

        crate::tui::utils::logger::log_info(&format!(
            "Liquidity screen focus initialized - Mode: {:?}, Focus: {:?}",
            self.mode, self.input_focus
        ));
    }

    /// Move focus to next input
    pub fn next_focus(&mut self) {
        self.clear_focus();
        self.input_focus = match self.mode {
            LiquidityMode::Provide => match self.input_focus {
                LiquidityInputFocus::FirstAssetAmount => LiquidityInputFocus::SecondAssetAmount,
                LiquidityInputFocus::SecondAssetAmount => LiquidityInputFocus::PoolSelection,
                LiquidityInputFocus::PoolSelection => LiquidityInputFocus::SlippageAmount,
                LiquidityInputFocus::SlippageAmount => LiquidityInputFocus::SlippageSwap,
                LiquidityInputFocus::SlippageSwap => LiquidityInputFocus::Execute,
                LiquidityInputFocus::Execute => LiquidityInputFocus::FirstAssetAmount,
                _ => LiquidityInputFocus::FirstAssetAmount,
            },
            LiquidityMode::Withdraw => match self.input_focus {
                LiquidityInputFocus::WithdrawAmount => LiquidityInputFocus::PoolSelection,
                LiquidityInputFocus::PoolSelection => LiquidityInputFocus::Execute,
                LiquidityInputFocus::Execute => LiquidityInputFocus::WithdrawAmount,
                _ => LiquidityInputFocus::WithdrawAmount,
            },
            LiquidityMode::Positions => LiquidityInputFocus::Mode,
        };
        self.set_focus();
    }

    /// Move focus to previous input
    pub fn previous_focus(&mut self) {
        self.clear_focus();
        self.input_focus = match self.mode {
            LiquidityMode::Provide => match self.input_focus {
                LiquidityInputFocus::FirstAssetAmount => LiquidityInputFocus::Execute,
                LiquidityInputFocus::SecondAssetAmount => LiquidityInputFocus::FirstAssetAmount,
                LiquidityInputFocus::PoolSelection => LiquidityInputFocus::SecondAssetAmount,
                LiquidityInputFocus::SlippageAmount => LiquidityInputFocus::PoolSelection,
                LiquidityInputFocus::SlippageSwap => LiquidityInputFocus::SlippageAmount,
                LiquidityInputFocus::Execute => LiquidityInputFocus::SlippageSwap,
                _ => LiquidityInputFocus::Execute,
            },
            LiquidityMode::Withdraw => match self.input_focus {
                LiquidityInputFocus::WithdrawAmount => LiquidityInputFocus::Execute,
                LiquidityInputFocus::PoolSelection => LiquidityInputFocus::WithdrawAmount,
                LiquidityInputFocus::Execute => LiquidityInputFocus::PoolSelection,
                _ => LiquidityInputFocus::Execute,
            },
            LiquidityMode::Positions => LiquidityInputFocus::Mode,
        };
        self.set_focus();
    }

    /// Clear focus from all inputs
    fn clear_focus(&mut self) {
        self.first_asset_input.set_focused(false);
        self.second_asset_input.set_focused(false);
        self.pool_dropdown.set_active(false);
        self.slippage_amount_input.set_focused(false);
        self.slippage_swap_input.set_focused(false);
        self.withdraw_amount_input.set_focused(false);
    }

    /// Public wrapper to clear all focus states (used by external modules)
    pub fn reset_focus(&mut self) {
        self.clear_focus();
    }

    /// Set focus on current input
    fn set_focus(&mut self) {
        match self.input_focus {
            LiquidityInputFocus::FirstAssetAmount => self.first_asset_input.set_focused(true),
            LiquidityInputFocus::SecondAssetAmount => self.second_asset_input.set_focused(true),
            LiquidityInputFocus::PoolSelection => self.pool_dropdown.set_active(true),
            LiquidityInputFocus::SlippageAmount => self.slippage_amount_input.set_focused(true),
            LiquidityInputFocus::SlippageSwap => self.slippage_swap_input.set_focused(true),
            LiquidityInputFocus::WithdrawAmount => self.withdraw_amount_input.set_focused(true),
            _ => {} // Mode and Execute focus handled separately
        }
    }

    /// Public wrapper to apply focus based on `input_focus` value (used by external modules)
    pub fn apply_focus(&mut self) {
        self.clear_focus();
        self.set_focus();
    }

    /// Check if any list is currently in editing mode
    pub fn is_any_list_editing(&self) -> bool {
        self.pool_dropdown.is_editing
    }

    /// Handle keyboard input using direct key events
    pub fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        navigation_mode: crate::tui::app::NavigationMode,
    ) -> bool {
        use crossterm::event::KeyCode;

        // Only handle events when in WithinScreen mode
        if navigation_mode != crate::tui::app::NavigationMode::WithinScreen {
            return false;
        }

        // Log significant key events for liquidity execution
        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
            && matches!(self.input_focus, LiquidityInputFocus::Execute)
        {
            crate::tui::utils::logger::log_info("=== LIQUIDITY EXECUTE KEY PRESSED ===");
            crate::tui::utils::logger::log_debug(&format!("Key event: {:?}", key));
            crate::tui::utils::logger::log_debug(&format!("Current focus: {:?}", self.input_focus));
        }

        // Handle regular input focus
        match self.input_focus {
            LiquidityInputFocus::Mode => {
                // Handle tab switching when in Mode focus
                match key.code {
                    KeyCode::Left => {
                        let new_mode = match self.mode {
                            LiquidityMode::Provide => LiquidityMode::Positions,
                            LiquidityMode::Withdraw => LiquidityMode::Provide,
                            LiquidityMode::Positions => LiquidityMode::Withdraw,
                        };
                        self.set_mode(new_mode);
                        crate::tui::utils::logger::log_info(&format!(
                            "Switched to liquidity mode: {:?}",
                            new_mode
                        ));
                        return true;
                    }
                    KeyCode::Right => {
                        let new_mode = match self.mode {
                            LiquidityMode::Provide => LiquidityMode::Withdraw,
                            LiquidityMode::Withdraw => LiquidityMode::Positions,
                            LiquidityMode::Positions => LiquidityMode::Provide,
                        };
                        self.set_mode(new_mode);
                        crate::tui::utils::logger::log_info(&format!(
                            "Switched to liquidity mode: {:?}",
                            new_mode
                        ));
                        return true;
                    }
                    KeyCode::Char('1') => {
                        self.set_mode(LiquidityMode::Provide);
                        crate::tui::utils::logger::log_info(
                            "Switched to Provide mode via number key",
                        );
                        return true;
                    }
                    KeyCode::Char('2') => {
                        self.set_mode(LiquidityMode::Withdraw);
                        crate::tui::utils::logger::log_info(
                            "Switched to Withdraw mode via number key",
                        );
                        return true;
                    }
                    KeyCode::Char('3') => {
                        self.set_mode(LiquidityMode::Positions);
                        crate::tui::utils::logger::log_info(
                            "Switched to Positions mode via number key",
                        );
                        return true;
                    }
                    KeyCode::Tab => {
                        // Tab moves focus to first focusable element in current mode
                        match self.mode {
                            LiquidityMode::Provide => {
                                self.input_focus = LiquidityInputFocus::FirstAssetAmount;
                                self.set_focus();
                            }
                            LiquidityMode::Withdraw => {
                                self.input_focus = LiquidityInputFocus::WithdrawAmount;
                                self.set_focus();
                            }
                            LiquidityMode::Positions => {
                                // Stay in Mode focus for positions view
                            }
                        }
                        return true;
                    }
                    KeyCode::Esc => {
                        // When in Mode focus, Esc should not exit the application
                        // but should be handled by the main app to return to ScreenLevel mode
                        return false; // Let the main app handle it properly
                    }
                    _ => return false,
                }
            }
            LiquidityInputFocus::PoolSelection => {
                let list_event = self.pool_dropdown.handle_key_event(key);

                if list_event == ListEvent::SelectionMade
                    || list_event == ListEvent::SelectionCancelled
                {
                    self.next_focus();
                }

                list_event != ListEvent::Ignored
            }
            LiquidityInputFocus::FirstAssetAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(tui_input::InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(tui_input::InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(tui_input::InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(tui_input::InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(tui_input::InputRequest::GoToNextChar),
                    KeyCode::Home => Some(tui_input::InputRequest::GoToStart),
                    KeyCode::End => Some(tui_input::InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    return self.first_asset_input.handle_input(request).is_some();
                }
                false
            }
            LiquidityInputFocus::SecondAssetAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(tui_input::InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(tui_input::InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(tui_input::InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(tui_input::InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(tui_input::InputRequest::GoToNextChar),
                    KeyCode::Home => Some(tui_input::InputRequest::GoToStart),
                    KeyCode::End => Some(tui_input::InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    return self.second_asset_input.handle_input(request).is_some();
                }
                false
            }
            LiquidityInputFocus::SlippageAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(tui_input::InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(tui_input::InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(tui_input::InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(tui_input::InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(tui_input::InputRequest::GoToNextChar),
                    KeyCode::Home => Some(tui_input::InputRequest::GoToStart),
                    KeyCode::End => Some(tui_input::InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    return self.slippage_amount_input.handle_input(request).is_some();
                }
                false
            }
            LiquidityInputFocus::SlippageSwap => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(tui_input::InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(tui_input::InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(tui_input::InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(tui_input::InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(tui_input::InputRequest::GoToNextChar),
                    KeyCode::Home => Some(tui_input::InputRequest::GoToStart),
                    KeyCode::End => Some(tui_input::InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    return self.slippage_swap_input.handle_input(request).is_some();
                }
                false
            }
            LiquidityInputFocus::WithdrawAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(tui_input::InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(tui_input::InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(tui_input::InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(tui_input::InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(tui_input::InputRequest::GoToNextChar),
                    KeyCode::Home => Some(tui_input::InputRequest::GoToStart),
                    KeyCode::End => Some(tui_input::InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    return self.withdraw_amount_input.handle_input(request).is_some();
                }
                false
            }
            LiquidityInputFocus::Execute => {
                // Handle execute button activation
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.validate() {
                            crate::tui::utils::logger::log_info(
                                "Liquidity execute button pressed - validation passed",
                            );
                            return true; // Event will be handled by app
                        } else {
                            crate::tui::utils::logger::log_warning(
                                "Liquidity validation failed - please check all fields",
                            );
                        }
                    }
                    _ => {}
                }
                false
            }
        }
    }

    /// Handle keyboard input (legacy method - kept for compatibility)
    pub fn handle_input(&mut self, input: InputRequest) -> bool {
        // This method is kept for backward compatibility with existing code
        // that still uses InputRequest. New code should use handle_key_event.
        match self.input_focus {
            LiquidityInputFocus::FirstAssetAmount => {
                self.first_asset_input.handle_input(input);
                true
            }
            LiquidityInputFocus::SecondAssetAmount => {
                self.second_asset_input.handle_input(input);
                true
            }
            LiquidityInputFocus::SlippageAmount => {
                self.slippage_amount_input.handle_input(input);
                true
            }
            LiquidityInputFocus::SlippageSwap => {
                self.slippage_swap_input.handle_input(input);
                true
            }
            LiquidityInputFocus::WithdrawAmount => {
                self.withdraw_amount_input.handle_input(input);
                true
            }
            _ => false,
        }
    }

    /// Navigate positions list
    pub fn navigate_positions(&mut self, next: bool) {
        if self.positions.is_empty() {
            return;
        }

        match self.selected_position {
            None => {
                self.selected_position = Some(0);
            }
            Some(current) => {
                if next {
                    self.selected_position = Some((current + 1) % self.positions.len());
                } else {
                    self.selected_position = Some(if current == 0 {
                        self.positions.len() - 1
                    } else {
                        current - 1
                    });
                }
            }
        }
    }

    /// Validate current form inputs
    pub fn validate(&mut self) -> bool {
        match self.mode {
            LiquidityMode::Provide => {
                let first_valid = self.first_asset_input.validate();
                let second_valid = self.second_asset_input.validate();
                let pool_valid = self.pool_dropdown.get_selected_value().is_some();
                let slippage_amount_valid = self.slippage_amount_input.validate();
                let slippage_swap_valid = self.slippage_swap_input.validate();

                first_valid
                    && second_valid
                    && pool_valid
                    && slippage_amount_valid
                    && slippage_swap_valid
            }
            LiquidityMode::Withdraw => {
                let withdraw_valid = self.withdraw_amount_input.validate();
                let pool_valid = self.pool_dropdown.get_selected_value().is_some();

                withdraw_valid && pool_valid
            }
            LiquidityMode::Positions => true, // No validation needed for positions view
        }
    }

    /// Get detailed validation errors for user feedback
    pub fn get_validation_errors(&mut self) -> Vec<String> {
        let mut errors = Vec::new();

        match self.mode {
            LiquidityMode::Provide => {
                if !self.first_asset_input.validate() {
                    if self.first_asset_input.value().is_empty() {
                        errors.push("Please enter first asset amount".to_string());
                    } else {
                        errors.push("Please enter a valid first asset amount".to_string());
                    }
                }

                if !self.second_asset_input.validate() {
                    if self.second_asset_input.value().is_empty() {
                        errors.push("Please enter second asset amount".to_string());
                    } else {
                        errors.push("Please enter a valid second asset amount".to_string());
                    }
                }

                if self.pool_dropdown.get_selected_value().is_none() {
                    errors.push("Please select a liquidity pool".to_string());
                }

                if !self.slippage_amount_input.validate() {
                    errors.push("Please enter valid liquidity slippage tolerance".to_string());
                }

                if !self.slippage_swap_input.validate() {
                    errors.push("Please enter valid swap slippage tolerance".to_string());
                }
            }
            LiquidityMode::Withdraw => {
                if !self.withdraw_amount_input.validate() {
                    if self.withdraw_amount_input.value().is_empty() {
                        errors.push("Please enter LP token amount to withdraw".to_string());
                    } else {
                        errors.push("Please enter a valid LP token amount".to_string());
                    }
                }

                if self.pool_dropdown.get_selected_value().is_none() {
                    errors.push("Please select a pool to withdraw from".to_string());
                }
            }
            LiquidityMode::Positions => {
                // No validation needed for positions view
            }
        }

        errors
    }

    /// Show confirmation modal for liquidity operations
    pub fn show_confirmation_modal(&mut self, operation_details: &LiquidityOperationDetails) {
        let title = match self.mode {
            LiquidityMode::Provide => "Confirm Provide Liquidity",
            LiquidityMode::Withdraw => "Confirm Withdraw Liquidity",
            _ => "Confirm Operation",
        };

        let content = match self.mode {
            LiquidityMode::Provide => format!(
                "Provide Liquidity:\n\n• First Asset: {} {}\n• Second Asset: {} {}\n• Pool: {}\n• Expected LP Tokens: {}\n• Liquidity Slippage: {}%\n• Swap Slippage: {}%\n\nProceed with transaction?",
                operation_details.first_amount,
                operation_details.first_asset,
                operation_details.second_amount,
                operation_details.second_asset,
                operation_details.pool_name,
                operation_details.expected_lp_tokens.clone().unwrap_or_else(|| "Calculating...".to_string()),
                operation_details.slippage_amount,
                operation_details.slippage_swap,
            ),
            LiquidityMode::Withdraw => format!(
                "Withdraw Liquidity:\n\n• LP Token Amount: {}\n• Pool: {}\n• Expected Assets: {}\n\nProceed with transaction?",
                operation_details.withdraw_amount.clone().unwrap_or_default(),
                operation_details.pool_name,
                operation_details.expected_assets.clone().unwrap_or_else(|| "Calculating...".to_string()),
            ),
            _ => "Invalid operation".to_string(),
        };

        self.modal_state = Some(ModalState::confirmation(
            title.to_string(),
            content,
            None,
            None,
        ));
        self.show_confirmation = true;
    }

    /// Hide confirmation modal
    pub fn hide_confirmation_modal(&mut self) {
        self.modal_state = None;
        self.show_confirmation = false;
    }
}

/// Liquidity operation details for confirmation
#[derive(Debug, Clone)]
pub struct LiquidityOperationDetails {
    pub first_amount: String,
    pub first_asset: String,
    pub second_amount: String,
    pub second_asset: String,
    pub pool_name: String,
    pub slippage_amount: String,
    pub slippage_swap: String,
    pub expected_lp_tokens: Option<String>,
    pub withdraw_amount: Option<String>,
    pub expected_assets: Option<String>,
}

// Global state for the liquidity screen
static mut LIQUIDITY_SCREEN_STATE: Option<LiquidityScreenState> = None;

pub fn get_liquidity_screen_state() -> &'static mut LiquidityScreenState {
    unsafe { LIQUIDITY_SCREEN_STATE.get_or_insert_with(LiquidityScreenState::default) }
}

/// Main render function for the liquidity screen
pub fn render_liquidity(f: &mut Frame, app: &App) {
    let size = f.area();

    // Create main layout: header, navigation, content, status
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header, navigation, and status bar
    render_header(f, &app.state, main_chunks[0]);
    render_navigation(f, &app.state, main_chunks[1]);
    render_status_bar(f, &app.state, main_chunks[3]);

    // Render liquidity content
    render_liquidity_content(f, main_chunks[2], app);

    // Render modal if shown
    let liquidity_state = get_liquidity_screen_state();
    if liquidity_state.show_confirmation {
        if let Some(ref modal_state) = liquidity_state.modal_state {
            render_modal(f, modal_state, size);
        }
    }

    // Render validation overlay if needed
    if app.state.current_screen == crate::tui::app::Screen::Liquidity {
        render_validation_overlay(f, size, app);
    }
}

/// Render the main liquidity content
fn render_liquidity_content(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    // Create tab titles with navigation hints
    let tabs = if matches!(liquidity_state.input_focus, LiquidityInputFocus::Mode) {
        vec!["1:Provide", "2:Withdraw", "3:Positions"]
    } else {
        vec!["Provide", "Withdraw", "Positions"]
    };

    let tab_index = match liquidity_state.mode {
        LiquidityMode::Provide => 0,
        LiquidityMode::Withdraw => 1,
        LiquidityMode::Positions => 2,
    };

    // Create layout with tabs and help text
    let has_mode_focus = matches!(liquidity_state.input_focus, LiquidityInputFocus::Mode);
    let chunks = if has_mode_focus {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tabs
                Constraint::Length(2), // Help text
                Constraint::Min(5),    // Content
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(7)])
            .split(area)
    };

    // Determine tab style based on focus
    let tab_border_style = if has_mode_focus {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Blue)
    };

    let tab_title = if has_mode_focus {
        "Liquidity Management [TAB FOCUS]"
    } else {
        "Liquidity Management"
    };

    // Render tabs
    let tabs_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(tab_border_style)
                .title(tab_title),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .select(tab_index);

    f.render_widget(tabs_widget, chunks[0]);

    // Render help text if in mode focus
    if has_mode_focus {
        let help_text = "← → : Switch tabs  |  1-3 : Jump to tab  |  Tab : Enter tab content  |  Esc : Return to screen navigation";
        let help_paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                    .border_style(Style::default().fg(Color::Gray)),
            );

        f.render_widget(help_paragraph, chunks[1]);

        // Render content based on current mode
        let content_area = chunks[2];
        match liquidity_state.mode {
            LiquidityMode::Provide => render_provide_liquidity_panel(f, content_area, app),
            LiquidityMode::Withdraw => render_withdraw_liquidity_panel(f, content_area, app),
            LiquidityMode::Positions => render_positions_panel(f, content_area, app),
        }
    } else {
        // Render content based on current mode
        let content_area = chunks[1];
        match liquidity_state.mode {
            LiquidityMode::Provide => render_provide_liquidity_panel(f, content_area, app),
            LiquidityMode::Withdraw => render_withdraw_liquidity_panel(f, content_area, app),
            LiquidityMode::Positions => render_positions_panel(f, content_area, app),
        }
    }
}

/// Render the provide liquidity panel
fn render_provide_liquidity_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left side: Input form
    render_provide_liquidity_form(f, chunks[0], app);

    // Right side: Preview and expected results
    render_provide_liquidity_preview(f, chunks[1], app);
}

/// Render the provide liquidity form
fn render_provide_liquidity_form(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // First asset
            Constraint::Length(3), // Second asset
            Constraint::Length(8), // Pool selection (taller for SimpleList)
            Constraint::Length(3), // Liquidity slippage
            Constraint::Length(3), // Swap slippage
            Constraint::Length(3), // Execute button
            Constraint::Min(0),    // Spacer
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Provide Liquidity");
    f.render_widget(block, area);

    // Render input fields
    liquidity_state.first_asset_input.render(f, chunks[0]);
    liquidity_state.second_asset_input.render(f, chunks[1]);

    // Render SimpleList (mutable access needed)
    let liquidity_state_mut = get_liquidity_screen_state();
    liquidity_state_mut.pool_dropdown.render(f, chunks[2]);

    liquidity_state.slippage_amount_input.render(f, chunks[3]);
    liquidity_state.slippage_swap_input.render(f, chunks[4]);

    // Render execute button
    render_execute_button(f, chunks[5], app, "Provide Liquidity");
}

/// Render the provide liquidity preview
fn render_provide_liquidity_preview(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Preview");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let preview_text = if liquidity_state.first_asset_input.value().is_empty()
        || liquidity_state.second_asset_input.value().is_empty()
        || liquidity_state.pool_dropdown.get_selected_value().is_none()
    {
        "Enter amounts and select pool to see preview"
    } else {
        "Calculating expected LP tokens..."
    };

    let expected_lp = liquidity_state
        .expected_lp_tokens
        .as_ref()
        .map(|tokens| {
            format!(
                "Expected LP Tokens: {}",
                format_large_number(&tokens.to_string())
            )
        })
        .unwrap_or_else(|| "Expected LP Tokens: Calculating...".to_string());

    let pool_name = liquidity_state
        .pool_dropdown
        .get_selected_label()
        .unwrap_or("No pool selected");

    let preview_content = format!(
        "{}\n\n{}\n\nPool: {}\n\nLiquidity Slippage: {}%\nSwap Slippage: {}%",
        preview_text,
        expected_lp,
        pool_name,
        liquidity_state.slippage_amount_input.value(),
        liquidity_state.slippage_swap_input.value(),
    );

    let paragraph = Paragraph::new(preview_content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render the withdraw liquidity panel
fn render_withdraw_liquidity_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left side: Input form
    render_withdraw_liquidity_form(f, chunks[0], app);

    // Right side: Expected assets
    render_withdraw_liquidity_preview(f, chunks[1], app);
}

/// Render the withdraw liquidity form
fn render_withdraw_liquidity_form(f: &mut Frame, area: Rect, app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // LP token amount
            Constraint::Length(8), // Pool selection (taller for SimpleList)
            Constraint::Length(3), // Execute button
            Constraint::Min(0),    // Spacer
        ])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Withdraw Liquidity");
    f.render_widget(block, area);

    // Render input fields
    liquidity_state.withdraw_amount_input.render(f, chunks[0]);

    // Render SimpleList (mutable access needed)
    let liquidity_state_mut = get_liquidity_screen_state();
    liquidity_state_mut.pool_dropdown.render(f, chunks[1]);

    // Render execute button
    render_execute_button(f, chunks[2], app, "Withdraw Liquidity");
}

/// Render the withdraw liquidity preview
fn render_withdraw_liquidity_preview(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Expected Assets");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let preview_text = if liquidity_state.withdraw_amount_input.value().is_empty()
        || liquidity_state.pool_dropdown.get_selected_value().is_none()
    {
        "Enter LP token amount and select pool"
    } else {
        "Calculating expected assets..."
    };

    let expected_assets = liquidity_state
        .expected_assets
        .as_ref()
        .map(|(amount1, amount2, denom1, denom2)| {
            format!(
                "Expected Assets:\n• {}: {}\n• {}: {}",
                denom1,
                format_large_number(&amount1.to_string()),
                denom2,
                format_large_number(&amount2.to_string())
            )
        })
        .unwrap_or_else(|| "Expected Assets: Calculating...".to_string());

    let pool_name = liquidity_state
        .pool_dropdown
        .get_selected_label()
        .unwrap_or("No pool selected");

    let preview_content = format!(
        "{}\n\n{}\n\nPool: {}",
        preview_text, expected_assets, pool_name,
    );

    let paragraph = Paragraph::new(preview_content)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, inner);
}

/// Render the positions panel
fn render_positions_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Top: Positions table
    render_positions_table(f, chunks[0], app);

    // Bottom: Position details
    render_position_details(f, chunks[1], app);
}

/// Render the current positions table
fn render_positions_table(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .title("Current Positions");

    if liquidity_state.positions.is_empty() {
        let empty_msg =
            Paragraph::new("No liquidity positions found\nProvide liquidity to see positions here")
                .style(Style::default().fg(Color::Gray))
                .block(block)
                .wrap(Wrap { trim: true });
        f.render_widget(empty_msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Pool").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Asset Pair").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("LP Tokens").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Value (USD)").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("PnL").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Share %").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let rows: Vec<Row> = liquidity_state
        .positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            let pnl_color = if position.pnl_percentage >= 0.0 {
                Color::Green
            } else {
                Color::Red
            };

            let pnl_text = format!("{:.2}% (${:.2})", position.pnl_percentage, position.pnl_usd);

            let mut row = Row::new(vec![
                Cell::from(position.pool_id.clone()),
                Cell::from(position.asset_pair.clone()),
                Cell::from(format_large_number(&position.lp_token_amount.to_string())),
                Cell::from(format!("${:.2}", position.estimated_value_usd)),
                Cell::from(pnl_text).style(Style::default().fg(pnl_color)),
                Cell::from(format!("{:.2}%", position.share_percentage)),
            ]);

            // Highlight selected row
            if Some(index) == liquidity_state.selected_position {
                row = row.style(Style::default().add_modifier(Modifier::REVERSED));
            }

            row
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(block);

    f.render_widget(table, area);
}

/// Render detailed information for the selected position
fn render_position_details(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title("Position Details");

    if let Some(selected_index) = liquidity_state.selected_position {
        if let Some(position) = liquidity_state.positions.get(selected_index) {
            let details = format!(
                "Pool ID: {}\n\nAsset Composition:\n• {}: {}\n• {}: {}\n\nPerformance:\n• Initial Value: ${:.2}\n• Current Value: ${:.2}\n• PnL: {:.2}% (${:.2})\n• Pool Share: {:.2}%\n\nLP Token Balance: {}",
                position.pool_id,
                position.first_asset_denom,
                format_large_number(&position.first_asset_amount.to_string()),
                position.second_asset_denom,
                format_large_number(&position.second_asset_amount.to_string()),
                position.initial_value_usd,
                position.estimated_value_usd,
                position.pnl_percentage,
                position.pnl_usd,
                position.share_percentage,
                format_large_number(&position.lp_token_amount.to_string()),
            );

            let paragraph = Paragraph::new(details)
                .style(Style::default().fg(Color::White))
                .block(block)
                .wrap(Wrap { trim: true });

            f.render_widget(paragraph, area);
        } else {
            let error_msg = Paragraph::new("Invalid position selection")
                .style(Style::default().fg(Color::Red))
                .block(block);
            f.render_widget(error_msg, area);
        }
    } else {
        let instruction_msg = Paragraph::new("Select a position from the table above to view details\n\nUse ↑/↓ arrow keys to navigate")
            .style(Style::default().fg(Color::Gray))
            .block(block)
            .wrap(Wrap { trim: true });
        f.render_widget(instruction_msg, area);
    }
}

/// Render execute button
fn render_execute_button(f: &mut Frame, area: Rect, app: &App, button_text: &str) {
    let liquidity_state = get_liquidity_screen_state();

    let button_style = if liquidity_state.input_focus == LiquidityInputFocus::Execute {
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    };

    let loading_text = match &app.state.loading_state {
        LoadingState::Loading { message, .. } => format!("Loading: {}", message),
        LoadingState::Success { message, .. } => format!("Success: {}", message),
        LoadingState::Error { message, .. } => format!("Error: {}", message),
        LoadingState::Idle => button_text.to_string(),
    };

    let button = Paragraph::new(loading_text)
        .style(button_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(button_style),
        );

    f.render_widget(button, area);
}

/// Handle liquidity screen input
pub fn handle_liquidity_screen_input(input: InputRequest) -> bool {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.handle_input(input)
}

/// Handle liquidity screen navigation
pub fn handle_liquidity_screen_navigation(next: bool) {
    let liquidity_state = get_liquidity_screen_state();

    match liquidity_state.mode {
        LiquidityMode::Positions => {
            liquidity_state.navigate_positions(next);
        }
        _ => {
            if next {
                liquidity_state.next_focus();
            } else {
                liquidity_state.previous_focus();
            }
        }
    }
}

/// Switch liquidity mode
pub fn switch_liquidity_mode(mode: LiquidityMode) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.set_mode(mode);
}

/// Execute liquidity operation with confirmation
pub fn execute_liquidity_operation() {
    let liquidity_state = get_liquidity_screen_state();

    if !liquidity_state.validate() {
        return;
    }

    let operation_details = match liquidity_state.mode {
        LiquidityMode::Provide => LiquidityOperationDetails {
            first_amount: liquidity_state.first_asset_input.value().to_string(),
            first_asset: "Asset1".to_string(), // This would come from pool info
            second_amount: liquidity_state.second_asset_input.value().to_string(),
            second_asset: "Asset2".to_string(), // This would come from pool info
            pool_name: liquidity_state
                .pool_dropdown
                .get_selected_label()
                .unwrap_or("Unknown Pool")
                .to_string(),
            slippage_amount: liquidity_state.slippage_amount_input.value().to_string(),
            slippage_swap: liquidity_state.slippage_swap_input.value().to_string(),
            expected_lp_tokens: liquidity_state
                .expected_lp_tokens
                .as_ref()
                .map(|t| t.to_string()),
            withdraw_amount: None,
            expected_assets: None,
        },
        LiquidityMode::Withdraw => LiquidityOperationDetails {
            first_amount: String::new(),
            first_asset: String::new(),
            second_amount: String::new(),
            second_asset: String::new(),
            pool_name: liquidity_state
                .pool_dropdown
                .get_selected_label()
                .unwrap_or("Unknown Pool")
                .to_string(),
            slippage_amount: String::new(),
            slippage_swap: String::new(),
            expected_lp_tokens: None,
            withdraw_amount: Some(liquidity_state.withdraw_amount_input.value().to_string()),
            expected_assets: liquidity_state
                .expected_assets
                .as_ref()
                .map(|(a1, a2, d1, d2)| format!("{} {}, {} {}", a1, d1, a2, d2)),
        },
        LiquidityMode::Positions => return, // No operation for positions view
    };

    liquidity_state.show_confirmation_modal(&operation_details);
}

/// Handle confirmation response
pub fn handle_liquidity_confirmation_response(confirmed: bool) -> bool {
    let liquidity_state = get_liquidity_screen_state();

    if confirmed {
        // TODO: Execute actual liquidity operation through the DEX client
        liquidity_state.hide_confirmation_modal();
        true
    } else {
        liquidity_state.hide_confirmation_modal();
        false
    }
}

/// Reset liquidity forms
pub fn reset_liquidity_forms() {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.first_asset_input.clear();
    liquidity_state.second_asset_input.clear();
    liquidity_state.withdraw_amount_input.clear();
    liquidity_state.expected_lp_tokens = None;
    liquidity_state.expected_assets = None;
}

/// Update expected LP tokens from calculation
pub fn update_expected_lp_tokens(amount: Uint128) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.expected_lp_tokens = Some(amount);
}

/// Update expected assets from withdrawal calculation
pub fn update_expected_assets(amount1: Uint128, amount2: Uint128, denom1: String, denom2: String) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.expected_assets = Some((amount1, amount2, denom1, denom2));
}

/// Update liquidity positions
pub fn update_liquidity_positions(positions: Vec<LiquidityPosition>) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.update_positions(positions);
}

/// Update available pools for liquidity operations
pub fn update_liquidity_pools(pools: Vec<(String, String)>) {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.update_available_pools(pools);
}

/// Initialize focus for the liquidity screen (called when entering the screen)
pub fn initialize_liquidity_screen_focus() {
    let liquidity_state = get_liquidity_screen_state();
    liquidity_state.initialize_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liquidity_screen_state_navigation() {
        let mut state = LiquidityScreenState::default();

        // Test provide mode navigation
        assert_eq!(state.input_focus, LiquidityInputFocus::FirstAssetAmount);
        state.next_focus();
        assert_eq!(state.input_focus, LiquidityInputFocus::SecondAssetAmount);
        state.next_focus();
        assert_eq!(state.input_focus, LiquidityInputFocus::PoolSelection);

        // Test mode switching
        state.set_mode(LiquidityMode::Withdraw);
        assert_eq!(state.mode, LiquidityMode::Withdraw);
        assert_eq!(state.input_focus, LiquidityInputFocus::WithdrawAmount);
    }

    #[test]
    fn test_liquidity_validation() {
        let mut state = LiquidityScreenState::default();

        // Test provide mode validation
        assert!(!state.validate()); // Should fail with empty inputs

        state.first_asset_input = state.first_asset_input.with_value("100");
        state.second_asset_input = state.second_asset_input.with_value("200");
        // Still should fail without pool selection
        assert!(!state.validate());
    }

    #[test]
    fn test_position_navigation() {
        let mut state = LiquidityScreenState::default();
        state.set_mode(LiquidityMode::Positions);

        // Add test positions
        let positions = vec![
            LiquidityPosition {
                pool_id: "1".to_string(),
                asset_pair: "USDC/USDT".to_string(),
                lp_token_amount: Uint128::new(1000),
                estimated_value_usd: 1000.0,
                initial_value_usd: 950.0,
                pnl_percentage: 5.26,
                pnl_usd: 50.0,
                share_percentage: 0.1,
                first_asset_amount: Uint128::new(500),
                second_asset_amount: Uint128::new(500),
                first_asset_denom: "USDC".to_string(),
                second_asset_denom: "USDT".to_string(),
            },
            LiquidityPosition {
                pool_id: "2".to_string(),
                asset_pair: "ATOM/OSMO".to_string(),
                lp_token_amount: Uint128::new(2000),
                estimated_value_usd: 2000.0,
                initial_value_usd: 2100.0,
                pnl_percentage: -4.76,
                pnl_usd: -100.0,
                share_percentage: 0.2,
                first_asset_amount: Uint128::new(1000),
                second_asset_amount: Uint128::new(1000),
                first_asset_denom: "ATOM".to_string(),
                second_asset_denom: "OSMO".to_string(),
            },
        ];

        state.update_positions(positions);

        // Test navigation
        state.navigate_positions(true);
        assert_eq!(state.selected_position, Some(0));

        state.navigate_positions(true);
        assert_eq!(state.selected_position, Some(1));

        state.navigate_positions(true);
        assert_eq!(state.selected_position, Some(0)); // Should wrap around
    }
}

/// Render validation error overlay for immediate feedback
fn render_validation_overlay(f: &mut Frame, area: Rect, _app: &App) {
    let liquidity_state = get_liquidity_screen_state();

    // Only show validation errors when the execute button is focused and validation fails
    if !matches!(liquidity_state.input_focus, LiquidityInputFocus::Execute) {
        return;
    }

    if liquidity_state.clone().validate() {
        return; // No errors to display
    }

    let errors = liquidity_state.clone().get_validation_errors();
    if errors.is_empty() {
        return;
    }

    // Create a small overlay at the bottom of the screen
    let overlay_height = (errors.len() + 2).min(6) as u16;
    let overlay_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - overlay_height - 4,
        width: area.width - 4,
        height: overlay_height,
    };

    // Clear the area
    f.render_widget(ratatui::widgets::Clear, overlay_area);

    // Create error content
    let error_lines: Vec<ratatui::text::Line> = errors
        .iter()
        .map(|error| {
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("• ", Style::default().fg(Color::Red)),
                ratatui::text::Span::styled(error, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let error_text = ratatui::text::Text::from(error_lines);

    let error_block = Paragraph::new(error_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title("Validation Errors")
                .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        )
        .style(Style::default().bg(Color::Black))
        .wrap(Wrap { trim: true });

    f.render_widget(error_block, overlay_area);
}
