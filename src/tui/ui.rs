//! UI Rendering Logic
//!
//! This module coordinates the rendering of all UI components and handles
//! the main UI layout and screen rendering.

#[cfg(feature = "tui")]
use crate::tui::app::App;
#[cfg(feature = "tui")]
use crate::tui::screens::dashboard::render_dashboard;
#[cfg(feature = "tui")]
use crate::tui::screens::liquidity::render_liquidity;
#[cfg(feature = "tui")]
use crate::tui::screens::pools::render_pools;
#[cfg(feature = "tui")]
use crate::tui::screens::swap::render_swap;
#[cfg(feature = "tui")]
use crate::Error;
#[cfg(feature = "tui")]
use ratatui::{prelude::*, widgets::*};

/// Main UI rendering function
pub fn render_ui(frame: &mut Frame, app: &mut App) -> Result<(), Error> {
    let size = frame.area();

    // Create main layout with header, body, and status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Body (main content)
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // Render header
    render_header(frame, chunks[0], app);

    // Render main content based on current screen
    render_main_content(frame, chunks[1], app)?;

    // Render status bar
    render_status_bar(frame, chunks[2], app);

    Ok(())
}

/// Render the header with navigation and basic info
fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(30), // Logo/Title
            Constraint::Min(0),     // Navigation tabs
            Constraint::Length(25), // Wallet/Network info
        ])
        .split(area);

    // Title/Logo
    let title = Paragraph::new("MANTRA DEX SDK")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, header_chunks[0]);

    // Navigation tabs
    let tabs: Vec<Line> = crate::tui::app::Screen::all()
        .iter()
        .enumerate()
        .map(|(i, screen)| {
            let style = if i == app.state.current_tab {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(screen.display_name(), style))
        })
        .collect();

    let tabs_widget = Tabs::new(tabs)
        .block(Block::default().borders(Borders::ALL).title("Navigation"))
        .select(app.state.current_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow));
    frame.render_widget(tabs_widget, header_chunks[1]);

    // Wallet/Network info
    let wallet_info = if let Some(address) = &app.state.wallet_address {
        format!("Wallet: {}", &address[..8])
    } else {
        "No Wallet".to_string()
    };

    let block_info = if let Some(height) = app.state.block_height {
        format!("Block: {}", height)
    } else {
        "Block: -".to_string()
    };

    let info_text = format!("{}\n{}", wallet_info, block_info);
    let info = Paragraph::new(info_text)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Network"));
    frame.render_widget(info, header_chunks[2]);
}

/// Render the main content area based on current screen
fn render_main_content(frame: &mut Frame, area: Rect, app: &App) -> Result<(), Error> {
    match app.state.current_screen {
        crate::tui::app::Screen::Dashboard => {
            render_dashboard(frame, app);
        }
        crate::tui::app::Screen::Pools => {
            render_pools(frame, app);
        }
        crate::tui::app::Screen::Swap => {
            render_swap(frame, app);
        }
        crate::tui::app::Screen::MultiHop => {
            render_multihop_placeholder(frame, area, app);
        }
        crate::tui::app::Screen::Liquidity => {
            render_liquidity(frame, app);
        }
        crate::tui::app::Screen::Rewards => {
            render_rewards_placeholder(frame, area, app);
        }
        crate::tui::app::Screen::Admin => {
            render_admin_placeholder(frame, area, app);
        }
        crate::tui::app::Screen::Settings => {
            render_settings_placeholder(frame, area, app);
        }
        crate::tui::app::Screen::TransactionDetails => {
            render_transaction_placeholder(frame, area, app);
        }
    }
    Ok(())
}

/// Render the status bar at the bottom
fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),     // Status message
            Constraint::Length(40), // Loading state
        ])
        .split(area);

    // Status message
    let status_text = if let Some(error) = &app.state.error_message {
        format!("Error: {}", error)
    } else if let Some(status) = &app.state.status_message {
        status.clone()
    } else {
        "Ready".to_string()
    };

    let status_color = if app.state.error_message.is_some() {
        Color::Red
    } else if app.state.status_message.is_some() {
        Color::Green
    } else {
        Color::Blue
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(status_color))
        .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(status, status_chunks[0]);

    // Loading state
    let loading_text = match &app.state.loading_state {
        crate::tui::app::LoadingState::Idle => "Idle",
        crate::tui::app::LoadingState::Loading(msg) => msg,
        crate::tui::app::LoadingState::Success(msg) => msg,
        crate::tui::app::LoadingState::Error(msg) => msg,
    };

    let loading_color = match &app.state.loading_state {
        crate::tui::app::LoadingState::Idle => Color::Gray,
        crate::tui::app::LoadingState::Loading(_) => Color::Yellow,
        crate::tui::app::LoadingState::Success(_) => Color::Green,
        crate::tui::app::LoadingState::Error(_) => Color::Red,
    };

    let loading = Paragraph::new(loading_text)
        .style(Style::default().fg(loading_color))
        .block(Block::default().borders(Borders::ALL).title("State"));
    frame.render_widget(loading, status_chunks[1]);

    // Help text at the bottom
    let help_text = "Tab: Next | Shift+Tab: Prev | Enter: Action | Esc: Back | q: Quit";
    let help = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    let help_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(1),
        width: area.width,
        height: 1,
    };
    frame.render_widget(help, help_area);
}

// Placeholder functions for different screens - these will be replaced with actual screen implementations

fn render_pools_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Pools Screen\n\nThis will show:\n• Available pools list\n• Pool details\n• TVL and APY information\n• Pool status indicators")
        .block(Block::default().borders(Borders::ALL).title("Pools"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_swap_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Swap Screen\n\nThis will show:\n• Token swap interface\n• Price impact calculation\n• Slippage settings\n• Transaction confirmation")
        .block(Block::default().borders(Borders::ALL).title("Swap"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_multihop_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Multi-hop Swap Screen\n\nThis will show:\n• Swap route builder\n• Route optimization\n• Multi-step transaction flow\n• Total price impact")
        .block(Block::default().borders(Borders::ALL).title("Multi-hop Swap"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_liquidity_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Liquidity Screen\n\nThis will show:\n• Provide liquidity interface\n• Withdraw liquidity options\n• Current positions\n• LP token management")
        .block(Block::default().borders(Borders::ALL).title("Liquidity"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_rewards_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Rewards Screen\n\nThis will show:\n• Claimable rewards\n• Reward history\n• Epoch information\n• Claim interface")
        .block(Block::default().borders(Borders::ALL).title("Rewards"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_admin_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Admin Screen\n\nThis will show:\n• Pool management\n• Pool creation wizard\n• Feature toggles\n• Administrative controls")
        .block(Block::default().borders(Borders::ALL).title("Admin"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_settings_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Settings Screen\n\nThis will show:\n• Network configuration\n• Wallet management\n• Display preferences\n• Application settings")
        .block(Block::default().borders(Borders::ALL).title("Settings"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}

fn render_transaction_placeholder(frame: &mut Frame, area: Rect, _app: &App) {
    let placeholder = Paragraph::new("Transaction Details Screen\n\nThis will show:\n• Transaction information\n• Status and confirmations\n• Gas fees and events\n• Transaction history")
        .block(Block::default().borders(Borders::ALL).title("Transaction Details"))
        .wrap(Wrap { trim: true });
    frame.render_widget(placeholder, area);
}
