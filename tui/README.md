# Mantra DEX TUI

A terminal user interface for interacting with the Mantra DEX blockchain.

## Features

- Dashboard view with overview information
- Pool listing and details view
- Token swap interface with simulation
- Liquidity operations
- Wallet management
- Terminal-based interface with keyboard navigation

## Installation

### Building from source

1. Clone the repository:
```bash
git clone https://github.com/your-org/mantra-dex-sdk.git
cd mantra-dex-sdk
```

2. Build the TUI:
```bash
cargo build --release -p mantra-dex-tui
```

3. The binary will be available at `target/release/mantra-dex-tui`.

## Configuration

The TUI uses a configuration file located at `~/.config/mantra-dex-tui/config.toml`. You can specify a custom configuration file with the `--config` option.

To set up your configuration:

1. Copy the example configuration file:
```bash
mkdir -p ~/.config/mantra-dex-tui
cp tui/config.toml.example ~/.config/mantra-dex-tui/config.toml
```

2. Edit the configuration file:
```bash
vim ~/.config/mantra-dex-tui/config.toml
```

3. Update the network settings.

## Usage

The TUI provides a keyboard-based interface for interacting with the Mantra DEX:

- Use numbers `1-5` to switch between tabs:
  - `1`: Dashboard
  - `2`: Pools
  - `3`: Swap
  - `4`: Liquidity
  - `5`: Wallet

- Press `e` to enter editing mode for input fields
- Use `Tab` and `Shift+Tab` to navigate between fields
- Press `Esc` to exit editing mode
- Press `q` to quit the application

### Swap Interface

1. Navigate to the Swap tab (press `3`)
2. Press `e` to enter editing mode
3. Fill in the required fields:
   - Pool ID
   - Offer Asset (e.g., `1000:uom`)
   - Ask Denom (e.g., `uusdt`)
   - Max Spread (default: `0.01`)
4. Press `Enter` to simulate the swap
5. Review the simulation results

## Development

### Dependencies

- [ratatui](https://github.com/ratatui-org/ratatui): Terminal UI library
- [crossterm](https://github.com/crossterm-rs/crossterm): Terminal manipulation library
- [mantra-dex-sdk](https://github.com/your-org/mantra-dex-sdk): Mantra DEX SDK

### Project Structure

- `src/main.rs`: Application entry point and event loop
- `src/app.rs`: Application state and logic
- `src/ui.rs`: UI rendering functions
- `src/config.rs`: Configuration handling
- `src/error.rs`: Error types
- `src/utils.rs`: Utility functions

## License

[License details] 