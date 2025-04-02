use crossterm::event::{KeyCode, KeyEvent};
use mantra_dex_sdk::{MantraDexClient, PoolInfo};
use tui_input::{Input, InputRequest};

use crate::config::TuiConfig;
use crate::error::{Result, TuiError};
use crate::utils::create_client;

pub type AppResult<T> = Result<T>;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TabState {
    Dashboard,
    Pools,
    Swap,
    Liquidity,
    Wallet,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Clone)]
pub struct WalletData {
    pub name: String,
    pub address: String,
    pub balances: Vec<(String, String)>, // (denom, amount)
}

pub struct SwapForm {
    pub pool_id: Input,
    pub offer_asset: Input,
    pub ask_denom: Input,
    pub max_spread: Input,
    pub active_field: usize,
    pub simulate_result: Option<String>,
}

impl Default for SwapForm {
    fn default() -> Self {
        let pool_id = Input::default();
        let offer_asset = Input::default();
        let ask_denom = Input::default();
        let mut max_spread = Input::default();
        
        // Default value for max spread - using individual char insertions
        for c in "0.01".chars() {
            max_spread.handle(InputRequest::InsertChar(c));
        }
        
        Self {
            pool_id,
            offer_asset,
            ask_denom,
            max_spread,
            active_field: 0,
            simulate_result: None,
        }
    }
}

pub struct App {
    pub config: TuiConfig,
    pub client: Option<MantraDexClient>,
    pub tab: TabState,
    pub input_mode: InputMode,
    pub should_quit: bool,
    pub message: Option<String>,
    
    // Wallet data
    pub wallet: Option<WalletData>,
    
    // Pool data
    pub pools: Vec<PoolInfo>,
    pub selected_pool_idx: Option<usize>,
    
    // Swap form
    pub swap_form: SwapForm,
}

impl App {
    pub async fn new(config: &TuiConfig) -> Result<Self> {
        // Create a client if we have an active wallet and password
        let client = if config.active_wallet.is_some() && config.get_session_password().is_some() {
            match create_client(config).await {
                Ok(client) => Some(client),
                Err(_) => None,
            }
        } else {
            None
        };
        
        // Create the app
        let mut app = Self {
            config: config.clone(),
            client,
            tab: TabState::Dashboard,
            input_mode: InputMode::Normal,
            should_quit: false,
            message: None,
            wallet: None,
            pools: Vec::new(),
            selected_pool_idx: None,
            swap_form: SwapForm::default(),
        };
        
        // Initially load data if client is available
        if app.client.is_some() {
            app.load_wallet_data().await?;
            app.load_pools().await?;
        }
        
        Ok(app)
    }
    
    pub fn tick(&mut self) {
        // Update any data that needs to be refreshed periodically
    }
    
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Result<bool> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key),
            InputMode::Editing => self.handle_editing_mode(key),
        }
    }
    
    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return Ok(true);
            },
            KeyCode::Char('1') => self.tab = TabState::Dashboard,
            KeyCode::Char('2') => self.tab = TabState::Pools,
            KeyCode::Char('3') => self.tab = TabState::Swap,
            KeyCode::Char('4') => self.tab = TabState::Liquidity,
            KeyCode::Char('5') => self.tab = TabState::Wallet,
            KeyCode::Char('e') => self.input_mode = InputMode::Editing,
            _ => {}
        }
        
        Ok(false)
    }
    
    fn handle_editing_mode(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            },
            KeyCode::Enter => {
                if self.tab == TabState::Swap {
                    self.simulate_swap()?;
                }
            },
            KeyCode::Tab => {
                if self.tab == TabState::Swap {
                    // Cycle through input fields
                    self.swap_form.active_field = (self.swap_form.active_field + 1) % 4;
                }
            },
            KeyCode::BackTab => {
                if self.tab == TabState::Swap {
                    // Cycle through input fields in reverse
                    self.swap_form.active_field = if self.swap_form.active_field == 0 {
                        3
                    } else {
                        self.swap_form.active_field - 1
                    };
                }
            },
            _ => {
                // Handle input for the active field in swap form
                if self.tab == TabState::Swap {
                    // Convert KeyEvent to InputRequest
                    let input_req = match key_event_to_input_request(key) {
                        Some(req) => req,
                        None => return Ok(false),
                    };
                    
                    match self.swap_form.active_field {
                        0 => {
                            self.swap_form.pool_id.handle(input_req);
                        },
                        1 => {
                            self.swap_form.offer_asset.handle(input_req);
                        },
                        2 => {
                            self.swap_form.ask_denom.handle(input_req);
                        },
                        3 => {
                            self.swap_form.max_spread.handle(input_req);
                        },
                        _ => {}
                    }
                }
            }
        }
        
        Ok(false)
    }
    
    // Function to load wallet data
    pub async fn load_wallet_data(&mut self) -> Result<()> {
        if let Some(client) = &self.client {
            // Get wallet address - handle the Result properly
            let wallet = client.wallet().map_err(|e| TuiError::Wallet(e.to_string()))?;
            let address = wallet.address().map_err(|e| TuiError::Wallet(e.to_string()))?.to_string();
            
            // Get wallet balances (placeholder for actual SDK call)
            let balances = vec![
                ("uom".to_string(), "1000.0".to_string()),
                ("uusdt".to_string(), "500.0".to_string()),
            ];
            
            // Get wallet name
            let name = self.config.active_wallet.clone().unwrap_or_default();
            
            self.wallet = Some(WalletData {
                name,
                address,
                balances,
            });
            
            Ok(())
        } else {
            Err(TuiError::Command("No client available".to_string()))
        }
    }
    
    // Function to load pools
    pub async fn load_pools(&mut self) -> Result<()> {
        if let Some(_client) = &self.client {
            // Get list of pools (placeholder for actual SDK call)
            self.pools = Vec::new();
            // In a real implementation, we would call the SDK to get the pool list
            
            Ok(())
        } else {
            Err(TuiError::Command("No client available".to_string()))
        }
    }
    
    // Function to simulate a swap
    pub fn simulate_swap(&mut self) -> Result<()> {
        if self.client.is_none() {
            self.swap_form.simulate_result = Some("No wallet connected".to_string());
            return Ok(());
        }
        
        let pool_id = self.swap_form.pool_id.value();
        let offer_asset = self.swap_form.offer_asset.value();
        let ask_denom = self.swap_form.ask_denom.value();
        
        if pool_id.is_empty() || offer_asset.is_empty() || ask_denom.is_empty() {
            self.swap_form.simulate_result = Some("Please fill in all required fields".to_string());
            return Ok(());
        }
        
        // In a real implementation, we would call the SDK to simulate the swap
        self.swap_form.simulate_result = Some(format!(
            "Simulated swap: {} → {} (not actually implemented)",
            offer_asset, ask_denom
        ));
        
        Ok(())
    }
}

// Helper function to convert KeyEvent to InputRequest
fn key_event_to_input_request(key: KeyEvent) -> Option<tui_input::InputRequest> {
    use tui_input::InputRequest;
    
    match key.code {
        KeyCode::Char(c) => Some(InputRequest::InsertChar(c)),
        KeyCode::Backspace => Some(InputRequest::DeletePrevChar),
        KeyCode::Delete => Some(InputRequest::DeleteNextChar),
        KeyCode::Left => Some(InputRequest::GoToPrevChar),
        KeyCode::Right => Some(InputRequest::GoToNextChar),
        KeyCode::Home => Some(InputRequest::GoToStart),
        KeyCode::End => Some(InputRequest::GoToEnd),
        _ => None,
    }
} 