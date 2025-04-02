mod app;
mod config;
mod error;
mod ui;
mod utils;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::app::{App, AppResult};
use crate::config::TuiConfig;

#[derive(Parser)]
#[command(name = "mantra-dex-tui")]
#[command(about = "Mantra DEX TUI", long_about = None)]
struct Cli {
    /// Path to config file
    #[arg(short, long)]
    config: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Setup logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse command-line arguments
    let cli = Cli::parse();

    // Initialize configuration
    let config_path = if let Some(path) = cli.config {
        std::path::PathBuf::from(path)
    } else {
        TuiConfig::default_path()
    };

    let config = TuiConfig::load(&config_path)?;

    // Initialize the terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialize the application state
    let app = App::new(&config).await?;

    // Run the main application loop
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Handle any errors during app execution
    if let Err(err) = res {
        eprintln!("{}", err);
        return Err(err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> AppResult<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // Render the UI
        terminal.draw(|f| ui::render(f, &mut app))?;

        // Handle input events
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key_event(key)? {
                    return Ok(());
                }
            }
        }

        // Check if it's time to update the app state
        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }
    }
} 