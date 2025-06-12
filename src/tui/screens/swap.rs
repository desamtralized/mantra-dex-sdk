//! Swap Screen Implementation
//!
//! This module provides the swap interface for the MANTRA DEX SDK TUI,
//! allowing users to perform token swaps with price impact calculations,
//! slippage settings, and transaction execution.

use crate::tui::{
    app::{App, LoadingState, SwapState},
    components::{
        forms::{InputType, TextInput},
        header::render_header,
        modals::{render_modal, ModalState},
        navigation::render_navigation,
        simple_list::{ListEvent, SimpleList, SimpleListOption},
        status_bar::render_status_bar,
    },
};
use mantra_dex_std::pool_manager::SimulationResponse;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph, Wrap},
    Frame,
};
use tui_input::InputRequest;

/// Input focus states for the swap screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwapInputFocus {
    Pool,
    FromToken,
    FromAmount,
    Slippage,
    Execute,
}

/// Current swap screen state
#[derive(Debug, Clone)]
pub struct SwapScreenState {
    /// Current input focus
    pub input_focus: SwapInputFocus,
    /// Pool selection list
    pub pool_dropdown: SimpleList,
    /// From token list
    pub from_token_dropdown: SimpleList,
    /// From amount input
    pub from_amount_input: TextInput,
    /// Slippage tolerance input
    pub slippage_input: TextInput,
    /// Whether confirmation modal is shown
    pub show_confirmation: bool,
    /// Modal state for confirmations
    pub modal_state: Option<ModalState>,
    /// Available tokens for selection
    pub available_tokens: Vec<String>,
    /// Available pools for the selected token pair
    pub available_pools: Vec<(String, String)>, // (pool_id, display_name)
    /// Timer for simulation trigger
    pub simulation_timer: Option<std::time::Instant>,
    /// Last input change time for simulation delay
    pub last_input_change: Option<std::time::Instant>,
}

impl Default for SwapScreenState {
    fn default() -> Self {
        let mut pool_dropdown = SimpleList::new("Available Pools");

        // Initialize with test data immediately (for development/testing)
        let test_pools = vec![
            ("1".to_string(), "Pool 1: USDC / USDT".to_string()),
            ("2".to_string(), "Pool 2: ATOM / OSMO".to_string()),
            ("3".to_string(), "Pool 3: MANTRA / USDC".to_string()),
            ("4".to_string(), "Pool 4: USDT / ATOM".to_string()),
            ("5".to_string(), "Pool 5: OSMO / MANTRA".to_string()),
        ];

        let pool_options: Vec<SimpleListOption> = test_pools
            .iter()
            .map(|(pool_id, display_name)| {
                SimpleListOption::new(display_name.clone(), pool_id.clone())
            })
            .collect();
        pool_dropdown = pool_dropdown.with_options(pool_options);

        // Start with empty token list - will be populated when a pool is selected
        let from_token_dropdown = SimpleList::new("Pool Tokens");

        let mut from_amount_input = TextInput::new("From Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let slippage_input = TextInput::new("Slippage Tolerance (%)")
            .with_type(InputType::Amount)
            .with_value("1.0")
            .with_placeholder("1.0");

        // Set initial focus
        from_amount_input.set_focused(false);

        let mut instance = Self {
            input_focus: SwapInputFocus::Pool,
            pool_dropdown,
            from_token_dropdown,
            from_amount_input,
            slippage_input,
            show_confirmation: false,
            modal_state: None,
            available_tokens: Vec::new(), // Will be populated when pool is selected
            available_pools: test_pools,
            simulation_timer: None,
            last_input_change: None,
        };

        // Apply initial focus
        instance.apply_focus();
        instance
    }
}

impl SwapScreenState {
    /// Initialize lists with available tokens
    pub fn initialize_tokens(&mut self, tokens: Vec<String>) {
        self.available_tokens = tokens.clone();

        // Update from token list while preserving focus state
        let was_active = self.from_token_dropdown.is_active;
        let mut dropdown = SimpleList::new("Available Tokens");
        let options: Vec<SimpleListOption> = tokens
            .iter()
            .map(|token| SimpleListOption::new(token.clone(), token.clone()))
            .collect();
        dropdown = dropdown.with_options(options);
        dropdown.set_active(was_active);
        self.from_token_dropdown = dropdown;
    }

    /// Update available pools based on selected tokens
    pub fn update_available_pools(&mut self, pools: Vec<(String, String)>) {
        self.available_pools = pools.clone();

        // Update pool list while preserving focus state
        let was_active = self.pool_dropdown.is_active;
        let mut dropdown = SimpleList::new("Available Pools");
        let options: Vec<SimpleListOption> = pools
            .iter()
            .map(|(pool_id, display_name)| {
                SimpleListOption::new(display_name.clone(), pool_id.clone())
            })
            .collect();
        dropdown = dropdown.with_options(options);
        dropdown.set_active(was_active);
        self.pool_dropdown = dropdown;
    }

    /// Update token list based on selected pool
    pub fn update_tokens_for_pool(&mut self, pool_id: &str) {
        // Attempt to derive the token pair for the selected pool using the cached `available_pools`.
        // `available_pools` entries are (pool_id, display_name) where display_name was created as
        // "<token_a> / <token_b>" in `App::update_swap_screen_pools`.  We can therefore recover the
        // two asset symbols by splitting on "/".

        let tokens_for_pool: Vec<String> = self
            .available_pools
            .iter()
            .find(|(id, _)| id == pool_id)
            .and_then(|(_, name)| {
                // Expected format: "Pool <num>: TOKEN_A / TOKEN_B"
                let after_colon = name
                    .split(':')
                    .nth(1) // take text after the first ':'
                    .map(|s| s.trim())?; // remove leading/trailing spaces

                let parts: Vec<String> = after_colon
                    .split('/')
                    .map(|s| s.trim().to_string())
                    .collect();

                if parts.len() == 2 {
                    Some(parts)
                } else {
                    None
                }
            })
            .unwrap_or_else(Vec::new);

        // Fallback: if we could not determine tokens from the pool name, keep the full list of
        // available tokens so the user is not left with an empty dropdown.
        let tokens_for_pool = if tokens_for_pool.is_empty() {
            self.available_tokens.clone()
        } else {
            tokens_for_pool
        };

        // Preserve current state
        let was_active = self.from_token_dropdown.is_active;
        let was_editing = self.from_token_dropdown.is_editing;

        // Update the options while preserving state
        let options: Vec<SimpleListOption> = tokens_for_pool
            .iter()
            .map(|token| SimpleListOption::new(token.clone(), token.clone()))
            .collect();

        self.from_token_dropdown.options = options;
        self.from_token_dropdown.label = "Pool Tokens".to_string();

        // Restore state
        self.from_token_dropdown.is_active = was_active;
        self.from_token_dropdown.is_editing = was_editing;

        // Reset selection and list state
        self.from_token_dropdown.selected_index = None;
        if !tokens_for_pool.is_empty() {
            self.from_token_dropdown.list_state.select(Some(0));
        } else {
            self.from_token_dropdown.list_state.select(None);
        }
    }

    /// Clear focus from all inputs
    fn clear_focus(&mut self) {
        self.pool_dropdown.set_active(false);
        self.from_token_dropdown.set_active(false);
        self.from_amount_input.set_focused(false);
        self.slippage_input.set_focused(false);
    }

    /// Public wrapper to clear all focus states (used by external modules)
    pub fn reset_focus(&mut self) {
        self.clear_focus();
    }

    /// Set focus on current input
    fn set_focus(&mut self) {
        match self.input_focus {
            SwapInputFocus::Pool => {
                self.pool_dropdown.set_active(true);
            }
            SwapInputFocus::FromToken => {
                self.from_token_dropdown.set_active(true);
            }
            SwapInputFocus::FromAmount => self.from_amount_input.set_focused(true),
            SwapInputFocus::Slippage => self.slippage_input.set_focused(true),
            SwapInputFocus::Execute => {} // Button focus handled separately
        }
    }

    /// Public wrapper to apply focus based on `input_focus` value (used by external modules)
    pub fn apply_focus(&mut self) {
        self.set_focus();
    }

    /// Mark input change for simulation trigger
    pub fn mark_input_change(&mut self) {
        self.last_input_change = Some(std::time::Instant::now());
    }

    /// Check if simulation should be triggered (after 5 seconds of inactivity)
    pub fn should_trigger_simulation(&mut self) -> bool {
        if let Some(last_change) = self.last_input_change {
            let elapsed = last_change.elapsed().as_secs();
            elapsed >= 5 && self.validate()
        } else {
            false
        }
    }

    /// Reset the simulation timer
    pub fn reset_simulation_timer(&mut self) {
        self.last_input_change = None;
        self.simulation_timer = None;
    }

    /// Check if any list is currently in editing mode
    pub fn is_any_list_editing(&self) -> bool {
        self.pool_dropdown.is_editing || self.from_token_dropdown.is_editing
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

        // Handle regular input focus
        match self.input_focus {
            SwapInputFocus::Pool => {
                let old_selection = self.pool_dropdown.selected_index;
                let list_event = self.pool_dropdown.handle_key_event(key);

                // Only update tokens when selection is confirmed, not during navigation
                if list_event == ListEvent::SelectionMade {
                    if let Some(selected_pool_value) = self
                        .pool_dropdown
                        .get_selected_value()
                        .map(|v| v.to_string())
                    {
                        self.update_tokens_for_pool(&selected_pool_value);
                    }
                    self.mark_input_change();
                }

                if list_event == ListEvent::SelectionMade
                    || list_event == ListEvent::SelectionCancelled
                {
                    self.next_focus();
                }

                list_event != ListEvent::Ignored
            }
            SwapInputFocus::FromToken => {
                let old_selection = self.from_token_dropdown.selected_index;
                let list_event = self.from_token_dropdown.handle_key_event(key);

                if self.from_token_dropdown.selected_index != old_selection {
                    self.mark_input_change();
                }

                if list_event == ListEvent::SelectionMade
                    || list_event == ListEvent::SelectionCancelled
                {
                    self.next_focus();
                }

                list_event != ListEvent::Ignored
            }
            SwapInputFocus::FromAmount => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(InputRequest::GoToNextChar),
                    KeyCode::Home => Some(InputRequest::GoToStart),
                    KeyCode::End => Some(InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    if self.from_amount_input.handle_input(request).is_some() {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }
            SwapInputFocus::Slippage => {
                let input_request = match key.code {
                    KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
                    KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
                    KeyCode::Delete => Some(InputRequest::DeleteNextChar),
                    KeyCode::Left => Some(InputRequest::GoToPrevChar),
                    KeyCode::Right => Some(InputRequest::GoToNextChar),
                    KeyCode::Home => Some(InputRequest::GoToStart),
                    KeyCode::End => Some(InputRequest::GoToEnd),
                    _ => None,
                };

                if let Some(request) = input_request {
                    if self.slippage_input.handle_input(request).is_some() {
                        self.mark_input_change();
                        return true;
                    }
                }
                false
            }
            SwapInputFocus::Execute => {
                // Handle execute button activation
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if self.validate() {
                            // Trigger swap execution
                            return true;
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
        false
    }

    /// Move to next focus (for testing/compatibility)
    pub fn next_focus(&mut self) {
        self.input_focus = match self.input_focus {
            SwapInputFocus::Pool => SwapInputFocus::FromToken,
            SwapInputFocus::FromToken => SwapInputFocus::FromAmount,
            SwapInputFocus::FromAmount => SwapInputFocus::Slippage,
            SwapInputFocus::Slippage => SwapInputFocus::Execute,
            SwapInputFocus::Execute => SwapInputFocus::Pool,
        };
        self.clear_focus();
        self.set_focus();
    }

    /// Move to previous focus
    pub fn previous_focus(&mut self) {
        self.input_focus = match self.input_focus {
            SwapInputFocus::Pool => SwapInputFocus::Execute,
            SwapInputFocus::FromToken => SwapInputFocus::Pool,
            SwapInputFocus::FromAmount => SwapInputFocus::FromToken,
            SwapInputFocus::Slippage => SwapInputFocus::FromAmount,
            SwapInputFocus::Execute => SwapInputFocus::Slippage,
        };
        self.clear_focus();
        self.set_focus();
    }

    /// Validate all inputs
    pub fn validate(&mut self) -> bool {
        let pool_valid = self.pool_dropdown.get_selected_value().is_some();
        let from_token_valid = self.from_token_dropdown.get_selected_value().is_some();
        let amount_valid = self.from_amount_input.validate();
        let slippage_valid = self.slippage_input.validate();

        pool_valid && from_token_valid && amount_valid && slippage_valid
    }

    /// Show confirmation modal
    pub fn show_confirmation_modal(&mut self, swap_details: &SwapDetails) {
        let message = format!(
            "Confirm swap:\n{} {} → {} {}\nPool: {}\nSlippage: {}%\nExpected output: {} {}\nPrice impact: {:.2}%",
            swap_details.from_amount,
            swap_details.from_token,
            swap_details.to_amount,
            swap_details.to_token,
            swap_details.pool_name,
            swap_details.slippage,
            swap_details.expected_output,
            swap_details.to_token,
            swap_details.price_impact
        );

        self.modal_state = Some(ModalState::confirmation(
            "Confirm Swap".to_string(),
            message,
            Some("Execute Swap".to_string()),
            Some("Cancel".to_string()),
        ));
        self.show_confirmation = true;
    }

    /// Hide confirmation modal
    pub fn hide_confirmation_modal(&mut self) {
        self.modal_state = None;
        self.show_confirmation = false;
    }
}

/// Swap details for confirmation
#[derive(Debug, Clone)]
pub struct SwapDetails {
    pub from_amount: String,
    pub from_token: String,
    pub to_amount: String,
    pub to_token: String,
    pub pool_name: String,
    pub slippage: String,
    pub expected_output: String,
    pub price_impact: f64,
    pub fee_amount: String,
}

// Global swap screen state - in a real implementation this would be part of the app state
static mut SWAP_SCREEN_STATE: Option<SwapScreenState> = None;

/// Get or initialize the swap screen state
pub(crate) fn get_swap_screen_state() -> &'static mut SwapScreenState {
    unsafe {
        if SWAP_SCREEN_STATE.is_none() {
            SWAP_SCREEN_STATE = Some(SwapScreenState::default());
        }
        SWAP_SCREEN_STATE.as_mut().unwrap()
    }
}

/// Render the complete swap screen
pub fn render_swap(f: &mut Frame, app: &App) {
    let size = f.area();

    // Create main layout: header, nav, content, status
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Navigation
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header and navigation
    render_header(f, &app.state, chunks[0]);
    render_navigation(f, &app.state, chunks[1]);

    // Check for simulation trigger (this should ideally be in the main event loop, but putting here for now)
    let swap_state = get_swap_screen_state();
    if swap_state.should_trigger_simulation() {
        // Reset the timer to prevent repeated triggers
        if let Some(sender) = app.get_event_sender() {
            let _ = sender.send(crate::tui::events::Event::TriggerSimulation);
        }
    }

    // Render swap content
    render_swap_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);

    // Render modal if visible
    if let Some(ref modal_state) = swap_state.modal_state {
        render_modal(f, modal_state, size);
    }
}

/// Render the main swap content area
fn render_swap_content(f: &mut Frame, area: Rect, app: &App) {
    // Create a horizontal layout: swap interface | simulation results only
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Split the left side: swap interface on top, execute button on bottom
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(main_chunks[0]);

    // Render components
    render_swap_interface(f, left_chunks[0], app);
    render_execute_button(f, left_chunks[1], app);
    render_simulation_results(f, main_chunks[1], app);
}

/// Render the swap input interface
fn render_swap_interface(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Swap Interface")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .padding(Padding::uniform(1));

    // Create vertical layout for form inputs
    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Pool selection list
            Constraint::Length(8), // Token selection list
            Constraint::Length(5), // From amount input (increased for better visibility)
            Constraint::Length(5), // Slippage tolerance (increased for better visibility)
        ])
        .split(block.inner(area));

    let swap_state = get_swap_screen_state();

    // Render form inputs
    render_pool_selection(f, input_chunks[0], app, swap_state);
    render_from_token_input(f, input_chunks[1], app, swap_state);
    render_from_amount_input(f, input_chunks[2], app, swap_state);
    render_slippage_input(f, input_chunks[3], app, swap_state);

    f.render_widget(block, area);
}

/// Render pool selection list
fn render_pool_selection(f: &mut Frame, area: Rect, _app: &App, swap_state: &mut SwapScreenState) {
    swap_state.pool_dropdown.render(f, area);
}

/// Render from token selection list
fn render_from_token_input(
    f: &mut Frame,
    area: Rect,
    _app: &App,
    swap_state: &mut SwapScreenState,
) {
    swap_state.from_token_dropdown.render(f, area);
}

/// Render from amount input with balance display
fn render_from_amount_input(
    f: &mut Frame,
    area: Rect,
    app: &App,
    swap_state: &mut SwapScreenState,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    // Render input field
    swap_state.from_amount_input.render(f, chunks[0]);

    // Render balance display
    let from_token = swap_state
        .from_token_dropdown
        .get_selected_label()
        .unwrap_or("Select Token");

    let balance_text = if from_token == "Select Token" {
        vec![
            Line::from(vec![Span::styled(
                "Select token",
                Style::default().fg(Color::Gray),
            )]),
            Line::from(vec![Span::styled(
                "to view balance",
                Style::default().fg(Color::Gray),
            )]),
        ]
    } else {
        let default_balance = "0.0".to_string();
        let balance = app
            .state
            .balances
            .get(from_token)
            .unwrap_or(&default_balance);

        vec![
            Line::from(vec![Span::styled(
                "Available:",
                Style::default().fg(Color::Gray),
            )]),
            Line::from(vec![Span::styled(
                format!("{} {}", balance, from_token),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]),
        ]
    };

    let balance_paragraph = Paragraph::new(Text::from(balance_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Balance")
                .border_style(Style::default().fg(Color::Green)),
        )
        .alignment(Alignment::Center);

    f.render_widget(balance_paragraph, chunks[1]);
}

/// Render slippage tolerance input
fn render_slippage_input(f: &mut Frame, area: Rect, _app: &App, swap_state: &mut SwapScreenState) {
    swap_state.slippage_input.render(f, area);
}

/// Render execute button
fn render_execute_button(f: &mut Frame, area: Rect, app: &App) {
    let swap_state = get_swap_screen_state();
    let is_focused = matches!(swap_state.input_focus, SwapInputFocus::Execute);
    let is_valid = swap_state.clone().validate();

    let (button_style, button_text) =
        if matches!(app.state.loading_state, LoadingState::Loading { .. }) {
            (Style::default().fg(Color::Yellow), "Processing Swap...")
        } else if !is_valid {
            (Style::default().fg(Color::DarkGray), "Invalid Input")
        } else if is_focused {
            (
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                "► Swap ◄",
            )
        } else {
            (
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                "Swap",
            )
        };

    let button = Paragraph::new(button_text)
        .style(button_style)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if is_focused {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Gray)
                }),
        );

    f.render_widget(button, area);
}

/// Render swap preview panel
fn _render_swap_preview(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Swap Preview")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let swap_state = get_swap_screen_state();
    let from_amount = swap_state.from_amount_input.value();
    let from_token = swap_state
        .from_token_dropdown
        .get_selected_label()
        .unwrap_or("Select Token");
    let pool_info = swap_state
        .pool_dropdown
        .get_selected_label()
        .unwrap_or("Select Pool");
    let slippage = swap_state.slippage_input.value();

    let content =
        if from_amount.is_empty() || from_token == "Select Token" || pool_info == "Select Pool" {
            vec![Line::from(vec![Span::styled(
                "Complete all fields to see preview",
                Style::default().fg(Color::Gray),
            )])]
        } else {
            // Determine the "to token" from the selected pool
            let to_token = determine_to_token_from_pool(&pool_info, &from_token);
            let estimated_output = _calculate_estimated_output(from_amount, &app.state.swap_state);
            let price_impact = _calculate_price_impact(from_amount, &app.state.swap_state);

            vec![
                Line::from(vec![
                    Span::styled("From: ", Style::default().fg(Color::White)),
                    Span::styled(
                        format!("{} {}", from_amount, from_token),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("To: ", Style::default().fg(Color::White)),
                    Span::styled(
                        format!("≈ {} {}", estimated_output, to_token),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Pool: ", Style::default().fg(Color::White)),
                    Span::styled(pool_info, Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Slippage Tolerance: ", Style::default().fg(Color::White)),
                    Span::styled(format!("{}%", slippage), Style::default().fg(Color::Yellow)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Price Impact: ", Style::default().fg(Color::White)),
                    Span::styled(
                        format!("{:.2}%", price_impact),
                        if price_impact > 5.0 {
                            Style::default().fg(Color::Red)
                        } else if price_impact > 1.0 {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::Green)
                        },
                    ),
                ]),
            ]
        };

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Determine the "to token" from the selected pool and from token
pub fn determine_to_token_from_pool(pool_info: &str, from_token: &str) -> String {
    // Extract the asset pair from pool display name (format: "Pool X: TokenA / TokenB")
    if let Some(pair_part) = pool_info.split(": ").nth(1) {
        let tokens: Vec<&str> = pair_part.split(" / ").collect();
        if tokens.len() == 2 {
            // Return the token that's not the from_token
            if tokens[0] == from_token {
                return tokens[1].to_string();
            } else if tokens[1] == from_token {
                return tokens[0].to_string();
            }
        }
    }
    "Unknown".to_string()
}

/// Render simulation results panel
fn render_simulation_results(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Simulation Results")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::uniform(1));

    let swap_state = get_swap_screen_state();

    let content = if let Some(ref simulation) = app.state.swap_state.simulation_result {
        render_simulation_details(simulation)
    } else if matches!(app.state.loading_state, LoadingState::Loading { .. }) {
        vec![Line::from(vec![Span::styled(
            "Running simulation...",
            Style::default().fg(Color::Yellow),
        )])]
    } else if swap_state.should_trigger_simulation() {
        vec![
            Line::from(vec![Span::styled(
                "⏳ Auto-simulation will run soon...",
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Form completed - simulation triggered after 5s of inactivity",
                Style::default().fg(Color::DarkGray),
            )]),
        ]
    } else if swap_state.last_input_change.is_some() {
        let elapsed = swap_state.last_input_change.unwrap().elapsed().as_secs();
        let remaining = 5_u64.saturating_sub(elapsed);
        vec![
            Line::from(vec![Span::styled(
                format!("⏱️  Simulation in {}s...", remaining),
                Style::default().fg(Color::Yellow),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Waiting for form completion and input inactivity",
                Style::default().fg(Color::DarkGray),
            )]),
        ]
    } else {
        vec![
            Line::from(vec![Span::styled(
                "Simulation will appear here",
                Style::default().fg(Color::Gray),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Complete the swap form to run simulation",
                Style::default().fg(Color::DarkGray),
            )]),
        ]
    };

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render detailed simulation results
fn render_simulation_details(simulation: &SimulationResponse) -> Vec<Line> {
    vec![
        Line::from(vec![
            Span::styled("Expected Output: ", Style::default().fg(Color::White)),
            Span::styled(
                format!("{}", simulation.return_amount),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Fee Breakdown:",
            Style::default().fg(Color::White),
        )]),
        Line::from(vec![
            Span::styled("  • Swap Fee: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", simulation.swap_fee_amount),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("  • Protocol Fee: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", simulation.protocol_fee_amount),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::styled("  • Burn Fee: ", Style::default().fg(Color::Gray)),
            Span::styled(
                format!("{}", simulation.burn_fee_amount),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Total Fees: ", Style::default().fg(Color::White)),
            Span::styled(
                format!(
                    "{}",
                    simulation.swap_fee_amount
                        + simulation.protocol_fee_amount
                        + simulation.burn_fee_amount
                ),
                Style::default().fg(Color::Red),
            ),
        ]),
    ]
}

/// Calculate estimated output based on simulation or basic logic
fn _calculate_estimated_output(amount: &str, _swap_state: &SwapState) -> String {
    if let Ok(amount_val) = amount.parse::<f64>() {
        // Placeholder: 1-to-1 swap with 0.3% fee
        let fee = amount_val * 0.003;
        format!("{:.4}", amount_val - fee)
    } else {
        "0.0".to_string()
    }
}

/// Calculate price impact based on simulation or basic logic
fn _calculate_price_impact(amount: &str, _swap_state: &SwapState) -> f64 {
    // Placeholder: Price impact increases with amount
    if let Ok(amount_val) = amount.parse::<f64>() {
        (amount_val / 1000.0).min(10.0) // Up to 10% impact
    } else {
        0.0
    }
}

/// Handle input for the swap screen (delegated from app)
pub fn handle_swap_screen_input(input: InputRequest) -> bool {
    let swap_state = get_swap_screen_state();
    swap_state.handle_input(input)
}

/// Execute swap with confirmation
pub fn execute_swap_with_confirmation() {
    let swap_state = get_swap_screen_state();

    let from_token = swap_state
        .from_token_dropdown
        .get_selected_label()
        .unwrap_or("Unknown")
        .to_string();

    let pool_info = swap_state
        .pool_dropdown
        .get_selected_label()
        .unwrap_or("Unknown Pool")
        .to_string();

    let to_token = determine_to_token_from_pool(&pool_info, &from_token);

    // Create swap details for confirmation
    let swap_details = SwapDetails {
        from_amount: swap_state.from_amount_input.value().to_string(),
        from_token,
        to_amount: "0.0".to_string(), // Would calculate
        to_token,
        pool_name: pool_info,
        slippage: swap_state.slippage_input.value().to_string(),
        expected_output: "0.0".to_string(), // Would calculate
        price_impact: 1.5,                  // Would calculate
        fee_amount: "0.001".to_string(),    // Would calculate
    };

    swap_state.show_confirmation_modal(&swap_details);
}

/// Handle confirmation modal response
pub fn handle_confirmation_response(confirmed: bool) -> bool {
    let swap_state = get_swap_screen_state();
    if confirmed {
        // Execute the actual swap
        swap_state.hide_confirmation_modal();
        true // Return true to indicate swap should be executed
    } else {
        swap_state.hide_confirmation_modal();
        false
    }
}

/// Reset swap form
pub fn reset_swap_form() {
    let swap_state = get_swap_screen_state();
    *swap_state = SwapScreenState::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swap_screen_state_navigation() {
        let mut state = SwapScreenState::default();
        assert_eq!(state.input_focus, SwapInputFocus::Pool);

        state.next_focus();
        assert_eq!(state.input_focus, SwapInputFocus::FromToken);

        state.next_focus();
        assert_eq!(state.input_focus, SwapInputFocus::FromAmount);
    }

    #[test]
    fn test_swap_validation() {
        let mut state = SwapScreenState::default();

        // Empty inputs should be invalid
        assert!(!state.validate());

        // Set valid inputs
        state
            .from_amount_input
            .handle_input(InputRequest::InsertChar('1'));
        state
            .from_amount_input
            .handle_input(InputRequest::InsertChar('0'));
        // Would need to set dropdown selections in real test

        // At least amount should now be valid
        state.from_amount_input.validate();
        assert!(state.from_amount_input.is_valid());
    }

    #[test]
    fn test_calculate_estimated_output() {
        let swap_state = SwapState::default();
        let result = _calculate_estimated_output("100.0", &swap_state);
        assert_eq!(result, "99.9700");
    }

    #[test]
    fn test_calculate_price_impact() {
        let swap_state = SwapState::default();
        assert_eq!(_calculate_price_impact("50.0", &swap_state), 0.05);
        assert_eq!(_calculate_price_impact("500.0", &swap_state), 0.5);
        assert_eq!(_calculate_price_impact("5000.0", &swap_state), 5.0);
    }
}
