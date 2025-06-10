//! MANTRA DEX SDK TUI Module
//!
//! This module provides a Terminal User Interface (TUI) for interacting with the MANTRA DEX SDK.
//! It offers a comprehensive interface for all DEX operations including swaps, liquidity management,
//! rewards, and administrative functions.

#[cfg(feature = "tui")]
pub mod app;
#[cfg(feature = "tui")]
pub mod components;
#[cfg(feature = "tui")]
pub mod events;
#[cfg(feature = "tui")]
pub mod screens;
#[cfg(feature = "tui")]
pub mod ui;
#[cfg(feature = "tui")]
pub mod utils;

#[cfg(feature = "tui")]
pub use app::{App, AppState};
#[cfg(feature = "tui")]
pub use events::{Event, EventHandler};
#[cfg(feature = "tui")]
pub use ui::render_ui;

// Note: Terminal management functions are directly defined in this module and automatically exported

#[cfg(feature = "tui")]
use crate::config::MantraNetworkConfig;
#[cfg(feature = "tui")]
use crate::{Error, MantraDexClient};
#[cfg(feature = "tui")]
use crossterm::{
    cursor, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
#[cfg(feature = "tui")]
use ratatui::{backend::CrosstermBackend, Terminal};
#[cfg(feature = "tui")]
use std::io::{self, Stdout};
#[cfg(feature = "tui")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "tui")]
pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Global flag to track if terminal cleanup is needed
#[cfg(feature = "tui")]
static TERMINAL_NEEDS_CLEANUP: AtomicBool = AtomicBool::new(false);

/// Initialize the terminal for TUI mode
///
/// Sets up the terminal with alternate screen and raw mode for TUI interaction.
/// Automatically tracks that cleanup will be needed.
#[cfg(feature = "tui")]
pub fn init_terminal() -> Result<TuiTerminal, Error> {
    enable_raw_mode().map_err(Error::Io)?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(Error::Io)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(Error::Io)?;

    // Hide cursor for a cleaner interface
    terminal.hide_cursor().map_err(Error::Io)?;

    // Mark that terminal cleanup will be needed
    TERMINAL_NEEDS_CLEANUP.store(true, Ordering::SeqCst);

    Ok(terminal)
}

/// Restore the terminal to normal mode
///
/// Disables raw mode, leaves alternate screen, and shows cursor.
/// Safe to call multiple times.
#[cfg(feature = "tui")]
pub fn restore_terminal(terminal: &mut TuiTerminal) -> Result<(), Error> {
    // Only restore if cleanup is needed
    if TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst) {
        disable_raw_mode().map_err(Error::Io)?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(Error::Io)?;
        terminal.show_cursor().map_err(Error::Io)?;

        // Mark cleanup as complete
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);
    }
    Ok(())
}

/// Emergency terminal cleanup for panic situations
///
/// Performs basic terminal restoration without error handling to ensure
/// terminal state is restored even during panics.
#[cfg(feature = "tui")]
fn emergency_terminal_cleanup() {
    if TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst) {
        // Ignore errors during emergency cleanup
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        let _ = execute!(io::stdout(), cursor::Show);
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);
    }
}

/// Setup panic handler for graceful terminal restoration
///
/// Installs a panic handler that will restore terminal state before
/// displaying panic information.
#[cfg(feature = "tui")]
pub fn setup_panic_handler() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        emergency_terminal_cleanup();
        original_hook(panic_info);
    }));
}

/// Main TUI application entry point
///
/// This is the primary function to run the TUI application. It handles:
/// - Terminal initialization and cleanup
/// - Panic handler setup
/// - Main application loop
/// - Graceful shutdown
///
/// # Arguments
/// * `client` - The MANTRA DEX client instance
/// * `config` - Network configuration
///
/// # Returns
/// * `Ok(())` - If the application exits normally
/// * `Err(Error)` - If there's an initialization or runtime error
#[cfg(feature = "tui")]
pub async fn run_tui(client: MantraDexClient, config: MantraNetworkConfig) -> Result<(), Error> {
    // Setup panic handler for graceful terminal restoration
    setup_panic_handler();

    // Initialize terminal
    let mut terminal = init_terminal().map_err(|e| {
        emergency_terminal_cleanup();
        e
    })?;

    // Create application state
    let mut app = App::new(client, config);
    let mut event_handler = EventHandler::new();

    // Set initial status
    app.set_status("MANTRA DEX TUI - Press 'q' to quit, 'h' for help".to_string());

    // Application result for error handling
    let app_result = run_app_loop(&mut terminal, &mut app, &mut event_handler).await;

    // Always attempt to restore terminal, even if app_result is an error
    if let Err(restore_error) = restore_terminal(&mut terminal) {
        // If we had an app error, prioritize that, otherwise report restore error
        if app_result.is_ok() {
            return Err(restore_error);
        }
        // Log restore error but return the original app error
        eprintln!("Warning: Failed to restore terminal: {}", restore_error);
    }

    app_result
}

/// Internal application loop
///
/// Separated from run_tui to allow better error handling and cleanup.
#[cfg(feature = "tui")]
async fn run_app_loop(
    terminal: &mut TuiTerminal,
    app: &mut App,
    event_handler: &mut EventHandler,
) -> Result<(), Error> {
    // Main application loop
    loop {
        // Render UI
        terminal
            .draw(|frame| {
                if let Err(e) = render_ui(frame, app) {
                    app.set_error(format!("Render error: {}", e));
                }
            })
            .map_err(Error::Io)?;

        // Handle events with timeout to allow for periodic updates
        match tokio::time::timeout(std::time::Duration::from_millis(100), event_handler.next())
            .await
        {
            Ok(Ok(event)) => match app.handle_event(event).await {
                Ok(should_quit) => {
                    if should_quit {
                        break;
                    }
                }
                Err(e) => {
                    app.set_error(format!("Event handling error: {}", e));
                }
            },
            Ok(Err(e)) => {
                app.set_error(format!("Event error: {}", e));
            }
            Err(_) => {
                // Timeout - continue loop for periodic updates
                // This allows the UI to refresh even without user input
            }
        }

        // Check if application should quit
        if app.state.should_quit {
            break;
        }
    }

    Ok(())
}

/// Quick terminal check
///
/// Verifies that the terminal supports the required features for the TUI.
/// This can be called before initializing the full TUI to provide better
/// error messages.
#[cfg(feature = "tui")]
pub fn check_terminal_support() -> Result<(), Error> {
    // Check if we're running in a terminal
    if !atty::is(atty::Stream::Stdout) {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "TUI requires a terminal (stdout is not a TTY)",
        )));
    }

    // Try to get terminal size
    let (_width, _height) = crossterm::terminal::size().map_err(Error::Io)?;

    // Check minimum terminal size
    if _width < 80 || _height < 24 {
        return Err(Error::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "Terminal too small: {}x{} (minimum: 80x24)",
                _width, _height
            ),
        )));
    }

    Ok(())
}

// Simple atty check implementation
#[cfg(feature = "tui")]
mod atty {
    pub enum Stream {
        Stdout,
    }

    pub fn is(_stream: Stream) -> bool {
        // Simple check - if we can get terminal size, we're probably in a terminal
        crossterm::terminal::size().is_ok()
    }
}

#[cfg(test)]
#[cfg(feature = "tui")]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_terminal_needs_cleanup_flag() {
        // Reset flag to initial state
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);

        // Flag should be false initially
        assert!(!TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));

        // After marking that cleanup is needed
        TERMINAL_NEEDS_CLEANUP.store(true, Ordering::SeqCst);
        assert!(TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));

        // Reset for other tests
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);
    }

    #[test]
    fn test_emergency_terminal_cleanup_when_not_needed() {
        // Reset flag
        TERMINAL_NEEDS_CLEANUP.store(false, Ordering::SeqCst);

        // Should be safe to call when cleanup not needed
        emergency_terminal_cleanup();

        // Flag should remain false
        assert!(!TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));
    }

    #[test]
    fn test_emergency_terminal_cleanup_when_needed() {
        // Set flag to indicate cleanup needed
        TERMINAL_NEEDS_CLEANUP.store(true, Ordering::SeqCst);

        // Call emergency cleanup
        emergency_terminal_cleanup();

        // Flag should be reset to false
        assert!(!TERMINAL_NEEDS_CLEANUP.load(Ordering::SeqCst));
    }

    #[test]
    fn test_panic_handler_setup() {
        // This test just verifies the function doesn't panic
        // In a real scenario, testing panic handlers is complex
        setup_panic_handler();

        // If we get here, the setup succeeded
        assert!(true);
    }

    #[test]
    fn test_check_terminal_support_success() {
        // In most test environments, this should pass
        // as they run in terminals
        if crossterm::terminal::size().is_ok() {
            // We can't actually test terminal support without a real terminal
            // so we'll just verify the function exists and can be called
            let result = check_terminal_support();
            // The result may be Ok or Err depending on test environment
            // We just want to make sure it doesn't panic
            let _ = result;
        }
    }
}
