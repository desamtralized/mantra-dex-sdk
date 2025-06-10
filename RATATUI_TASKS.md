# MANTRA DEX SDK TUI Module - Comprehensive Development Plan

## Phase 1: Project Setup and Dependencies

### Task 1.1: Create TUI Module Structure
- [x] Create `src/tui/` directory with modular structure:
  - `src/tui/mod.rs` - Main TUI module
  - `src/tui/app.rs` - Application state management
  - `src/tui/ui.rs` - UI rendering logic
  - `src/tui/events.rs` - Event handling
  - `src/tui/components/` - Reusable UI components
  - `src/tui/screens/` - Different screen implementations

### Task 1.2: Update Dependencies
- [x] Add ratatui 0.29.0 to `Cargo.toml` with required features:
  ```toml
  ratatui = { version = "0.29.0", features = ["crossterm", "all-widgets"] }
  crossterm = "0.28.1"
  ```
- [x] Add supporting dependencies for TUI functionality:
  - `tokio-util` for async utilities
  - `tui-input` for text input handling
  - `chrono` for date/time formatting

### Task 1.3: Export TUI Module
- [x] Update `src/lib.rs` to export the TUI module
- [x] Add feature flag `tui` to make TUI optional

## Phase 2: Core TUI Infrastructure

### Task 2.1: Application State Management (`src/tui/app.rs`)
- [x] Define `AppState` struct with:
  - Current screen/view
  - Selected pool information
  - User balances cache
  - Transaction history
  - Error/status messages
  - Loading states
- [x] Implement state transitions between screens
- [x] Add methods for updating state from async operations

### Task 2.2: Event System (`src/tui/events.rs`)
- [x] Create `EventHandler` for keyboard/mouse events
- [x] Implement event types:
  - Navigation events (Tab, Enter, Esc, Arrow keys)
  - Input events (typing, backspace)
  - Action events (execute swap, provide liquidity)
- [x] Add async event processing for blockchain operations

### Task 2.3: Terminal Management
- [x] Implement terminal initialization and cleanup
- [x] Add panic handler for graceful terminal restoration
- [x] Create main TUI entry point function

## Phase 3: Core UI Components

### Task 3.1: Layout Components (`src/tui/components/`)
- [x] **Header Component** - Display:
  - MANTRA DEX SDK logo/title
  - Network status (mainnet/testnet)
  - Connected wallet address
  - Current block height
- [x] **Navigation Menu** - Tab-based navigation:
  - Dashboard, Pools, Swap, Liquidity, Rewards, Admin
- [x] **Status Bar** - Bottom status with:
  - Current action status
  - Error messages
  - Loading indicators
- [x] **Modal/Popup Component** for confirmations and details

### Task 3.2: Data Display Components
- [x] **Balance Table** - Display user token balances
- [x] **Pool Info Card** - Show pool details with:
  - Pool ID, assets, liquidity, fees
  - Pool status (Available/Disabled)
  - Pool type and features
- [x] **Transaction Table** - Recent transactions with status
- [x] **Progress Bar** for loading states

### Task 3.3: Input Components  
- [x] **Text Input** with validation for:
  - Addresses, amounts, pool IDs
- [x] **Dropdown/Select** for:
  - Pool selection, token selection
- [x] **Checkbox/Toggle** for:
  - Feature toggles, confirmations

## Phase 4: Screen Implementations

### Task 4.1: Dashboard Screen (`src/tui/screens/dashboard.rs`)
- [x] **Overview Panel**:
  - Total portfolio value
  - Active positions
  - Recent activity summary
- [x] **Quick Stats**:
  - Total pools available
  - Network status
  - Current epoch information
- [x] **Recent Transactions** list
- [x] **Network Health** indicators

### Task 4.2: Pools Screen (`src/tui/screens/pools.rs`)
- [x] **Pool List Table** with columns:
  - Pool ID, Asset Pair, TVL, APY, Status
  - Sortable by different criteria
- [x] **Pool Details Panel** (when pool selected):
  - Complete pool information
  - Liquidity composition
  - Fee structure breakdown
- [x] **Pool Search/Filter** functionality
- [x] **Pool Status** indicators with color coding

### Task 4.3: Swap Screen (`src/tui/screens/swap.rs`)
- [x] **Swap Interface**:
  - "From" token input with balance display
  - "To" token selection
  - Pool selection dropdown
  - Slippage tolerance setting
- [x] **Swap Preview**:
  - Expected output amount
  - Price impact calculation
  - Fee breakdown
- [x] **Simulation Results** display
- [x] **Execute Swap** with confirmation modal

### Task 4.4: Liquidity Screen (`src/tui/screens/liquidity.rs`)
- [ ] **Provide Liquidity Panel**:
  - Dual asset input fields
  - Pool selection
  - Slippage settings (liquidity + swap)
  - Expected LP tokens
- [ ] **Withdraw Liquidity Panel**:
  - LP token amount input
  - Expected asset outputs
- [ ] **Current Positions** table showing:
  - Pool positions, LP tokens, estimated value
- [ ] **Position Details** with PnL calculations

### Task 4.5: Rewards Screen (`src/tui/screens/rewards.rs`)
- [ ] **Rewards Dashboard**:
  - Total claimable rewards
  - Rewards by pool/epoch
  - Current epoch information
- [ ] **Claim Interface**:
  - Claim all rewards option
  - Claim until specific epoch
  - Epoch selection input
- [ ] **Rewards History** table
- [ ] **Epoch Timeline** visualization

### Task 4.6: Multi-hop Swap Screen (`src/tui/screens/multihop.rs`)
- [ ] **Swap Route Builder**:
  - Add/remove swap operations
  - Route visualization
  - Optimal path suggestions
- [ ] **Route Analysis**:
  - Total price impact
  - Fee breakdown per hop
  - Estimated output
- [ ] **Execute Multi-hop** with confirmation

## Phase 5: Admin/Advanced Features

### Task 5.1: Admin Screen (`src/tui/screens/admin.rs`)
- [ ] **Pool Management Panel**:
  - Create new pools interface
  - Pool feature toggles (deposits, withdrawals, swaps)
  - Fee structure editor
- [ ] **Pool Feature Controls**:
  - Enable/disable operations per pool
  - Global feature updates
- [ ] **Pool Creation Wizard**:
  - Asset selection
  - Decimals configuration
  - Fee structure setup with validation
  - Pool type selection

### Task 5.2: Settings Screen (`src/tui/screens/settings.rs`)
- [ ] **Network Configuration**:
  - Switch between mainnet/testnet
  - Custom RPC endpoint
- [ ] **Wallet Management**:
  - Import from mnemonic
  - Display current address
  - Export/backup options
- [ ] **Display Preferences**:
  - Theme selection
  - Refresh intervals
  - Decimal precision

### Task 5.3: Transaction Details Screen
- [ ] **Transaction Viewer**:
  - Transaction hash, status, block height
  - Gas used, fees paid
  - Event logs and messages
- [ ] **Transaction History** with filtering
- [ ] **Export** transaction data

## Phase 6: Advanced UI Features

### Task 6.1: Real-time Updates
- [ ] **Auto-refresh** for:
  - Balances every 30 seconds
  - Pool data every 60 seconds
  - Transaction status checks
- [ ] **Live Price Updates** where applicable
- [ ] **Background data syncing** without blocking UI

### Task 6.2: Error Handling & User Experience
- [ ] **Comprehensive Error Display**:
  - Network errors with retry options
  - Validation errors with guidance
  - Transaction failures with explanations
- [ ] **Loading States** for all async operations
- [ ] **Confirmation Dialogs** for all transactions
- [ ] **Help/Documentation** overlay with keyboard shortcuts

### Task 6.3: Data Visualization
- [ ] **Progress Bars** for:
  - Transaction confirmation progress
  - Sync status

## Phase 7: Integration & Testing

### Task 7.1: SDK Integration
- [ ] **Async Integration**:
  - Non-blocking UI during blockchain operations
  - Proper error propagation from SDK
  - Status updates during long operations
- [ ] **State Synchronization**:
  - Keep UI state in sync with blockchain state
  - Handle network disconnections gracefully

### Task 7.2: Testing Framework
- [ ] **Unit Tests** for:
  - State management logic
  - Event handling

### Task 7.3: Documentation
- [ ] **Documentation** with screenshots and usage examples

### Task 8.1: Accessibility & Usability
- [ ] **Keyboard Navigation**:
  - Consistent key bindings across screens
  - Tab order for form inputs
  - Escape key handling
- [ ] **Color Scheme**:
  - Support for different terminal color capabilities
  - High contrast mode
- [ ] **Responsive Layout**:
  - Adapt to different terminal sizes
  - Minimum size requirements

### Task 8.2: Documentation
- [ ] **User Guide** with instructions
- [ ] **README** with setup and usage instructions



## File Structure Summary

```
src/
├── tui/
│   ├── mod.rs              # Main TUI module exports
│   ├── app.rs              # Application state management  
│   ├── ui.rs               # Main UI rendering coordinator
│   ├── events.rs           # Event handling system
│   ├── components/         # Reusable UI components
│   │   ├── mod.rs
│   │   ├── header.rs       # Header with wallet/network info
│   │   ├── navigation.rs   # Tab-based navigation menu
│   │   ├── status_bar.rs   # Bottom status bar
│   │   ├── tables.rs       # Data tables (pools, transactions)
│   │   ├── forms.rs        # Input forms and validation
│   │   ├── modals.rs       # Popup dialogs and confirmations
│   │   └── charts.rs       # Data visualization components
│   ├── screens/            # Full-screen implementations
│   │   ├── mod.rs
│   │   ├── dashboard.rs    # Overview and summary
│   │   ├── pools.rs        # Pool listing and details
│   │   ├── swap.rs         # Single swap interface
│   │   ├── multihop.rs     # Multi-hop swap builder
│   │   ├── liquidity.rs    # Provide/withdraw liquidity
│   │   ├── rewards.rs      # Rewards claiming and history
│   │   ├── admin.rs        # Pool management and admin
│   │   ├── settings.rs     # Configuration and preferences
│   │   └── transaction.rs  # Transaction details viewer
│   └── utils/              # TUI utility functions
│       ├── mod.rs
│       ├── formatting.rs   # Number and text formatting
│       ├── validation.rs   # Input validation helpers
│       └── async_ops.rs    # Async operation helpers
```

This comprehensive plan covers all the major features of your MANTRA DEX SDK and provides a complete TUI interface using ratatui 0.29.0. The modular structure allows for incremental development and testing of individual components.
