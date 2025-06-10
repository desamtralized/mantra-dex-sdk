use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::tui::app::{App, AppScreen}; // Ensure App and AppScreen are imported
use tui_input::backend::crossterm::EventHandler; // For input field

// Key event handler
pub fn handle_key_event(key_event: KeyEvent, app: &mut App) {
    // Global quit command
    if key_event.code == KeyCode::Char('q') && key_event.modifiers == KeyModifiers::CONTROL {
        app.quit();
        return;
    }

    // Clear error on most key presses, specific handlers below might re-set it.
    // This is a bit broad, but helps ensure errors don't stick around too long.
    // More targeted clearing is done in specific handlers.
    // app.clear_error_message(); // Decided against this broad approach.

    match app.current_screen {
        AppScreen::Home => match key_event.code {
            KeyCode::Char('c') | KeyCode::Char('C') => {
                app.clear_error_message();
                app.current_screen = AppScreen::CreateWallet;
            }
            KeyCode::Char('i') | KeyCode::Char('I') => {
                app.clear_error_message();
                app.current_screen = AppScreen::ImportWallet;
            }
            KeyCode::Char('q') => app.quit(),
            _ => {}
        },
        AppScreen::CreateWallet => match key_event.code {
            KeyCode::Enter => {
                // app.create_new_wallet() will clear its own errors if any were set prior
                app.create_new_wallet();
            }
            KeyCode::Esc => {
                app.clear_error_message();
                app.current_screen = AppScreen::Home;
            }
            KeyCode::Char('q') => app.quit(),
            _ => {}
        },
        AppScreen::ImportWallet => match key_event.code {
            KeyCode::Enter => {
                // import_wallet_from_mnemonic will clear previous and set new errors if they occur
                app.import_wallet_from_mnemonic();
            }
            KeyCode::Esc => {
                app.clear_error_message();
                app.mnemonic_input.reset();
                app.current_screen = AppScreen::Home;
            }
            KeyCode::Char('q') => app.quit(),
            // Use tui-input to handle input for the mnemonic field
            _ => {
                // Clear previous error when user starts typing
                if app.error_message.is_some() {
                     app.clear_error_message();
                }
                app.mnemonic_input.handle_event(&crossterm::event::Event::Key(key_event));
            }
        },
        AppScreen::WalletDashboard => match key_event.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('H') => { // Go back home
                app.clear_error_message();
                app.current_screen = AppScreen::Home;
                app.generated_mnemonic = None; // Clear generated mnemonic
                app.wallet_address = None;
                app.wallet_balance = None;
            }
            KeyCode::Char('q') => app.quit(),
            KeyCode::Char('p') | KeyCode::Char('P') => { // Go to Pools
                app.clear_error_message();
                // We call fetch_pools here. In a real async app, this might be initiated
                // earlier or on screen load. For now, direct call.
                app.fetch_pools(); // This will change screen to ViewPools or show error
            }
            // Example navigation, can be removed or adapted
            KeyCode::Char('j') | KeyCode::Down => app.decrement_counter(),
            KeyCode::Char('k') | KeyCode::Up => app.increment_counter(),
            _ => {}
        },
        AppScreen::ViewPools => match key_event.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('H') => {
                app.clear_error_message();
                app.current_screen = AppScreen::WalletDashboard;
            }
            KeyCode::Char('q') => app.quit(),
            KeyCode::Down | KeyCode::Char('j') => {
                app.clear_error_message(); // Clear error when navigating list
                if let Some(pools) = &app.pools {
                    if !pools.is_empty() {
                        app.selected_pool_index = (app.selected_pool_index + 1) % pools.len();
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.clear_error_message(); // Clear error when navigating list
                if let Some(pools) = &app.pools {
                    if !pools.is_empty() {
                        app.selected_pool_index = if app.selected_pool_index == 0 {
                            pools.len() - 1
                        } else {
                            app.selected_pool_index - 1
                        };
                    }
                }
            }
            KeyCode::Enter => { // Select pool and go to Swap screen
                // app.prepare_swap() will clear its own errors
                app.prepare_swap();
            }
            _ => {}
        },
        AppScreen::Swap => match key_event.code {
            KeyCode::Esc => {
                app.clear_error_message();
                app.current_screen = AppScreen::ViewPools; // Go back to pool list
            }
            KeyCode::Char('q') => app.quit(),
            KeyCode::Tab => { // Switch between asset_in and asset_out (simplified)
                app.clear_error_message();
                // This is a basic toggle. A real implementation might cycle focus.
                // Or, specific keys to choose asset_in vs asset_out if there are more than 2.
                let temp_index = app.asset_in_index;
                app.asset_in_index = app.asset_out_index;
                app.asset_out_index = temp_index;
                app.amount_in_input.reset(); // Reset amount when assets change
                app.amount_out_display = "0.0".to_string();
            }
            KeyCode::Enter => { // Go to ConfirmSwap screen
                app.clear_error_message();
                if !app.amount_in_input.value().is_empty() && app.amount_in_input.value().parse::<f64>().is_ok() {
                    app.current_screen = AppScreen::ConfirmSwap;
                } else {
                    app.error_message = Some("Please enter a valid amount to swap.".to_string());
                }
            }
            // Handle input for the amount_in_input field
            _ => {
                if app.error_message.is_some() {
                    app.clear_error_message();
                }
                if app.amount_in_input.handle_event(&crossterm::event::Event::Key(key_event)).is_some() {
                    // If input changed, recalculate preview
                    app.calculate_swap_preview();
                }
            }
        },
        AppScreen::ConfirmSwap => match key_event.code {
            KeyCode::Esc => {
                app.clear_error_message();
                app.current_screen = AppScreen::Swap; // Go back to Swap screen
            }
            KeyCode::Char('q') => app.quit(),
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                // app.execute_swap() will clear previous error and set new ones if they occur
                app.execute_swap(); // This will change screen to SwapResult or show error
            }
            _ => {}
        },
        AppScreen::SwapResult => match key_event.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('h') | KeyCode::Char('H') => {
                app.clear_error_message(); // Clear any error from the swap result
                app.current_screen = AppScreen::WalletDashboard; // Back to dashboard
                app.swap_tx_hash = None; // Clear tx hash
            }
            KeyCode::Char('q') => app.quit(),
            _ => {}
        },
    }
}
