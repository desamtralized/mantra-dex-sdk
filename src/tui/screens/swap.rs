//! Swap Screen Implementation
//!
//! This module provides the swap interface for the MANTRA DEX SDK TUI,
//! allowing users to perform token swaps with price impact calculations,
//! slippage settings, and transaction execution.

use crate::tui::{
    app::{App, LoadingState, SwapState},
    components::{
        forms::{Dropdown, DropdownOption, InputType, TextInput},
        header::render_header,
        modals::{render_modal, ModalState},
        navigation::render_navigation,
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
    FromAmount,
    ToToken,
    Pool,
    Slippage,
    Execute,
}

/// Current swap screen state
#[derive(Debug, Clone)]
pub struct SwapScreenState {
    /// Current input focus
    pub input_focus: SwapInputFocus,
    /// From amount input
    pub from_amount_input: TextInput,
    /// To token dropdown
    pub to_token_dropdown: Dropdown<String>,
    /// Pool selection dropdown
    pub pool_dropdown: Dropdown<String>,
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
}

impl Default for SwapScreenState {
    fn default() -> Self {
        let mut from_amount_input = TextInput::new("From Amount")
            .with_type(InputType::Amount)
            .required()
            .with_placeholder("0.0");

        let to_token_dropdown = Dropdown::new("To Token").required();

        let pool_dropdown = Dropdown::new("Select Pool").required();

        let slippage_input = TextInput::new("Slippage Tolerance (%)")
            .with_type(InputType::Amount)
            .with_value("1.0")
            .with_placeholder("1.0");

        // Set initial focus
        from_amount_input.set_focused(true);

        Self {
            input_focus: SwapInputFocus::FromAmount,
            from_amount_input,
            to_token_dropdown,
            pool_dropdown,
            slippage_input,
            show_confirmation: false,
            modal_state: None,
            available_tokens: vec![
                "USDC".to_string(),
                "USDT".to_string(),
                "ATOM".to_string(),
                "OSMO".to_string(),
                "MANTRA".to_string(),
            ],
            available_pools: Vec::new(),
        }
    }
}

impl SwapScreenState {
    /// Initialize dropdowns with available tokens
    pub fn initialize_tokens(&mut self, tokens: Vec<String>) {
        self.available_tokens = tokens.clone();

        // Update to token dropdown
        let mut dropdown = Dropdown::new("To Token").required();
        for token in &tokens {
            dropdown = dropdown.add_option(DropdownOption::new(token.clone(), token.clone()));
        }
        self.to_token_dropdown = dropdown;
    }

    /// Update available pools based on selected tokens
    pub fn update_available_pools(&mut self, pools: Vec<(String, String)>) {
        self.available_pools = pools.clone();

        // Update pool dropdown
        let mut dropdown = Dropdown::new("Select Pool").required();
        for (pool_id, display_name) in &pools {
            dropdown =
                dropdown.add_option(DropdownOption::new(display_name.clone(), pool_id.clone()));
        }
        self.pool_dropdown = dropdown;
    }

    /// Move focus to next input
    pub fn next_focus(&mut self) {
        self.clear_focus();
        self.input_focus = match self.input_focus {
            SwapInputFocus::FromAmount => SwapInputFocus::ToToken,
            SwapInputFocus::ToToken => SwapInputFocus::Pool,
            SwapInputFocus::Pool => SwapInputFocus::Slippage,
            SwapInputFocus::Slippage => SwapInputFocus::Execute,
            SwapInputFocus::Execute => SwapInputFocus::FromAmount,
        };
        self.set_focus();
    }

    /// Move focus to previous input
    pub fn previous_focus(&mut self) {
        self.clear_focus();
        self.input_focus = match self.input_focus {
            SwapInputFocus::FromAmount => SwapInputFocus::Execute,
            SwapInputFocus::ToToken => SwapInputFocus::FromAmount,
            SwapInputFocus::Pool => SwapInputFocus::ToToken,
            SwapInputFocus::Slippage => SwapInputFocus::Pool,
            SwapInputFocus::Execute => SwapInputFocus::Slippage,
        };
        self.set_focus();
    }

    /// Clear focus from all inputs
    fn clear_focus(&mut self) {
        self.from_amount_input.set_focused(false);
        self.to_token_dropdown.set_focused(false);
        self.pool_dropdown.set_focused(false);
        self.slippage_input.set_focused(false);
    }

    /// Set focus on current input
    fn set_focus(&mut self) {
        match self.input_focus {
            SwapInputFocus::FromAmount => self.from_amount_input.set_focused(true),
            SwapInputFocus::ToToken => self.to_token_dropdown.set_focused(true),
            SwapInputFocus::Pool => self.pool_dropdown.set_focused(true),
            SwapInputFocus::Slippage => self.slippage_input.set_focused(true),
            SwapInputFocus::Execute => {} // Button focus handled separately
        }
    }

    /// Handle keyboard input
    pub fn handle_input(&mut self, input: InputRequest) -> bool {
        match self.input_focus {
            SwapInputFocus::FromAmount => {
                self.from_amount_input.handle_input(input);
                true
            }
            SwapInputFocus::ToToken => match input {
                InputRequest::GoToPrevWord => {
                    self.to_token_dropdown.move_up();
                    true
                }
                InputRequest::GoToNextWord => {
                    self.to_token_dropdown.move_down();
                    true
                }
                InputRequest::GoToStart => {
                    self.to_token_dropdown.toggle();
                    true
                }
                InputRequest::GoToEnd => {
                    self.to_token_dropdown.select_current();
                    true
                }
                _ => false,
            },
            SwapInputFocus::Pool => match input {
                InputRequest::GoToPrevWord => {
                    self.pool_dropdown.move_up();
                    true
                }
                InputRequest::GoToNextWord => {
                    self.pool_dropdown.move_down();
                    true
                }
                InputRequest::GoToStart => {
                    self.pool_dropdown.toggle();
                    true
                }
                InputRequest::GoToEnd => {
                    self.pool_dropdown.select_current();
                    true
                }
                _ => false,
            },
            SwapInputFocus::Slippage => {
                self.slippage_input.handle_input(input);
                true
            }
            SwapInputFocus::Execute => false,
        }
    }

    /// Validate all inputs
    pub fn validate(&mut self) -> bool {
        let amount_valid = self.from_amount_input.validate();
        let to_token_valid = self.to_token_dropdown.selected_value().is_some();
        let pool_valid = self.pool_dropdown.selected_value().is_some();
        let slippage_valid = self.slippage_input.validate();

        amount_valid && to_token_valid && pool_valid && slippage_valid
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
fn get_swap_screen_state() -> &'static mut SwapScreenState {
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

    // Render swap content
    render_swap_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);

    // Render modal if visible
    let swap_state = get_swap_screen_state();
    if let Some(ref modal_state) = swap_state.modal_state {
        render_modal(f, modal_state, size);
    }
}

/// Render the main swap content area
fn render_swap_content(f: &mut Frame, area: Rect, app: &App) {
    // Create a horizontal layout: swap interface | swap preview & simulation
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Split the left side: swap interface on top, execute button on bottom
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(main_chunks[0]);

    // Split the right side: preview on top, simulation results on bottom
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    // Render components
    render_swap_interface(f, left_chunks[0], app);
    render_execute_button(f, left_chunks[1], app);
    render_swap_preview(f, right_chunks[0], app);
    render_simulation_results(f, right_chunks[1], app);
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
            Constraint::Length(4), // From amount + balance
            Constraint::Length(4), // To token selection
            Constraint::Length(4), // Pool selection
            Constraint::Length(4), // Slippage tolerance
        ])
        .split(block.inner(area));

    let swap_state = get_swap_screen_state();

    // Render form inputs
    render_from_amount_input(f, input_chunks[0], app, swap_state);
    render_to_token_input(f, input_chunks[1], app, swap_state);
    render_pool_selection(f, input_chunks[2], app, swap_state);
    render_slippage_input(f, input_chunks[3], app, swap_state);

    f.render_widget(block, area);
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
    let default_token = "USDC".to_string();
    let from_token = app
        .state
        .swap_state
        .from_asset
        .as_ref()
        .unwrap_or(&default_token);
    let default_balance = "0.0".to_string();
    let balance = app
        .state
        .balances
        .get(from_token)
        .unwrap_or(&default_balance);

    let balance_text = vec![
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
    ];

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

/// Render to token selection dropdown
fn render_to_token_input(f: &mut Frame, area: Rect, _app: &App, swap_state: &mut SwapScreenState) {
    swap_state.to_token_dropdown.render(f, area);
}

/// Render pool selection dropdown
fn render_pool_selection(f: &mut Frame, area: Rect, _app: &App, swap_state: &mut SwapScreenState) {
    swap_state.pool_dropdown.render(f, area);
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
                "► Execute Swap ◄",
            )
        } else {
            (
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                "Execute Swap",
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
fn render_swap_preview(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Swap Preview")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let swap_state = get_swap_screen_state();
    let from_amount = swap_state.from_amount_input.value();
    let to_token = swap_state
        .to_token_dropdown
        .selected_text()
        .unwrap_or("Select Token");
    let slippage = swap_state.slippage_input.value();

    let content = if from_amount.is_empty() || to_token == "Select Token" {
        vec![Line::from(vec![Span::styled(
            "Enter swap details to see preview",
            Style::default().fg(Color::Gray),
        )])]
    } else {
        let estimated_output = calculate_estimated_output(from_amount, &app.state.swap_state);
        let price_impact = calculate_price_impact(from_amount, &app.state.swap_state);

        vec![
            Line::from(vec![
                Span::styled("From: ", Style::default().fg(Color::White)),
                Span::styled(
                    format!(
                        "{} {}",
                        from_amount,
                        app.state
                            .swap_state
                            .from_asset
                            .as_ref()
                            .unwrap_or(&"USDC".to_string())
                    ),
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

/// Render simulation results panel
fn render_simulation_results(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Simulation Results")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::uniform(1));

    let content = if let Some(ref simulation) = app.state.swap_state.simulation_result {
        render_simulation_details(simulation)
    } else if matches!(app.state.loading_state, LoadingState::Loading { .. }) {
        vec![Line::from(vec![Span::styled(
            "Running simulation...",
            Style::default().fg(Color::Yellow),
        )])]
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

/// Calculate estimated output for preview (placeholder implementation)
fn calculate_estimated_output(amount: &str, _swap_state: &SwapState) -> String {
    if let Ok(amount_val) = amount.parse::<f64>() {
        // Simple 1:1 ratio for demo - real implementation would use pool data
        format!("{:.6}", amount_val * 0.99) // Assume small slippage
    } else {
        "0.0".to_string()
    }
}

/// Calculate price impact (placeholder implementation)
fn calculate_price_impact(amount: &str, _swap_state: &SwapState) -> f64 {
    if let Ok(amount_val) = amount.parse::<f64>() {
        // Simple calculation for demo - real implementation would use pool data
        if amount_val > 1000.0 {
            5.0 // High impact for large trades
        } else if amount_val > 100.0 {
            1.5 // Medium impact
        } else {
            0.1 // Low impact
        }
    } else {
        0.0
    }
}

/// Handle swap screen input events
pub fn handle_swap_screen_input(input: InputRequest) -> bool {
    let swap_state = get_swap_screen_state();
    swap_state.handle_input(input)
}

/// Handle swap screen navigation
pub fn handle_swap_screen_navigation(next: bool) {
    let swap_state = get_swap_screen_state();
    if next {
        swap_state.next_focus();
    } else {
        swap_state.previous_focus();
    }
}

/// Execute swap with confirmation
pub fn execute_swap_with_confirmation() {
    let swap_state = get_swap_screen_state();

    // Create swap details for confirmation
    let swap_details = SwapDetails {
        from_amount: swap_state.from_amount_input.value().to_string(),
        from_token: "USDC".to_string(), // Would get from app state
        to_amount: "0.0".to_string(),   // Would calculate
        to_token: swap_state
            .to_token_dropdown
            .selected_text()
            .unwrap_or("Unknown")
            .to_string(),
        pool_name: swap_state
            .pool_dropdown
            .selected_text()
            .unwrap_or("Unknown Pool")
            .to_string(),
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
        assert_eq!(state.input_focus, SwapInputFocus::FromAmount);

        state.next_focus();
        assert_eq!(state.input_focus, SwapInputFocus::ToToken);

        state.next_focus();
        assert_eq!(state.input_focus, SwapInputFocus::Pool);
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
        let result = calculate_estimated_output("100.0", &swap_state);
        assert_eq!(result, "99.000000");
    }

    #[test]
    fn test_calculate_price_impact() {
        let swap_state = SwapState::default();
        assert_eq!(calculate_price_impact("50.0", &swap_state), 0.1);
        assert_eq!(calculate_price_impact("500.0", &swap_state), 1.5);
        assert_eq!(calculate_price_impact("5000.0", &swap_state), 5.0);
    }
}
