use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{prelude::*, Terminal};
use std::io;
use tui_input::Input;

use crate::tui::{event::handle_key_event, ui::render_ui};
use crate::wallet::MantraWallet;
use crate::client::MantraDexClient; // Import MantraDexClient
use crate::config::NetworkConstants; // For default client config
use mantra_dex_std::pool_manager::PoolInfo; // Import PoolInfo

// Define the different screens/states of the TUI application
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AppScreen {
    Home,
    CreateWallet,
    ImportWallet,
    WalletDashboard,
    ViewPools,
    Swap,
    ConfirmSwap,
    SwapResult,
    // TODO: Add other screens like Liquidity Management, etc.
}

pub struct App {
    // General App State
    pub running: bool,
    pub current_screen: AppScreen,
    pub error_message: Option<String>, // For displaying general errors

    // Wallet State
    pub wallet: Option<MantraWallet>, // Store the wallet instance
    pub mnemonic_input: Input,
    pub generated_mnemonic: Option<String>,
    pub wallet_address: Option<String>,
    pub wallet_balance: Option<String>, // TODO: Use proper types like Vec<Coin>

    // DEX Client and State
    pub dex_client: Option<MantraDexClient>,
    pub pools: Option<Vec<PoolInfo>>,
    pub selected_pool_index: usize,
    pub asset_in_index: usize, // 0 or 1 within the selected pool's assets
    pub asset_out_index: usize, // The other asset in the pair
    pub amount_in_input: Input,
    pub amount_out_display: String, // Estimated amount out
    pub swap_tx_hash: Option<String>,

    // Example counter, can be removed
    pub counter: i32,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            current_screen: AppScreen::Home,
            error_message: None,
            wallet: None,
            mnemonic_input: Input::default(),
            generated_mnemonic: None,
            wallet_address: None,
            wallet_balance: None,
            dex_client: None,
            pools: None,
            selected_pool_index: 0,
            asset_in_index: 0,
            asset_out_index: 1,
            amount_in_input: Input::default(),
            amount_out_display: "0.0".to_string(),
            swap_tx_hash: None,
            counter: 0,
        }
    }

    pub fn clear_error_message(&mut self) {
        self.error_message = None;
    }

    // Wallet interaction methods
    fn initialize_dex_client_from_wallet(&mut self, wallet: MantraWallet) {
        self.clear_error_message();
        // Assuming MantraDexClient::new takes network config and wallet
        // This is a placeholder for actual client initialization
        // You'll need to decide how to get network config (e.g., default, from file)
        let config = NetworkConstants::default_dukong(); // Example: use default config
        match MantraDexClient::new(config, wallet.clone()) {
            Ok(client) => {
                self.dex_client = Some(client);
                self.wallet = Some(wallet); // Store the wallet
            }
            Err(_e) => {
                self.error_message = Some("Failed to initialize DEX client.".to_string());
                // Fallback or error state
                self.current_screen = AppScreen::WalletDashboard; // Or Home
            }
        }
    }

    pub fn create_new_wallet(&mut self) {
        self.clear_error_message();
        let new_wallet = MantraWallet::new_random();
        self.generated_mnemonic = Some(new_wallet.mnemonic_phrase().to_string());
        self.wallet_address = Some(new_wallet.address_str("mantra"));

        self.initialize_dex_client_from_wallet(new_wallet);

        self.current_screen = AppScreen::WalletDashboard;
        self.wallet_balance = Some("0 MNTRA (fetch TODO)".to_string()); // Placeholder
    }

    pub fn import_wallet_from_mnemonic(&mut self) {
        self.clear_error_message();
        let mnemonic = self.mnemonic_input.value();
        if mnemonic.trim().is_empty() {
            self.error_message = Some("Mnemonic cannot be empty.".to_string());
            return;
        }
        match MantraWallet::from_mnemonic(mnemonic, "") {
            Ok(imported_wallet) => {
                self.wallet_address = Some(imported_wallet.address_str("mantra"));
                self.initialize_dex_client_from_wallet(imported_wallet);
                // Check if dex_client initialization failed
                if self.error_message.is_some() {
                    self.current_screen = AppScreen::WalletDashboard; // Or home, if client is critical
                                                                    // but wallet itself is fine
                    return;
                }
                self.current_screen = AppScreen::WalletDashboard;
                self.wallet_balance = Some("0 MNTRA (fetch TODO)".to_string()); // Placeholder
                self.mnemonic_input = Input::default();
                self.generated_mnemonic = None;
            }
            Err(_e) => {
                self.error_message = Some("Invalid mnemonic or failed to import wallet.".to_string());
                // self.current_screen = AppScreen::ImportWallet; // Already on this screen
                // self.mnemonic_input.reset(); // Optionally reset
            }
        }
    }

    // Placeholder for DEX interaction methods
    pub fn fetch_pools(&mut self) { // Made synchronous
        self.clear_error_message();
        if let Some(_client) = &self.dex_client { // _client as it's not used for simulated data
            // In a real app, this would be an async call using `_client`
            // For now, simulate with placeholder data or leave as TODO
            // Simulating a potential error during fetch
            // if some_condition_for_error {
            //     self.error_message = Some("Failed to fetch pools. Network error.".to_string());
            //     return;
            // }
            // match client.query_pools().await {
            //     Ok(pools_response) => self.pools = Some(pools_response.pools),
            //     Err(_) => self.error_message = Some("Failed to fetch pools.".to_string()),
            // }
            // Placeholder data:
            self.pools = Some(vec![
                PoolInfo {
                    pool_id: "pool1".to_string(),
                    assets: vec!["OM".to_string(), "USDT".to_string()],
                    pool_type: mantra_dex_std::pool_manager::PoolType::XYK {},
                    total_share: "1000000".to_string(),
                    //.. other fields if necessary, or use Default::default() if possible
                },
                PoolInfo {
                    pool_id: "pool2".to_string(),
                    assets: vec!["ATOM".to_string(), "MNTRA".to_string()],
                    pool_type: mantra_dex_std::pool_manager::PoolType::XYK {},
                    total_share: "2000000".to_string(),
                },
            ]);
            self.current_screen = AppScreen::ViewPools;
        } else {
            self.error_message = Some("Wallet not connected. Cannot fetch pools.".to_string());
            self.current_screen = AppScreen::WalletDashboard; // Or Home
        }
    }

    pub fn prepare_swap(&mut self) {
        self.clear_error_message();
        // Logic to set asset_in_index, asset_out_index based on user selection in ViewPools
        // For now, assume selected_pool_index is set and we swap asset 0 for asset 1
        if self.pools.is_some() && self.selected_pool_index < self.pools.as_ref().unwrap().len() {
            self.asset_in_index = 0; // Default to first asset in pool
            self.asset_out_index = 1; // Default to second asset in pool
            self.amount_in_input.reset();
            self.amount_out_display = "0.0".to_string();
            self.current_screen = AppScreen::Swap;
        } else {
            self.error_message = Some("No pool selected or pools not loaded.".to_string());
            self.current_screen = AppScreen::ViewPools;
        }
    }

    pub fn calculate_swap_preview(&mut self) {
        // This function is mostly for display, errors handled by input parsing or swap execution
        // self.clear_error_message(); // Not usually needed here unless calc itself can fail
        // TODO: Implement actual swap preview logic using DEX SDK / client
        // For now, a placeholder:
        if let Ok(amount_in) = self.amount_in_input.value().parse::<f64>() {
            self.amount_out_display = format!("{:.6}", amount_in * 0.98); // Simulate 2% fee/slippage
        } else {
            self.amount_out_display = "Invalid input".to_string();
        }
    }

    pub fn execute_swap(&mut self) { // Made synchronous
        self.clear_error_message();
        // TODO: Implement actual swap execution using DEX SDK / client
        // This would involve:
        // 1. Getting the amount from amount_in_input
        // 2. Getting the selected pool and assets
        // 3. Constructing and signing the swap transaction
        // 4. Broadcasting the transaction
        // 5. Handling the response (success or error)
        // For now, simulate success or error:
        if self.amount_in_input.value() == "666" { // Simulate an error condition
            self.error_message = Some("Swap failed: Insufficient funds (simulated error for amount 666).".to_string());
            // self.current_screen = AppScreen::Swap; // Stay on swap screen or go to result with error
            self.current_screen = AppScreen::SwapResult; // Go to result screen to show error
            self.swap_tx_hash = None;
        } else if self.amount_in_input.value().is_empty() || self.amount_in_input.value().parse::<f64>().is_err() {
            self.error_message = Some("Invalid amount for swap.".to_string());
            self.current_screen = AppScreen::Swap; // Stay on swap to correct
            self.swap_tx_hash = None;
        }
        else {
            self.swap_tx_hash = Some("0x123abc_simulated_tx_hash_789xyz".to_string());
            self.current_screen = AppScreen::SwapResult;
        }
    }


    // Example state modification, can be removed if not used
    pub fn increment_counter(&mut self) {
        self.counter += 1;
    }

    pub fn decrement_counter(&mut self) {
        self.counter -= 1;
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        // Note: The main loop is synchronous. Async operations like fetch_pools or execute_swap
        // would need to be handled differently, e.g., by spawning a tokio task and
        // communicating results back to the App, or by integrating with a tokio runtime
        // if the entire TUI is run within one. For this iteration, we'll call them and they'll
        // update state synchronously (simulated for now).

        while self.running {
            terminal.draw(|frame| render_ui(frame, self))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let CrosstermEvent::Key(key) = event::read()? {
                    handle_key_event(key, self);
                }
            }
        }
        Ok(())
    }
}

// Main function to setup and run the TUI application
pub fn start_tui() -> io::Result<()> { // Made synchronous
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let res = app.run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
        return Err(err);
    }

    Ok(())
}
