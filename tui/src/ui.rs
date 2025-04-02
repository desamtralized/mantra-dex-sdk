use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, ListItem, Paragraph, Tabs, List,
    },
    Frame,
};

use crate::app::{App, InputMode, TabState};

pub fn render<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Create the layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),  // Tabs
                Constraint::Min(0),     // Content
                Constraint::Length(1),  // Status bar
            ]
            .as_ref(),
        )
        .split(f.size());

    // Render tabs
    render_tabs(f, app, chunks[0]);

    // Render main content based on the active tab
    match app.tab {
        TabState::Dashboard => render_dashboard(f, app, chunks[1]),
        TabState::Pools => render_pools(f, app, chunks[1]),
        TabState::Swap => render_swap(f, app, chunks[1]),
        TabState::Liquidity => render_liquidity(f, app, chunks[1]),
        TabState::Wallet => render_wallet(f, app, chunks[1]),
    }

    // Render status bar
    render_status_bar(f, app, chunks[2]);

    // Render any popup messages
    if let Some(msg) = &app.message {
        render_popup(f, app, msg);
    }
}

fn render_tabs<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let titles = [
        "Dashboard",
        "Pools",
        "Swap",
        "Liquidity",
        "Wallet",
    ]
    .iter()
    .map(|t| {
        let (first, rest) = t.split_at(1);
        Line::from(vec![
            Span::styled(
                first,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled(rest, Style::default().fg(Color::White)),
        ])
    })
    .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Mantra DEX"))
        .select(app.tab as usize)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn render_dashboard<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ]
            .as_ref(),
        )
        .split(area);

    // Wallet overview
    let wallet_info = if let Some(wallet) = &app.wallet {
        format!("Wallet: {}\nAddress: {}", wallet.name, wallet.address)
    } else {
        "No wallet connected".to_string()
    };

    let wallet_widget = Paragraph::new(wallet_info)
        .block(Block::default().borders(Borders::ALL).title("Wallet Overview"))
        .style(Style::default().fg(Color::White));

    f.render_widget(wallet_widget, chunks[0]);

    // Stats/activity
    let stats = "Total Value Locked: --\nTotal Pools: --\nActive Swaps: --";
    let stats_widget = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).title("DEX Stats"))
        .style(Style::default().fg(Color::White));

    f.render_widget(stats_widget, chunks[1]);
}

fn render_pools<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(30),
                Constraint::Percentage(70),
            ]
            .as_ref(),
        )
        .split(area);

    // Pool list
    let pools: Vec<ListItem> = if app.pools.is_empty() {
        vec![ListItem::new("No pools available")]
    } else {
        app.pools
            .iter()
            .map(|p| ListItem::new(format!("Pool {}", p.pool_identifier)))
            .collect()
    };

    let pools_widget = List::new(pools)
        .block(Block::default().borders(Borders::ALL).title("Pools"))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(pools_widget, chunks[0]);

    // Pool details
    let pool_details = if let Some(idx) = app.selected_pool_idx {
        if idx < app.pools.len() {
            let pool = &app.pools[idx];
            format!("Pool ID: {}\nPool Assets: --\nTotal Liquidity: --", pool.pool_identifier)
        } else {
            "Select a pool to see details".to_string()
        }
    } else {
        "Select a pool to see details".to_string()
    };

    let details_widget = Paragraph::new(pool_details)
        .block(Block::default().borders(Borders::ALL).title("Pool Details"))
        .style(Style::default().fg(Color::White));

    f.render_widget(details_widget, chunks[1]);
}

fn render_swap<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),  // Pool ID
                Constraint::Length(3),  // Offer Asset
                Constraint::Length(3),  // Ask Denom
                Constraint::Length(3),  // Max Spread
                Constraint::Length(1),  // Spacer
                Constraint::Length(3),  // Simulate button
                Constraint::Min(0),     // Results
            ]
            .as_ref(),
        )
        .split(area);

    // Pool ID
    let pool_id_input = Paragraph::new(app.swap_form.pool_id.value())
        .style(Style::default().fg(if app.input_mode == InputMode::Editing && app.swap_form.active_field == 0 {
            Color::Yellow
        } else {
            Color::White
        }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Pool ID")
                .border_style(Style::default().fg(if app.swap_form.active_field == 0 {
                    Color::Yellow
                } else {
                    Color::White
                })),
        );
    f.render_widget(pool_id_input, chunks[0]);

    // Offer Asset
    let offer_asset_input = Paragraph::new(app.swap_form.offer_asset.value())
        .style(Style::default().fg(if app.input_mode == InputMode::Editing && app.swap_form.active_field == 1 {
            Color::Yellow
        } else {
            Color::White
        }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Offer Asset (e.g., 1000:uom)")
                .border_style(Style::default().fg(if app.swap_form.active_field == 1 {
                    Color::Yellow
                } else {
                    Color::White
                })),
        );
    f.render_widget(offer_asset_input, chunks[1]);

    // Ask Denom
    let ask_denom_input = Paragraph::new(app.swap_form.ask_denom.value())
        .style(Style::default().fg(if app.input_mode == InputMode::Editing && app.swap_form.active_field == 2 {
            Color::Yellow
        } else {
            Color::White
        }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Ask Denom (e.g., uusdt)")
                .border_style(Style::default().fg(if app.swap_form.active_field == 2 {
                    Color::Yellow
                } else {
                    Color::White
                })),
        );
    f.render_widget(ask_denom_input, chunks[2]);

    // Max Spread
    let max_spread_input = Paragraph::new(app.swap_form.max_spread.value())
        .style(Style::default().fg(if app.input_mode == InputMode::Editing && app.swap_form.active_field == 3 {
            Color::Yellow
        } else {
            Color::White
        }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Max Spread (e.g., 0.01)")
                .border_style(Style::default().fg(if app.swap_form.active_field == 3 {
                    Color::Yellow
                } else {
                    Color::White
                })),
        );
    f.render_widget(max_spread_input, chunks[3]);

    // Simulate button
    let simulate_text = if app.input_mode == InputMode::Normal {
        "Press 'e' to edit, then Enter to simulate"
    } else {
        "Press Enter to simulate swap"
    };
    let simulate_button = Paragraph::new(simulate_text)
        .style(Style::default().fg(Color::Green))
        .block(Block::default().borders(Borders::ALL).title("Simulate"));
    f.render_widget(simulate_button, chunks[5]);

    // Results
    if let Some(result) = &app.swap_form.simulate_result {
        let result_widget = Paragraph::new(result.clone())
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Simulation Results"));
        f.render_widget(result_widget, chunks[6]);
    }

    // Set cursor position when in editing mode
    if app.input_mode == InputMode::Editing {
        match app.swap_form.active_field {
            0 => {
                f.set_cursor(
                    chunks[0].x + app.swap_form.pool_id.cursor() as u16 + 1,
                    chunks[0].y + 1,
                );
            }
            1 => {
                f.set_cursor(
                    chunks[1].x + app.swap_form.offer_asset.cursor() as u16 + 1,
                    chunks[1].y + 1,
                );
            }
            2 => {
                f.set_cursor(
                    chunks[2].x + app.swap_form.ask_denom.cursor() as u16 + 1,
                    chunks[2].y + 1,
                );
            }
            3 => {
                f.set_cursor(
                    chunks[3].x + app.swap_form.max_spread.cursor() as u16 + 1,
                    chunks[3].y + 1,
                );
            }
            _ => {}
        }
    }
}

fn render_liquidity<B: Backend>(f: &mut Frame<B>, _app: &App, area: Rect) {
    let text = "Liquidity operations are not yet implemented.";
    let widget = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Liquidity"))
        .style(Style::default().fg(Color::White));

    f.render_widget(widget, area);
}

fn render_wallet<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    if let Some(wallet) = &app.wallet {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3),  // Wallet info
                    Constraint::Min(0),     // Balances
                ]
                .as_ref(),
            )
            .split(area);

        // Wallet info
        let wallet_info = format!("Name: {}\nAddress: {}", wallet.name, wallet.address);
        let info_widget = Paragraph::new(wallet_info)
            .block(Block::default().borders(Borders::ALL).title("Wallet Info"))
            .style(Style::default().fg(Color::White));
        
        f.render_widget(info_widget, chunks[0]);

        // Balances
        let balances: Vec<ListItem> = wallet
            .balances
            .iter()
            .map(|(denom, amount)| ListItem::new(format!("{}: {}", denom, amount)))
            .collect();

        let balances_widget = List::new(balances)
            .block(Block::default().borders(Borders::ALL).title("Balances"))
            .style(Style::default().fg(Color::White));
        
        f.render_widget(balances_widget, chunks[1]);
    } else {
        let text = "No wallet connected. Please connect a wallet first.";
        let widget = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Wallet"))
            .style(Style::default().fg(Color::White));

        f.render_widget(widget, area);
    }
}

fn render_status_bar<B: Backend>(f: &mut Frame<B>, app: &App, area: Rect) {
    let mode = match app.input_mode {
        InputMode::Normal => "NORMAL",
        InputMode::Editing => "EDITING",
    };

    let wallet_status = if app.wallet.is_some() {
        "Wallet: Connected"
    } else {
        "Wallet: Not Connected"
    };

    let network = &app.config.network.network_name;

    let status = format!("{} | {} | Network: {} | Press 'q' to quit", mode, wallet_status, network);
    let status_widget = Paragraph::new(status)
        .style(Style::default().fg(Color::White));

    f.render_widget(status_widget, area);
}

fn render_popup<B: Backend>(f: &mut Frame<B>, _app: &App, message: &str) {
    let area = f.size();
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(40),
                Constraint::Length(3),
                Constraint::Percentage(40),
            ]
            .as_ref(),
        )
        .split(area);

    let popup_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));

    let text = Text::from(message.to_string());
    let paragraph = Paragraph::new(text)
        .block(popup_block)
        .style(Style::default().fg(Color::White))
        .alignment(ratatui::layout::Alignment::Center);

    f.render_widget(Clear, popup_layout[1]);
    f.render_widget(paragraph, popup_layout[1]);
} 