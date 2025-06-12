# 🔐 AI Coder Task: Implement Wallet Persistence with Encryption

**Project:** Mantra DEX SDK TUI - Add encrypted wallet saving and loading

**Goal:** Enable users to save their wallet after first-time setup and automatically load it on subsequent launches, eliminating the need to re-enter mnemonics every time.

## Quick Start Instructions

1. **Read the detailed requirements**: `WALLET_PERSISTENCE_PRD.md`
2. **Follow the task list**: `WALLET_PERSISTENCE_IMPLEMENTATION_TASKS.md` 
3. **Work through tasks sequentially**, marking completed tasks with `[x]`

## Key Implementation Points

### 🚀 **Start Here (Phase 1)**
- Add missing dependencies: `aes-gcm = "0.10"` and `argon2 = "0.5"` to `Cargo.toml`
- Verify `WalletStorage` exports in `src/wallet.rs`
- Create password input and wallet save modal components

### 🔄 **Critical Flow Changes (Phase 2)** 
- Create `src/tui/screens/wallet_selection.rs` for startup wallet selection
- Modify app startup in `src/bin/main.rs` to check for saved wallets first
- Add `Screen::WalletSelection` variant and state management

### 🎯 **Core Features (Phases 3-4)**
- Add wallet save modal to wizard completion flow
- Implement password authentication for saved wallets
- Connect `WalletStorage::save_wallet()` and `WalletStorage::load_wallet()` to UI

## Success Criteria
- ✅ New users can save wallet after setup with password
- ✅ Returning users see wallet selection screen on startup  
- ✅ Password authentication works securely
- ✅ Users can still create new wallets when saved wallets exist

## Security Requirements
- 🔒 Use existing AES-256-GCM + Argon2 encryption (already implemented)
- �� Clear sensitive data from memory after use
- 🔒 Validate password strength (minimum 12 chars, complexity)
- 🔒 Implement 3-attempt limit with lockout

## Architecture Notes
- **Existing**: `WalletStorage` struct provides encryption foundation
- **Current Flow**: Wizard runs every startup → needs conditional startup logic
- **New Flow**: Check saved wallets → selection screen OR wizard
- **Key Files**: `src/tui/app.rs`, `src/tui/screens/wizard.rs`, `src/wallet/storage.rs`

**Work through the task list systematically. Each phase builds on the previous one. Focus on security and user experience throughout implementation.**
