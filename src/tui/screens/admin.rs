//! Admin Screen Implementation
//!
//! This module provides the admin view for the MANTRA DEX SDK TUI,
//! allowing pool creation, feature management, and administrative operations.

use crate::tui::{
    app::{App, LoadingState},
    components::{
        header::render_header, navigation::render_navigation, status_bar::render_status_bar,
    },
};
use cosmwasm_std::{Decimal, Uint128};
use mantra_dex_std::{fee::PoolFee, pool_manager::PoolType};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Gauge, Padding, Paragraph, Row, Table, Wrap},
    Frame,
};

/// Admin screen operational modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminMode {
    PoolManagement,
    PoolCreation,
    FeatureControls,
}

/// Pool creation wizard state
#[derive(Debug, Clone)]
pub struct PoolCreationState {
    pub step: PoolCreationStep,
    pub asset_denoms: Vec<String>,
    pub asset_decimals: Vec<u8>,
    pub protocol_fee: String,
    pub swap_fee: String,
    pub burn_fee: String,
    pub pool_type: PoolType,
    pub pool_identifier: String,
    pub current_input: String,
    pub current_asset_index: usize,
    pub validation_errors: Vec<String>,
}

/// Pool creation wizard steps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolCreationStep {
    AssetSelection,
    DecimalsConfiguration,
    FeeStructure,
    PoolTypeSelection,
    Confirmation,
}

impl Default for PoolCreationState {
    fn default() -> Self {
        Self {
            step: PoolCreationStep::AssetSelection,
            asset_denoms: Vec::new(),
            asset_decimals: Vec::new(),
            protocol_fee: "0.001".to_string(),
            swap_fee: "0.003".to_string(),
            burn_fee: "0.0".to_string(),
            pool_type: PoolType::ConstantProduct,
            pool_identifier: String::new(),
            current_input: String::new(),
            current_asset_index: 0,
            validation_errors: Vec::new(),
        }
    }
}

/// Pool feature control state
#[derive(Debug, Clone)]
pub struct PoolFeatureState {
    pub selected_pool_id: String,
    pub withdrawals_enabled: bool,
    pub deposits_enabled: bool,
    pub swaps_enabled: bool,
    pub input_mode: FeatureInputMode,
}

/// Feature control input modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureInputMode {
    PoolSelection,
    FeatureToggles,
}

impl Default for PoolFeatureState {
    fn default() -> Self {
        Self {
            selected_pool_id: String::new(),
            withdrawals_enabled: true,
            deposits_enabled: true,
            swaps_enabled: true,
            input_mode: FeatureInputMode::PoolSelection,
        }
    }
}

/// Admin screen state management
#[derive(Debug, Clone)]
pub struct AdminState {
    pub mode: AdminMode,
    pub pool_creation: PoolCreationState,
    pub feature_control: PoolFeatureState,
    pub selected_tab: usize,
}

impl Default for AdminState {
    fn default() -> Self {
        Self {
            mode: AdminMode::PoolManagement,
            pool_creation: PoolCreationState::default(),
            feature_control: PoolFeatureState::default(),
            selected_tab: 0,
        }
    }
}

/// Render the complete admin screen
pub fn render_admin(f: &mut Frame, app: &App) {
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

    // Render admin content
    render_admin_content(f, chunks[2], app);

    // Render status bar
    render_status_bar(f, &app.state, chunks[3]);
}

/// Render the main admin content area
fn render_admin_content(f: &mut Frame, area: Rect, app: &App) {
    // Create vertical layout: tabs + content
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Render admin tabs
    render_admin_tabs(f, main_chunks[0]);

    // Render content based on current mode
    match AdminMode::PoolManagement {
        AdminMode::PoolManagement => render_pool_management(f, main_chunks[1], app),
        AdminMode::PoolCreation => render_pool_creation_wizard(f, main_chunks[1]),
        AdminMode::FeatureControls => render_feature_controls(f, main_chunks[1], app),
    }
}

/// Render admin mode tabs
fn render_admin_tabs(f: &mut Frame, area: Rect) {
    let tabs = vec!["Pool Management", "Create Pool", "Feature Controls"];
    let selected_tab = 0; // This would come from app state in a full implementation

    let tab_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    for (i, (tab_name, chunk)) in tabs.iter().zip(tab_chunks.iter()).enumerate() {
        let style = if i == selected_tab {
            Style::default().bg(Color::Cyan).fg(Color::Black)
        } else {
            Style::default().fg(Color::White)
        };

        let tab = Paragraph::new(*tab_name)
            .style(style)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(tab, *chunk);
    }
}

/// Render pool management interface
fn render_pool_management(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left: Pool list with admin actions
    render_admin_pool_list(f, chunks[0], app);

    // Right: Pool admin actions
    render_pool_admin_actions(f, chunks[1], app);
}

/// Render pool list with admin controls
fn render_admin_pool_list(f: &mut Frame, area: Rect, app: &App) {
    let pool_data: Vec<Vec<String>> = app
        .state
        .pool_cache
        .values()
        .map(|entry| {
            let pool = &entry.pool_info;
            vec![
                pool.pool_info.pool_identifier.to_string(),
                format!("{} assets", pool.pool_info.assets.len()),
                format!(
                    "W:{} D:{} S:{}",
                    if pool.pool_info.status.withdrawals_enabled {
                        "✓"
                    } else {
                        "✗"
                    },
                    if pool.pool_info.status.deposits_enabled {
                        "✓"
                    } else {
                        "✗"
                    },
                    if pool.pool_info.status.swaps_enabled {
                        "✓"
                    } else {
                        "✗"
                    }
                ),
            ]
        })
        .collect();

    let header = Row::new(vec![
        Cell::from("Pool ID").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Assets").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Features").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let rows: Vec<Row> = pool_data
        .iter()
        .map(|pool| {
            Row::new(
                pool.iter()
                    .map(|cell| Cell::from(cell.as_str()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Length(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("Pools - Admin View")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue)),
    )
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    f.render_widget(table, area);
}

/// Render pool admin actions panel
fn render_pool_admin_actions(f: &mut Frame, area: Rect, _app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Pool selection
            Constraint::Length(10), // Feature toggles
            Constraint::Min(0),     // Actions
        ])
        .split(area);

    // Pool selection
    render_pool_selection_input(f, chunks[0]);

    // Feature toggles
    render_feature_toggles(f, chunks[1]);

    // Admin actions
    render_admin_actions(f, chunks[2]);
}

/// Render pool selection input
fn render_pool_selection_input(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Pool Selection")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![
            Span::styled("Pool ID: ", Style::default().fg(Color::Yellow)),
            Span::styled("[Enter pool ID]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Instructions:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("• Enter pool ID to manage"),
        Line::from("• Press Tab to navigate"),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render feature toggle controls
fn render_feature_toggles(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Feature Controls")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![
            Span::styled("• Withdrawals: ", Style::default().fg(Color::White)),
            Span::styled("ENABLED", Style::default().fg(Color::Green)),
            Span::styled(" [W]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("• Deposits: ", Style::default().fg(Color::White)),
            Span::styled("ENABLED", Style::default().fg(Color::Green)),
            Span::styled(" [D]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("• Swaps: ", Style::default().fg(Color::White)),
            Span::styled("ENABLED", Style::default().fg(Color::Green)),
            Span::styled(" [S]", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press key to toggle feature",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render admin action buttons
fn render_admin_actions(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Admin Actions")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Available Actions:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• [Enter] ", Style::default().fg(Color::Green)),
            Span::styled("Apply Changes", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("• [R] ", Style::default().fg(Color::Blue)),
            Span::styled("Reset Features", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("• [A] ", Style::default().fg(Color::Cyan)),
            Span::styled("Enable All", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("• [X] ", Style::default().fg(Color::Red)),
            Span::styled("Disable All", Style::default().fg(Color::White)),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render pool creation wizard
fn render_pool_creation_wizard(f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Progress indicator
            Constraint::Min(0),    // Current step content
            Constraint::Length(5), // Navigation help
        ])
        .split(area);

    // Progress indicator
    render_creation_progress(f, chunks[0]);

    // Current step content
    render_creation_step_content(f, chunks[1]);

    // Navigation help
    render_creation_navigation_help(f, chunks[2]);
}

/// Render pool creation progress indicator
fn render_creation_progress(f: &mut Frame, area: Rect) {
    let current_step = 1; // This would come from state
    let total_steps = 5;
    let progress = (current_step as f64 / total_steps as f64) * 100.0;

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(format!(
                    "Pool Creation Progress (Step {} of {})",
                    current_step, total_steps
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .percent(progress as u16);

    f.render_widget(gauge, area);
}

/// Render current creation step content
fn render_creation_step_content(f: &mut Frame, area: Rect) {
    // For now, render asset selection step
    render_asset_selection_step(f, area);
}

/// Render asset selection step
fn render_asset_selection_step(f: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Asset input
    render_asset_input(f, chunks[0]);

    // Right: Asset list
    render_asset_list(f, chunks[1]);
}

/// Render asset input form
fn render_asset_input(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Add Assets")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Asset Denom:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(vec![Span::styled(
            "[Enter asset denomination]",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Decimals:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(vec![Span::styled(
            "[Enter decimal places]",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press Enter to add asset",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render current asset list
fn render_asset_list(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Pool Assets")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Assets (min 2 required):",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• ", Style::default().fg(Color::White)),
            Span::styled("No assets added yet", Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Pool creation requires:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from("  - At least 2 assets"),
        Line::from("  - Valid decimal configuration"),
        Line::from("  - Fee structure (max 20% total)"),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render creation navigation help
fn render_creation_navigation_help(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Navigation")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![
            Span::styled("• [Tab] ", Style::default().fg(Color::Green)),
            Span::styled("Next field  ", Style::default().fg(Color::White)),
            Span::styled("• [Shift+Tab] ", Style::default().fg(Color::Green)),
            Span::styled("Previous field", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("• [Enter] ", Style::default().fg(Color::Cyan)),
            Span::styled("Next step  ", Style::default().fg(Color::White)),
            Span::styled("• [Esc] ", Style::default().fg(Color::Red)),
            Span::styled("Cancel creation", Style::default().fg(Color::White)),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render feature controls interface
fn render_feature_controls(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Global controls
            Constraint::Min(0),    // Pool-specific controls
        ])
        .split(area);

    // Global feature controls
    render_global_feature_controls(f, chunks[0]);

    // Pool-specific controls
    render_pool_specific_controls(f, chunks[1], app);
}

/// Render global feature controls
fn render_global_feature_controls(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Global Feature Updates")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "⚠️  Global Operations:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Note: v3.0.0 requires per-pool targeting"),
        Line::from("Global operations apply to specified pools only"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Use Pool Management tab for individual pool control",
            Style::default().fg(Color::Cyan),
        )]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render pool-specific controls
fn render_pool_specific_controls(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: Pool selection and features
    render_feature_pool_selection(f, chunks[0]);

    // Right: Bulk operations
    render_bulk_operations(f, chunks[1], app);
}

/// Render feature pool selection
fn render_feature_pool_selection(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Pool Feature Management")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Target Pool ID:",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(vec![Span::styled(
            "[Enter pool ID to modify]",
            Style::default().fg(Color::Gray),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Available Operations:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from("• Toggle individual features"),
        Line::from("• Enable/disable all operations"),
        Line::from("• View current feature status"),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

/// Render bulk operations panel
fn render_bulk_operations(f: &mut Frame, area: Rect, _app: &App) {
    let block = Block::default()
        .title("Bulk Operations")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .padding(Padding::uniform(1));

    let content = vec![
        Line::from(vec![Span::styled(
            "Multi-Pool Operations:",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("• [1] ", Style::default().fg(Color::Green)),
            Span::styled(
                "Enable all features for pool",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("• [2] ", Style::default().fg(Color::Red)),
            Span::styled(
                "Disable all features for pool",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Warning:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Changes are immediate!", Style::default().fg(Color::White)),
        ]),
    ];

    let paragraph = Paragraph::new(Text::from(content))
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_state_default() {
        let state = AdminState::default();
        assert_eq!(state.mode, AdminMode::PoolManagement);
        assert_eq!(state.selected_tab, 0);
    }

    #[test]
    fn test_pool_creation_state_default() {
        let state = PoolCreationState::default();
        assert_eq!(state.step, PoolCreationStep::AssetSelection);
        assert!(state.asset_denoms.is_empty());
        assert_eq!(state.protocol_fee, "0.001");
        assert_eq!(state.swap_fee, "0.003");
        assert_eq!(state.burn_fee, "0.0");
    }

    #[test]
    fn test_pool_feature_state_default() {
        let state = PoolFeatureState::default();
        assert!(state.withdrawals_enabled);
        assert!(state.deposits_enabled);
        assert!(state.swaps_enabled);
        assert_eq!(state.input_mode, FeatureInputMode::PoolSelection);
    }
}
