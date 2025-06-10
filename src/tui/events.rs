//! Event Handling System
//!
//! This module manages keyboard and mouse events for the TUI application,
//! providing a structured way to handle user input and system events.

#[cfg(feature = "tui")]
use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};

#[cfg(feature = "tui")]
use std::time::Duration;
#[cfg(feature = "tui")]
use tokio::sync::mpsc;

/// Application events that can be handled
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Quit the application
    Quit,
    /// Navigate to next tab
    Tab,
    /// Navigate to previous tab (Shift+Tab)
    BackTab,
    /// Enter/confirm action
    Enter,
    /// Escape/cancel action
    Escape,
    /// Arrow key navigation
    Up,
    Down,
    Left,
    Right,
    /// Character input
    Char(char),
    /// Backspace key
    Backspace,
    /// Delete key
    Delete,
    /// Home key
    Home,
    /// End key
    End,
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Insert key
    Insert,
    /// Function keys
    F(u8),
    /// Ctrl+key combinations
    Ctrl(char),
    /// Alt+key combinations
    Alt(char),
    /// Refresh/reload action (typically F5)
    Refresh,
    /// Help action (typically F1)
    Help,
    /// Mouse events (placeholder for future implementation)
    Mouse,
    /// Custom application events
    Custom(String),

    // === DEX-specific Action Events ===
    /// Execute a swap operation
    ExecuteSwap {
        from_asset: String,
        to_asset: String,
        amount: String,
        pool_id: Option<u64>,
        slippage_tolerance: Option<String>,
    },
    /// Provide liquidity to a pool
    ProvideLiquidity {
        pool_id: u64,
        asset_1_amount: String,
        asset_2_amount: String,
        slippage_tolerance: Option<String>,
    },
    /// Withdraw liquidity from a pool
    WithdrawLiquidity {
        pool_id: u64,
        lp_token_amount: String,
        slippage_tolerance: Option<String>,
    },
    /// Claim rewards for specific epochs
    ClaimRewards {
        pool_id: Option<u64>,
        epochs: Option<Vec<u64>>,
        claim_all: bool,
    },
    /// Execute multi-hop swap
    ExecuteMultiHopSwap {
        operations: Vec<SwapOperation>,
    },
    /// Create a new pool (admin)
    CreatePool {
        asset_1: String,
        asset_2: String,
        swap_fee: String,
        exit_fee: String,
        pool_features: Vec<String>,
    },
    /// Update pool features (admin)
    UpdatePoolFeatures {
        pool_id: u64,
        features: Vec<String>,
        enabled: bool,
    },
    /// Simulate swap to get preview
    SimulateSwap {
        from_asset: String,
        to_asset: String,
        amount: String,
        pool_id: Option<u64>,
    },
    /// Simulate liquidity provision
    SimulateLiquidity {
        pool_id: u64,
        asset_1_amount: String,
        asset_2_amount: String,
    },

    // === Async Blockchain Events ===
    /// Blockchain operation completed successfully
    BlockchainSuccess {
        operation: String,
        result: String,
        transaction_hash: Option<String>,
    },
    /// Blockchain operation failed
    BlockchainError {
        operation: String,
        error: String,
    },
    /// Blockchain operation is in progress
    BlockchainProgress {
        operation: String,
        status: String,
        progress: Option<f32>, // 0.0 to 1.0
    },
    /// Data refresh completed
    DataRefresh {
        data_type: String,
        success: bool,
        error: Option<String>,
    },
}

/// Swap operation for multi-hop swaps
#[derive(Debug, Clone, PartialEq)]
pub struct SwapOperation {
    pub from_asset: String,
    pub to_asset: String,
    pub pool_id: u64,
    pub amount: String,
}

/// Event handler for processing terminal events
pub struct EventHandler {
    /// Receiver for events
    receiver: mpsc::UnboundedReceiver<Event>,
    /// Sender for events (for custom events)
    sender: mpsc::UnboundedSender<Event>,
    /// Handle for the background terminal event processing task
    _terminal_task: tokio::task::JoinHandle<()>,
}

/// Async blockchain operation processor
pub struct AsyncBlockchainProcessor {
    /// Event sender to communicate with the main event loop
    event_sender: mpsc::UnboundedSender<Event>,
}

impl AsyncBlockchainProcessor {
    /// Create a new async blockchain processor
    pub fn new(event_sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { event_sender }
    }

    /// Execute a swap operation asynchronously
    pub async fn execute_swap(
        &self,
        from_asset: String,
        to_asset: String,
        amount: String,
        _pool_id: Option<u64>,
        _slippage_tolerance: Option<String>,
    ) {
        let operation = "swap".to_string();

        // Send progress event
        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Initiating swap...".to_string(),
            progress: Some(0.1),
        });

        // TODO: Replace with actual SDK call
        // For now, simulate async operation
        tokio::time::sleep(Duration::from_millis(500)).await;

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Broadcasting transaction...".to_string(),
            progress: Some(0.5),
        });

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Confirming transaction...".to_string(),
            progress: Some(0.8),
        });

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate success/failure (in real implementation, check actual SDK result)
        let success = true; // TODO: Replace with actual SDK call result

        if success {
            let _ = self.event_sender.send(Event::BlockchainSuccess {
                operation: operation.clone(),
                result: format!(
                    "Swapped {} {} for {} {}",
                    amount, from_asset, "calculated_amount", to_asset
                ),
                transaction_hash: Some("0x1234567890abcdef...".to_string()),
            });
        } else {
            let _ = self.event_sender.send(Event::BlockchainError {
                operation: operation.clone(),
                error: "Insufficient liquidity in pool".to_string(),
            });
        }
    }

    /// Provide liquidity to a pool asynchronously
    pub async fn provide_liquidity(
        &self,
        pool_id: u64,
        _asset_1_amount: String,
        _asset_2_amount: String,
        _slippage_tolerance: Option<String>,
    ) {
        let operation = "provide_liquidity".to_string();

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Calculating LP tokens...".to_string(),
            progress: Some(0.2),
        });

        tokio::time::sleep(Duration::from_millis(800)).await;

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Broadcasting transaction...".to_string(),
            progress: Some(0.6),
        });

        tokio::time::sleep(Duration::from_millis(1200)).await;

        let success = true; // TODO: Replace with actual SDK call result

        if success {
            let _ = self.event_sender.send(Event::BlockchainSuccess {
                operation: operation.clone(),
                result: format!(
                    "Added liquidity to pool {}. LP tokens received: calculated_amount",
                    pool_id
                ),
                transaction_hash: Some("0xabcdef1234567890...".to_string()),
            });
        } else {
            let _ = self.event_sender.send(Event::BlockchainError {
                operation: operation.clone(),
                error: "Failed to provide liquidity: slippage tolerance exceeded".to_string(),
            });
        }
    }

    /// Withdraw liquidity from a pool asynchronously
    pub async fn withdraw_liquidity(
        &self,
        pool_id: u64,
        lp_token_amount: String,
        _slippage_tolerance: Option<String>,
    ) {
        let operation = "withdraw_liquidity".to_string();

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Calculating withdrawal amounts...".to_string(),
            progress: Some(0.3),
        });

        tokio::time::sleep(Duration::from_millis(600)).await;

        let success = true; // TODO: Replace with actual SDK call result

        if success {
            let _ = self.event_sender.send(Event::BlockchainSuccess {
                operation: operation.clone(),
                result: format!(
                    "Withdrew {} LP tokens from pool {}. Assets received: calculated_amounts",
                    lp_token_amount, pool_id
                ),
                transaction_hash: Some("0xfedcba0987654321...".to_string()),
            });
        } else {
            let _ = self.event_sender.send(Event::BlockchainError {
                operation: operation.clone(),
                error: "Withdrawal failed: insufficient LP tokens".to_string(),
            });
        }
    }

    /// Claim rewards asynchronously
    pub async fn claim_rewards(
        &self,
        pool_id: Option<u64>,
        _epochs: Option<Vec<u64>>,
        claim_all: bool,
    ) {
        let operation = "claim_rewards".to_string();

        let _ = self.event_sender.send(Event::BlockchainProgress {
            operation: operation.clone(),
            status: "Calculating claimable rewards...".to_string(),
            progress: Some(0.4),
        });

        tokio::time::sleep(Duration::from_millis(400)).await;

        let success = true; // TODO: Replace with actual SDK call result

        if success {
            let result = if claim_all {
                "Claimed all available rewards".to_string()
            } else if let Some(pool) = pool_id {
                format!("Claimed rewards from pool {}", pool)
            } else {
                "Claimed rewards for specified epochs".to_string()
            };

            let _ = self.event_sender.send(Event::BlockchainSuccess {
                operation: operation.clone(),
                result,
                transaction_hash: Some("0x1111222233334444...".to_string()),
            });
        } else {
            let _ = self.event_sender.send(Event::BlockchainError {
                operation: operation.clone(),
                error: "No rewards available to claim".to_string(),
            });
        }
    }

    /// Refresh data from blockchain
    pub async fn refresh_data(&self, data_type: String) {
        tokio::time::sleep(Duration::from_millis(300)).await;

        let success = true; // TODO: Replace with actual SDK call result (90% success rate in real implementation)

        let _ = self.event_sender.send(Event::DataRefresh {
            data_type: data_type.clone(),
            success,
            error: if success {
                None
            } else {
                Some(format!("Failed to refresh {}", data_type))
            },
        });
    }
}

impl EventHandler {
    /// Create a new event handler
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        // Spawn a task to handle terminal events
        let event_sender = sender.clone();
        let terminal_task = tokio::spawn(async move {
            loop {
                // Poll for events with a timeout to avoid blocking
                if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    if let Ok(terminal_event) = event::read() {
                        if let Some(app_event) = Self::convert_terminal_event(terminal_event) {
                            if event_sender.send(app_event).is_err() {
                                break; // Channel closed, exit the loop
                            }
                        }
                    }
                }

                // Small delay to prevent high CPU usage
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        Self {
            receiver,
            sender,
            _terminal_task: terminal_task,
        }
    }

    /// Get the next event
    pub async fn next(&mut self) -> Result<Event, Box<dyn std::error::Error + Send + Sync>> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| "Event channel closed".into())
    }

    /// Convert a terminal event to an application event
    fn convert_terminal_event(terminal_event: event::Event) -> Option<Event> {
        match terminal_event {
            event::Event::Key(key_event) => Self::convert_key_event(key_event),
            event::Event::Mouse(_) => Some(Event::Mouse),
            event::Event::Resize(_, _) => None, // Handle resize events if needed
            _ => None,
        }
    }

    /// Convert a key event to an application event
    fn convert_key_event(key_event: KeyEvent) -> Option<Event> {
        match key_event {
            // Quit events
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Quit),

            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => Some(Event::Quit),

            // Tab navigation
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Tab),

            KeyEvent {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::SHIFT,
                ..
            } => Some(Event::BackTab),

            // Action keys
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Enter),

            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Escape),

            // Arrow keys
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Up),

            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Down),

            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Left),

            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Right),

            // Editing keys
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Backspace),

            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Delete),

            KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Home),

            KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::End),

            KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::PageUp),

            KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::PageDown),

            KeyEvent {
                code: KeyCode::Insert,
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Insert),

            // Function keys
            KeyEvent {
                code: KeyCode::F(n),
                modifiers: KeyModifiers::NONE,
                ..
            } => match n {
                1 => Some(Event::Help),
                5 => Some(Event::Refresh),
                _ => Some(Event::F(n)),
            },

            // Character input
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE,
                ..
            } => Some(Event::Char(c)),

            // Ctrl + character combinations
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                // Skip 'c' since it's already handled as quit
                if c != 'c' {
                    Some(Event::Ctrl(c))
                } else {
                    None
                }
            }

            // Alt + character combinations
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::ALT,
                ..
            } => Some(Event::Alt(c)),

            // Ignore other key combinations
            _ => None,
        }
    }

    /// Send a custom event
    pub fn send_custom_event(&self, event: Event) -> Result<(), mpsc::error::SendError<Event>> {
        self.sender.send(event)
    }

    /// Get an async blockchain processor for performing blockchain operations
    pub fn get_blockchain_processor(&self) -> AsyncBlockchainProcessor {
        AsyncBlockchainProcessor::new(self.sender.clone())
    }

    /// Process a DEX action event by spawning an async operation
    pub fn process_action_event(&self, event: Event) {
        let processor = self.get_blockchain_processor();

        match event {
            Event::ExecuteSwap {
                from_asset,
                to_asset,
                amount,
                pool_id,
                slippage_tolerance,
            } => {
                tokio::spawn(async move {
                    processor
                        .execute_swap(from_asset, to_asset, amount, pool_id, slippage_tolerance)
                        .await;
                });
            }
            Event::ProvideLiquidity {
                pool_id,
                asset_1_amount,
                asset_2_amount,
                slippage_tolerance,
            } => {
                tokio::spawn(async move {
                    processor
                        .provide_liquidity(
                            pool_id,
                            asset_1_amount,
                            asset_2_amount,
                            slippage_tolerance,
                        )
                        .await;
                });
            }
            Event::WithdrawLiquidity {
                pool_id,
                lp_token_amount,
                slippage_tolerance,
            } => {
                tokio::spawn(async move {
                    processor
                        .withdraw_liquidity(pool_id, lp_token_amount, slippage_tolerance)
                        .await;
                });
            }
            Event::ClaimRewards {
                pool_id,
                epochs,
                claim_all,
            } => {
                tokio::spawn(async move {
                    processor.claim_rewards(pool_id, epochs, claim_all).await;
                });
            }
            Event::Refresh => {
                tokio::spawn(async move {
                    processor.refresh_data("all".to_string()).await;
                });
            }
            _ => {
                // Send the event directly for non-blockchain actions
                let _ = self.send_custom_event(event);
            }
        }
    }

    /// Check if an event is a blockchain action that requires async processing
    pub fn is_blockchain_action(event: &Event) -> bool {
        matches!(
            event,
            Event::ExecuteSwap { .. }
                | Event::ProvideLiquidity { .. }
                | Event::WithdrawLiquidity { .. }
                | Event::ClaimRewards { .. }
                | Event::ExecuteMultiHopSwap { .. }
                | Event::CreatePool { .. }
                | Event::UpdatePoolFeatures { .. }
                | Event::SimulateSwap { .. }
                | Event::SimulateLiquidity { .. }
        )
    }

    /// Check if an event is a blockchain response
    pub fn is_blockchain_response(event: &Event) -> bool {
        matches!(
            event,
            Event::BlockchainSuccess { .. }
                | Event::BlockchainError { .. }
                | Event::BlockchainProgress { .. }
                | Event::DataRefresh { .. }
        )
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_conversion() {
        // Test quit events
        let quit_q = KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(EventHandler::convert_key_event(quit_q), Some(Event::Quit));

        let quit_ctrl_c = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(
            EventHandler::convert_key_event(quit_ctrl_c),
            Some(Event::Quit)
        );

        // Test navigation
        let tab = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(EventHandler::convert_key_event(tab), Some(Event::Tab));

        // Test character input
        let char_a = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            kind: event::KeyEventKind::Press,
            state: event::KeyEventState::NONE,
        };
        assert_eq!(
            EventHandler::convert_key_event(char_a),
            Some(Event::Char('a'))
        );
    }

    #[test]
    fn test_blockchain_action_detection() {
        // Test DEX action events
        let swap_event = Event::ExecuteSwap {
            from_asset: "USDC".to_string(),
            to_asset: "OM".to_string(),
            amount: "100".to_string(),
            pool_id: Some(1),
            slippage_tolerance: Some("0.01".to_string()),
        };
        assert!(EventHandler::is_blockchain_action(&swap_event));

        let liquidity_event = Event::ProvideLiquidity {
            pool_id: 1,
            asset_1_amount: "100".to_string(),
            asset_2_amount: "50".to_string(),
            slippage_tolerance: Some("0.01".to_string()),
        };
        assert!(EventHandler::is_blockchain_action(&liquidity_event));

        // Test non-blockchain events
        let quit_event = Event::Quit;
        assert!(!EventHandler::is_blockchain_action(&quit_event));

        let char_event = Event::Char('a');
        assert!(!EventHandler::is_blockchain_action(&char_event));
    }

    #[test]
    fn test_blockchain_response_detection() {
        // Test blockchain response events
        let success_event = Event::BlockchainSuccess {
            operation: "swap".to_string(),
            result: "Success".to_string(),
            transaction_hash: Some("0x123".to_string()),
        };
        assert!(EventHandler::is_blockchain_response(&success_event));

        let error_event = Event::BlockchainError {
            operation: "swap".to_string(),
            error: "Failed".to_string(),
        };
        assert!(EventHandler::is_blockchain_response(&error_event));

        let progress_event = Event::BlockchainProgress {
            operation: "swap".to_string(),
            status: "In progress".to_string(),
            progress: Some(0.5),
        };
        assert!(EventHandler::is_blockchain_response(&progress_event));

        // Test non-response events
        let quit_event = Event::Quit;
        assert!(!EventHandler::is_blockchain_response(&quit_event));
    }

    #[test]
    fn test_swap_operation() {
        let swap_op = SwapOperation {
            from_asset: "USDC".to_string(),
            to_asset: "OM".to_string(),
            pool_id: 1,
            amount: "100".to_string(),
        };

        assert_eq!(swap_op.from_asset, "USDC");
        assert_eq!(swap_op.to_asset, "OM");
        assert_eq!(swap_op.pool_id, 1);
        assert_eq!(swap_op.amount, "100");
    }

    #[tokio::test]
    async fn test_async_blockchain_processor() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let processor = AsyncBlockchainProcessor::new(sender);

        // Test that async operations send events
        tokio::spawn(async move {
            processor.refresh_data("pools".to_string()).await;
        });

        // Should receive a DataRefresh event
        if let Some(event) = receiver.recv().await {
            match event {
                Event::DataRefresh { data_type, .. } => {
                    assert_eq!(data_type, "pools");
                }
                _ => panic!("Expected DataRefresh event"),
            }
        }
    }
}
