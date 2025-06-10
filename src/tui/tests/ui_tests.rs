#[cfg(test)]
mod ui_tests {
    use crate::tui::app::App;
    use crate::tui::ui::render_ui;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn ui_render_initial_state_does_not_panic() {
        let app = App::new();
        let backend = TestBackend::new(100, 30); // Arbitrary size, width >= ~80 for some layouts
        let mut terminal = Terminal::new(backend).unwrap();

        // Test if rendering the initial state panics
        let result = std::panic::catch_unwind(|| {
            terminal.draw(|frame| render_ui(frame, &app)).unwrap();
        });
        assert!(result.is_ok(), "UI rendering panicked for initial state");
    }

    #[test]
    fn ui_render_wallet_dashboard_does_not_panic() {
        let mut app = App::new();
        app.create_new_wallet(); // Go to dashboard
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let result = std::panic::catch_unwind(|| {
            terminal.draw(|frame| render_ui(frame, &app)).unwrap();
        });
        assert!(result.is_ok(), "UI rendering panicked for WalletDashboard");
    }

    #[test]
    fn ui_render_import_wallet_screen_does_not_panic() {
        let mut app = App::new();
        app.current_screen = crate::tui::app::AppScreen::ImportWallet;
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let result = std::panic::catch_unwind(|| {
            terminal.draw(|frame| render_ui(frame, &app)).unwrap();
        });
        assert!(result.is_ok(), "UI rendering panicked for ImportWallet screen");
    }


    // More advanced UI tests would involve:
    // - Creating specific App states for different screens.
    // - Rendering the UI to the TestBackend.
    // - Asserting the content of the backend's buffer using `backend.assert_buffer(&expected_buffer)`.
    // This requires constructing an `ExpectedBuffer` which is tedious but provides thorough testing.
    // Example:
    // use ratatui::buffer::Buffer;
    // let mut expected_buffer = Buffer::empty(frame.size());
    // expected_buffer.set_string(0,0, "Expected Text", Style::default());
    // backend.assert_buffer(&expected_buffer); (pseudo-code for where this would go)
}
