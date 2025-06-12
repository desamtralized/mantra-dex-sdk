# Wallet Persistence & Encryption Implementation Task List

## AI Coding Agent Instructions

This document contains a comprehensive task list for implementing wallet persistence with encryption in the Mantra DEX SDK TUI. Each task should be completed in order, with the checkbox marked as `[x]` when finished.

**Project Context:**
- Current codebase has a wizard-based setup flow that requires mnemonic input every time
- `WalletStorage` struct in `src/wallet/storage.rs` provides encryption foundation
- Need to modify startup flow to detect saved wallets and show selection screen
- Must maintain security best practices throughout implementation

---

## Phase 1: Foundation & Dependencies (High Priority)

### Task 1.1: Add Missing Encryption Dependencies
- [ ] **Add `aes-gcm` and `argon2` to Cargo.toml**
  - Add `aes-gcm = "0.10"` to dependencies section
  - Add `argon2 = "0.5"` to dependencies section
  - Run `cargo check` to verify dependencies compile correctly
  - Commit changes with message: "Add encryption dependencies for wallet persistence"

### Task 1.2: Verify WalletStorage Integration
- [ ] **Check WalletStorage module organization**
  - Verify `src/wallet/storage.rs` exports are correct in `src/wallet.rs` or `src/lib.rs`
  - Add `pub mod storage;` to `src/wallet.rs` if not present
  - Add `pub use storage::*;` for public API exports
  - Test that `WalletStorage::new()` can be called from main application

### Task 1.3: Create New UI Components Module Structure
- [ ] **Create password input component**
  - Create `src/tui/components/password_input.rs`
  - Implement `PasswordInput` struct with validation
  - Add password strength indicator functionality
  - Include password masking/revealing toggle
  - Export in `src/tui/components/mod.rs`

- [ ] **Create wallet save modal component**
  - Create `src/tui/components/wallet_save_modal.rs`
  - Implement modal with wallet name input and password creation
  - Add form validation and submission handling
  - Include progress indicators for encryption operations
  - Export in `src/tui/components/mod.rs`

---

## Phase 2: Startup Flow Modification (High Priority)

### Task 2.1: Create Wallet Selection Screen
- [ ] **Implement wallet selection screen**
  - Create `src/tui/screens/wallet_selection.rs`
  - Create `WalletSelectionState` struct with wallet list and selection
  - Implement `render_wallet_selection()` function with proper layout
  - Add keyboard navigation (↑/↓ for selection, Enter to confirm)
  - Include "Create New Wallet" and "Recover Wallet" options
  - Export in `src/tui/screens/mod.rs`

### Task 2.2: Add Wallet Selection to App State
- [ ] **Extend AppState for wallet selection**
  - Add `WalletSelectionState` to `AppState` in `src/tui/app.rs`
  - Add `Screen::WalletSelection` variant to `Screen` enum
  - Add wallet selection state initialization in `AppState::default()`
  - Implement focus management for wallet selection screen

### Task 2.3: Modify Application Startup Logic
- [ ] **Update main entry points**
  - Modify `src/bin/main.rs` to check for saved wallets before showing wizard
  - Add wallet detection logic using `WalletStorage::has_saved_wallets()`
  - Set initial screen to `WalletSelection` if wallets exist, otherwise `Wizard`
  - Update `src/bin/tui.rs` with same logic for consistency

### Task 2.4: Implement Wallet Selection Event Handling
- [ ] **Add wallet selection event handling**
  - Extend `handle_event` in `src/tui/app.rs` for `Screen::WalletSelection`
  - Handle Up/Down arrow navigation through wallet list
  - Handle Enter key to select wallet and prompt for password
  - Handle 'n' key to create new wallet (go to wizard)
  - Handle 'r' key to recover existing wallet (go to wizard)
  - Handle Escape key to quit application

---

## Phase 3: Wizard Flow Enhancement (Medium Priority)

### Task 3.1: Add Wallet Save Modal to Wizard
- [ ] **Modify wizard completion flow**
  - Update `WizardStep` enum in `src/tui/screens/wizard.rs` to include `WalletSave` step
  - Add wallet save modal rendering after `SecurityWarning` step
  - Integrate wallet name input and password creation forms
  - Add skip option for users who don't want to save

### Task 3.2: Implement Wallet Save Logic
- [ ] **Connect wizard to wallet storage**
  - Modify `apply_wizard_settings` in `src/tui/app.rs` to call wallet save
  - Use `WalletStorage::save_wallet()` with user-provided name and password
  - Handle encryption errors gracefully with user-friendly messages
  - Show success confirmation when wallet is saved
  - Update wizard state to proceed to completion

### Task 3.3: Add Wallet Save Modal State Management
- [ ] **Extend WizardState for wallet saving**
  - Add wallet save fields to `WizardState` in `src/tui/screens/wizard.rs`
  - Add `wallet_name`, `save_password`, `save_password_confirm` fields
  - Add `show_save_modal` boolean flag
  - Implement validation for wallet name and password strength

---

## Phase 4: Password Authentication (Medium Priority)

### Task 4.1: Create Password Authentication Modal
- [ ] **Implement password prompt modal**
  - Create `src/tui/components/password_prompt.rs`
  - Implement modal with password input and submit/cancel buttons
  - Add password masking and error message display
  - Include retry mechanism for incorrect passwords
  - Export in components module

### Task 4.2: Integrate Password Authentication
- [ ] **Add authentication flow to wallet selection**
  - Modify wallet selection screen to show password prompt on Enter
  - Use `WalletStorage::load_wallet()` with entered password
  - Handle incorrect password errors with retry mechanism
  - Implement 3-attempt limit with temporary lockout
  - Load wallet into application state on successful authentication

### Task 4.3: Add Password Authentication State
- [ ] **Extend app state for authentication**
  - Add password authentication state to `AppState`
  - Track failed attempt count and lockout time
  - Add current authenticating wallet metadata
  - Implement state transitions for authentication flow

---

## Phase 5: UI Enhancements (Medium Priority)

### Task 5.1: Implement Password Strength Indicator
- [ ] **Create password strength component**
  - Create `src/tui/components/password_strength.rs`
  - Implement visual strength indicator (weak/medium/strong)
  - Add real-time validation feedback
  - Include specific requirements display (length, complexity, etc.)
  - Use color coding (red/yellow/green) for strength levels

### Task 5.2: Enhance Wallet Selection UI
- [ ] **Improve wallet selection screen visual design**
  - Add proper borders and styling to wallet list
  - Display wallet address (truncated: first 8 + "..." + last 8 chars)
  - Show last accessed timestamp in human-readable format
  - Add visual indicators for selected wallet
  - Include help text at bottom of screen

### Task 5.3: Add Progress Indicators
- [ ] **Implement loading states**
  - Add spinner/progress indicator for wallet encryption operations
  - Show "Encrypting wallet..." message during save operation
  - Add "Decrypting wallet..." message during load operation
  - Implement non-blocking UI updates during crypto operations

---

## Phase 6: Security Enhancements (Medium Priority)

### Task 6.1: Implement Password Validation
- [ ] **Add comprehensive password validation**
  - Use existing `validate_password()` function from `WalletStorage`
  - Implement minimum 12 character requirement
  - Require uppercase, lowercase, numbers, and symbols
  - Block common passwords (add common passwords list)
  - Add real-time validation feedback in UI

### Task 6.2: Add Security Logging
- [ ] **Implement security event logging**
  - Add failed authentication attempt logging
  - Log successful wallet loads (without sensitive data)
  - Track password strength of saved wallets
  - Implement log rotation and size limits
  - Store logs in `~/.mantra_dex/logs/security.log`

### Task 6.3: Implement Memory Security
- [ ] **Add secure memory management**
  - Clear password fields from memory after use
  - Implement `Drop` trait for sensitive data structures
  - Use `zeroize` crate for secure memory clearing
  - Ensure mnemonic strings are cleared after wallet loading
  - Add `zeroize = "1.7"` dependency to Cargo.toml

---

## Phase 7: Error Handling & Recovery (Low Priority)

### Task 7.1: Enhance Error Messages
- [ ] **Improve error messaging throughout flow**
  - Add user-friendly error messages for all wallet operations
  - Implement contextual help for common issues
  - Add recovery suggestions for each error type
  - Ensure error messages are consistent across components

## Phase 8: Documentation & Polish (Low Priority)

### Task 8.1: Update Documentation
- [ ] **Update project documentation**
  - Update README.md with wallet persistence feature
  - Add security considerations section
  - Document new keyboard shortcuts
  - Add troubleshooting guide for wallet issues

### Task 8.2: Add Help Text
- [ ] **Implement contextual help**
  - Add help text to wallet selection screen
  - Add help text to password creation modal
  - Update existing help system with new shortcuts
  - Add tooltips for password strength requirements

---

## Implementation Notes

### Important Considerations:
1. **Security First**: Never compromise on security for user experience
2. **Backward Compatibility**: Ensure existing users can still use the application
3. **Error Recovery**: Always provide users a way to recover from errors

### Key Files to Monitor:
- `src/tui/app.rs` - Main application state and event handling
- `src/tui/screens/wizard.rs` - Wizard flow modifications
- `src/wallet/storage.rs` - Encryption and storage logic
- `Cargo.toml` - Dependency management
- `src/bin/main.rs` and `src/bin/tui.rs` - Application entry points

---

**Implementation Status**: Not Started  
**Priority**: High (User Experience Critical)  