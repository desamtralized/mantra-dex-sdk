#[cfg(test)]
mod event_tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use crate::tui::app::{App, AppScreen};
    use crate::tui::event::handle_key_event;

    // Helper to create a KeyEvent
    fn key_event(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn key_event_ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn event_quit_app_ctrl_q() {
        let mut app = App::new();
        app.running = true;
        handle_key_event(key_event_ctrl(KeyCode::Char('q')), &mut app);
        assert_eq!(app.running, false, "App should not be running after Ctrl+Q");
    }

    #[test]
    fn event_home_screen_navigation() {
        let mut app = App::new();
        app.current_screen = AppScreen::Home;

        // Navigate to Create Wallet
        handle_key_event(key_event(KeyCode::Char('c')), &mut app);
        assert_eq!(app.current_screen, AppScreen::CreateWallet);

        // Reset and Navigate to Import Wallet
        app.current_screen = AppScreen::Home;
        handle_key_event(key_event(KeyCode::Char('i')), &mut app);
        assert_eq!(app.current_screen, AppScreen::ImportWallet);

        // Quit from Home
        app.current_screen = AppScreen::Home;
        app.running = true;
        handle_key_event(key_event(KeyCode::Char('q')), &mut app); // 'q' on home screen
        assert_eq!(app.running, false, "App should quit with 'q' from Home");
    }

    #[test]
    fn event_create_wallet_screen_actions() {
        let mut app = App::new();
        app.current_screen = AppScreen::CreateWallet;

        // Press Enter to create wallet
        handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(app.current_screen, AppScreen::WalletDashboard);
        assert!(app.wallet.is_some());

        // Press Esc to go back Home
        app.current_screen = AppScreen::CreateWallet; // Reset screen
        handle_key_event(key_event(KeyCode::Esc), &mut app);
        assert_eq!(app.current_screen, AppScreen::Home);
    }

    #[test]
    fn event_import_wallet_screen_actions() {
        let mut app = App::new();
        app.current_screen = AppScreen::ImportWallet;

        // Type into mnemonic input (basic check, tui-input handles actual text)
        handle_key_event(key_event(KeyCode::Char('a')), &mut app);
        assert_eq!(app.mnemonic_input.value(), "a");

        // Press Esc to go back Home
        app.current_screen = AppScreen::ImportWallet; // Reset screen
        app.mnemonic_input.reset();
        handle_key_event(key_event(KeyCode::Esc), &mut app);
        assert_eq!(app.current_screen, AppScreen::Home);
        assert_eq!(app.mnemonic_input.value(), ""); // Mnemonic input reset

        // Press Enter with invalid (empty) mnemonic - relies on app logic for error
        app.current_screen = AppScreen::ImportWallet;
        app.mnemonic_input.reset();
        handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert!(app.error_message.is_some()); // Expect error message
        assert_eq!(app.current_screen, AppScreen::ImportWallet); // Stays on screen
    }

    #[test]
    fn event_wallet_dashboard_navigation() {
        let mut app = App::new();
        app.current_screen = AppScreen::WalletDashboard;
        // Ensure wallet is "connected" for pool fetching
        app.create_new_wallet(); // Creates a wallet and dex_client
        app.current_screen = AppScreen::WalletDashboard; // Reset to dashboard after creation

        // Navigate to Home
        handle_key_event(key_event(KeyCode::Esc), &mut app);
        assert_eq!(app.current_screen, AppScreen::Home);

        // Navigate to View Pools
        app.current_screen = AppScreen::WalletDashboard; // Reset
        handle_key_event(key_event(KeyCode::Char('p')), &mut app);
        assert_eq!(app.current_screen, AppScreen::ViewPools);
        assert!(app.pools.is_some()); // Pools should be fetched (simulated)
    }

    #[test]
    fn event_view_pools_screen_navigation_and_selection() {
        let mut app = App::new();
        app.create_new_wallet(); // Initialize wallet and client
        app.fetch_pools();       // Populate pools
        app.current_screen = AppScreen::ViewPools; // Set screen

        assert_eq!(app.selected_pool_index, 0);

        // Navigate down pool list
        handle_key_event(key_event(KeyCode::Down), &mut app);
        if app.pools.as_ref().map_or(0, |p| p.len()) > 1 {
            assert_eq!(app.selected_pool_index, 1);
        } else {
            assert_eq!(app.selected_pool_index, 0);
        }

        // Navigate up pool list
        handle_key_event(key_event(KeyCode::Up), &mut app);
        assert_eq!(app.selected_pool_index, 0);

        // Press Enter to go to Swap screen
        handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(app.current_screen, AppScreen::Swap);
    }

    #[test]
    fn event_swap_screen_input_and_navigation() {
        let mut app = App::new();
        app.create_new_wallet();
        app.fetch_pools();
        app.prepare_swap(); // Sets current_screen to Swap and initializes swap state

        // Type into amount input
        handle_key_event(key_event(KeyCode::Char('1')), &mut app);
        assert_eq!(app.amount_in_input.value(), "1");

        // Tab to switch assets (simple toggle)
        let initial_asset_in = app.asset_in_index;
        handle_key_event(key_event(KeyCode::Tab), &mut app);
        assert_ne!(app.asset_in_index, initial_asset_in);
        assert_eq!(app.amount_in_input.value(), ""); // Amount resets

        // Enter to confirm swap (assuming valid amount)
        app.amount_in_input.handle_event(&crossterm::event::Event::Key(key_event(KeyCode::Char('5')))); // Input "5"
        handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(app.current_screen, AppScreen::ConfirmSwap);

        // Enter with empty amount (should set error and stay)
        app.current_screen = AppScreen::Swap; // Reset to swap screen
        app.amount_in_input.reset();
        handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(app.current_screen, AppScreen::Swap);
        assert!(app.error_message.is_some());
        assert_eq!(app.error_message.unwrap(), "Please enter a valid amount to swap.");
    }

    #[test]
    fn event_confirm_swap_screen_actions() {
        let mut app = App::new();
        app.create_new_wallet();
        app.fetch_pools();
        app.prepare_swap();
        app.amount_in_input.handle_event(&crossterm::event::Event::Key(key_event(KeyCode::Char('1'))));
        app.current_screen = AppScreen::ConfirmSwap;

        // Confirm (Enter)
        handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(app.current_screen, AppScreen::SwapResult);
        assert!(app.swap_tx_hash.is_some()); // Simulated success

        // Go back (Esc)
        app.current_screen = AppScreen::ConfirmSwap; // Reset
        handle_key_event(key_event(KeyCode::Esc), &mut app);
        assert_eq!(app.current_screen, AppScreen::Swap);
    }

    #[test]
    fn event_swap_result_screen_navigation() {
        let mut app = App::new();
        app.current_screen = AppScreen::SwapResult;

        handle_key_event(key_event(KeyCode::Enter), &mut app);
        assert_eq!(app.current_screen, AppScreen::WalletDashboard);
    }
}
