use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use tui_input::Input; // Required for rendering the input field

use crate::tui::app::{App, AppScreen};

// Main UI rendering function
pub fn render_ui<B: Backend>(frame: &mut Frame<B>, app: &App) {
    let main_block = Block::default().title("Mantra DEX TUI").borders(Borders::ALL);
    let main_area = frame.size();
    frame.render_widget(main_block, main_area);

    // Create a central area within the main block for content
    let content_area = Layout::default()
        .margin(1) // Margin inside the main_block
        .constraints([Constraint::Percentage(100)])
        .split(main_area)[0];

    match app.current_screen {
        AppScreen::Home => render_home_screen(frame, app, content_area),
        AppScreen::CreateWallet => render_create_wallet_screen(frame, app, content_area),
        AppScreen::ImportWallet => render_import_wallet_screen(frame, app, content_area),
        AppScreen::WalletDashboard => render_wallet_dashboard_screen(frame, app, content_area),
        AppScreen::ViewPools => render_view_pools_screen(frame, app, content_area),
        AppScreen::Swap => render_swap_screen(frame, app, content_area),
        AppScreen::ConfirmSwap => render_confirm_swap_screen(frame, app, content_area),
        AppScreen::SwapResult => render_swap_result_screen(frame, app, content_area),
    }

    // Render error messages if any
    if let Some(err_msg) = &app.error_message {
        let error_area = Layout::default()
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(main_area)[1]; // Bottom area for error
        let error_paragraph = Paragraph::new(err_msg.as_str())
            .style(Style::default().fg(Color::Red))
            .block(Block::default().borders(Borders::TOP).title("Error"));
        frame.render_widget(error_paragraph, error_area);
        // Consider clearing the error message in app state after displaying it or after a key press
    }
}

// Renders the Home screen (choose create or import)
fn render_home_screen<B: Backend>(frame: &mut Frame<B>, _app: &App, area: Rect) {
    let choices = vec![
        ListItem::new(Line::from(Span::styled("Create New Wallet (c)", Style::default()))),
        ListItem::new(Line::from(Span::styled("Import Existing Wallet (i)", Style::default()))),
        ListItem::new(Line::from(Span::styled("Quit (q)", Style::default()))),
    ];

    let list = List::new(choices)
        .block(Block::default().title("Welcome").borders(Borders::NONE))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    let vertical_center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Push down
            Constraint::Length(list.len() as u16 + 2), // Height for the list + border
            Constraint::Min(0),         // Fill remaining
        ])
        .split(area)[1]; // Get the middle chunk

    let horizontal_center = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Push from left
            Constraint::Percentage(40), // Width for the list
            Constraint::Percentage(30), // Push from right
        ])
        .split(vertical_center)[1]; // Get the middle chunk for list


    frame.render_widget(list, horizontal_center);
}

// Renders the Create Wallet screen
fn render_create_wallet_screen<B: Backend>(frame: &mut Frame<B>, _app: &App, area: Rect) {
    let text = vec![
        Line::from("Press Enter to generate a new wallet."),
        Line::from("Press Esc to go back or Ctrl+q to quit."),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default().title("Create New Wallet").borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

// Renders the Import Wallet screen
fn render_import_wallet_screen<B: Backend>(frame: &mut Frame<B>, app: &App, area: Rect) {
    // Clear error on screen load (or handle more gracefully)
    // app.error_message = None; // This needs to be done in app logic, not render
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // For instructions
                Constraint::Length(3), // For input field
                Constraint::Min(1),    // Spacer
            ]
            .as_ref(),
        )
        .split(area);

    let instructions = Paragraph::new(
        "Enter your 12 or 24-word mnemonic phrase below.\nPress Enter to import, Esc to go back, or Ctrl+q to quit.",
    )
    .block(Block::default().title("Import Wallet").borders(Borders::NONE));
    frame.render_widget(instructions, chunks[0]);

    let input_field = Paragraph::new(app.mnemonic_input.value())
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title("Mnemonic Phrase"));
    frame.render_widget(input_field, chunks[1]);

    // Make the cursor visible and position it
    frame.set_cursor(
        chunks[1].x + app.mnemonic_input.visual_cursor() as u16 + 1,
        chunks[1].y + 1,
    );
}

// Renders the Wallet Dashboard screen
fn render_wallet_dashboard_screen<B: Backend>(frame: &mut Frame<B>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(if app.generated_mnemonic.is_some() { 8 } else { 1 }), // Mnemonic display (dynamic height)
                Constraint::Length(3), // Address display
                Constraint::Length(3), // Balance display
                Constraint::Min(1),    // Spacer
                Constraint::Length(1), // Footer instructions
            ]
            .as_ref(),
        )
        .split(area);

    let mut info_text = Vec::new();

    if let Some(mnemonic) = &app.generated_mnemonic {
        info_text.push(Line::from(Span::styled(
            "IMPORTANT: Save your mnemonic phrase securely!",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        info_text.push(Line::from(" "));
        info_text.push(Line::from("Your new mnemonic:"));
        info_text.push(Line::from(mnemonic.as_str())); // Display the actual mnemonic
        info_text.push(Line::from(" "));
        info_text.push(Line::from("-------------------------------------------------"));
        info_text.push(Line::from(" "));
    } else {
        // This is a placeholder to ensure layout consistency when no mnemonic is shown
        info_text.push(Line::from(" "));
    }


    let mnemonic_display = Paragraph::new(info_text)
        .block(Block::default().title("Wallet Created/Imported").borders(Borders::NONE))
        .wrap(Wrap { trim: true });
    frame.render_widget(mnemonic_display, chunks[0]);


    let address_display = Paragraph::new(format!(
        "Address: {}",
        app.wallet_address.as_deref().unwrap_or("N/A")
    ))
    .block(Block::default().title("Wallet Details").borders(Borders::NONE));
    frame.render_widget(address_display, chunks[1]);

    let balance_display = Paragraph::new(format!(
        "Balance: {}",
        app.wallet_balance.as_deref().unwrap_or("Loading...")
    ));
    frame.render_widget(balance_display, chunks[2]);

    let footer_text = "Nav: (H)ome | (P)ools | Ctrl+(Q)uit. Counter: (J/K)";
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[4]);
}

// Renders the View Pools screen
fn render_view_pools_screen<B: Backend>(frame: &mut Frame<B>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(area);

    let title = Block::default().title("Available Liquidity Pools (Press Enter to Swap)").borders(Borders::NONE);
    frame.render_widget(title, chunks[0]);

    if let Some(pools) = &app.pools {
        if pools.is_empty() {
            let empty_msg = Paragraph::new("No pools available or fetched.")
                .alignment(Alignment::Center);
            frame.render_widget(empty_msg, chunks[0]);
            return;
        }

        let items: Vec<ListItem> = pools
            .iter()
            .enumerate()
            .map(|(i, pool)| {
                let content = format!(
                    "ID: {} - Assets: {} & {} (Type: {:?}, Share: {})",
                    pool.pool_id,
                    pool.assets.get(0).unwrap_or(&"N/A".to_string()),
                    pool.assets.get(1).unwrap_or(&"N/A".to_string()),
                    pool.pool_type,
                    pool.total_share
                );
                if i == app.selected_pool_index {
                    ListItem::new(Span::styled(content, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                } else {
                    ListItem::new(Span::raw(content))
                }
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_symbol("> ");
        frame.render_widget(list, chunks[0]);

    } else {
        let loading_msg = Paragraph::new("Loading pools... (or press 'P' on Dashboard if stuck)")
            .alignment(Alignment::Center);
        frame.render_widget(loading_msg, chunks[0]);
    }
    let footer_text = "Nav: (Up/Down) Select | (Enter) Swap | (H)ome Dashboard | Ctrl+(Q)uit";
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[1]);
}

// Renders the Swap screen
fn render_swap_screen<B: Backend>(frame: &mut Frame<B>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Pool info
                Constraint::Length(3), // Asset In Input
                Constraint::Length(1), // Asset Out Info (estimated)
                Constraint::Length(3), // Instructions
                Constraint::Min(0),    // Spacer
            ]
            .as_ref(),
        )
        .split(area);

    let pool_info_text = if let Some(pools) = &app.pools {
        if let Some(pool) = pools.get(app.selected_pool_index) {
            format!(
                "Swapping in Pool: {}\nAssets: {} (Sell) <-> {} (Buy)",
                pool.pool_id,
                pool.assets.get(app.asset_in_index).unwrap_or(&"N/A".to_string()),
                pool.assets.get(app.asset_out_index).unwrap_or(&"N/A".to_string())
            )
        } else {
            "Error: Selected pool not found.".to_string()
        }
    } else {
        "Error: Pools not loaded.".to_string()
    };
    let pool_display = Paragraph::new(pool_info_text)
        .block(Block::default().title("Swap Details").borders(Borders::NONE));
    frame.render_widget(pool_display, chunks[0]);

    let amount_in_title = if let Some(pools) = &app.pools {
        if let Some(pool) = pools.get(app.selected_pool_index) {
            format!("Amount of {} to Sell:", pool.assets.get(app.asset_in_index).unwrap_or(&"N/A".to_string()))
        } else { "Amount In:".to_string() }
    } else { "Amount In:".to_string() };

    let input_field = Paragraph::new(app.amount_in_input.value())
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title(amount_in_title));
    frame.render_widget(input_field, chunks[1]);
    frame.set_cursor(
        chunks[1].x + app.amount_in_input.visual_cursor() as u16 + 1,
        chunks[1].y + 1,
    );

    let amount_out_title = if let Some(pools) = &app.pools {
        if let Some(pool) = pools.get(app.selected_pool_index) {
            format!("Estimated {} to Receive:", pool.assets.get(app.asset_out_index).unwrap_or(&"N/A".to_string()))
        } else { "Amount Out:".to_string() }
    } else { "Amount Out:".to_string() };

    let amount_out_display = Paragraph::new(app.amount_out_display.clone())
        .block(Block::default().title(amount_out_title).borders(Borders::NONE));
    frame.render_widget(amount_out_display, chunks[2]);

    let instructions = Paragraph::new("Enter amount, then press Enter to confirm. Tab to switch sell asset (resets amount). Esc to go back.")
        .wrap(Wrap { trim: true });
    frame.render_widget(instructions, chunks[3]);
}

// Renders the Confirm Swap screen
fn render_confirm_swap_screen<B: Backend>(frame: &mut Frame<B>, app: &App, area: Rect) {
    let pool_asset_in = app.pools.as_ref()
        .and_then(|p| p.get(app.selected_pool_index))
        .and_then(|p_info| p_info.assets.get(app.asset_in_index))
        .map_or("N/A", |s| s.as_str());
    let pool_asset_out = app.pools.as_ref()
        .and_then(|p| p.get(app.selected_pool_index))
        .and_then(|p_info| p_info.assets.get(app.asset_out_index))
        .map_or("N/A", |s| s.as_str());

    let text = vec![
        Line::from("Please confirm your swap:"),
        Line::from(" "),
        Line::from(format!("Sell: {} {}", app.amount_in_input.value(), pool_asset_in)),
        Line::from(format!("Receive (Estimated): {} {}", app.amount_out_display, pool_asset_out)),
        Line::from(" "),
        Line::from("Press Enter or 'Y' to execute swap."),
        Line::from("Press Esc to go back or Ctrl+q to quit."),
    ];
    let paragraph = Paragraph::new(text)
        .block(Block::default().title("Confirm Swap").borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

// Renders the Swap Result screen
fn render_swap_result_screen<B: Backend>(frame: &mut Frame<B>, app: &App, area: Rect) {
    let mut text_lines = Vec::new();
    if let Some(tx_hash) = &app.swap_tx_hash {
        text_lines.push(Line::from("Swap Submitted Successfully!"));
        text_lines.push(Line::from(" "));
        text_lines.push(Line::from(format!("Transaction Hash: {}", tx_hash)));
    } else if let Some(err_msg) = &app.error_message { // Should be using app.error_message for swap errors too
        text_lines.push(Line::from(Span::styled("Swap Failed!", Style::default().fg(Color::Red))));
        text_lines.push(Line::from(" "));
        text_lines.push(Line::from(err_msg.as_str()));
    } else {
        // Should not happen if logic is correct, means we are on SwapResult without hash or error
        text_lines.push(Line::from(Span::styled("Swap status unknown.", Style::default().fg(Color::Yellow))));
    }
    text_lines.push(Line::from(" "));
    text_lines.push(Line::from("Press Enter, Esc or 'h' to return to Wallet Dashboard."));

    let paragraph = Paragraph::new(text_lines)
        .block(Block::default().title("Swap Result").borders(Borders::ALL))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}
