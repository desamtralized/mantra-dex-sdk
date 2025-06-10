// Ensure the library crate is accessible.
// The name "mantra_dex_sdk" should match the `name` field in `[package]` section of Cargo.toml
// or what's specified in `[lib]` section if it has a custom name.
// Assuming the library is named "mantra_dex_sdk" as per previous Cargo.toml.
use mantra_dex_sdk::tui::app::start_tui;

fn main() -> std::io::Result<()> {
    // Call the TUI startup function from the library crate's tui module
    if let Err(e) = start_tui() {
        // Basic error handling for TUI startup failures
        // eprintln is used to print to standard error
        eprintln!("Failed to start TUI: {}", e);
        // Propagate the error out of main if needed, or handle specific exit codes
        return Err(e);
    }
    Ok(())
}
