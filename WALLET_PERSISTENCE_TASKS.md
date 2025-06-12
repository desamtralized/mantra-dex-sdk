# Wallet Persistence & Security Feature - Task List

## Phase 1: Foundation & Core Infrastructure
- [ ] **Investigate current wallet/mnemonic handling code** - Locate existing wallet setup wizard and mnemonic entry logic
- [ ] **Add encryption dependencies** - Add `aes-gcm`, `argon2`, and `serde` crates to Cargo.toml
- [ ] **Create wallet storage module** - Implement `src/wallet/storage.rs` with encryption/decryption functions
- [ ] **Define wallet file format** - Create structs for encrypted wallet data and metadata
- [ ] **Implement secure file operations** - Functions to save/load encrypted wallet files with proper permissions

## Phase 2: Modify Existing Wallet Setup Flow
- [ ] **Add save wallet modal** - Create modal component asking "Save Wallet for Future Use?" after mnemonic entry
- [ ] **Create wallet naming input** - Add text input for wallet label/name
- [ ] **Implement password creation flow** - Create password input with confirmation and strength indicator
- [ ] **Integrate encryption on save** - Connect wallet setup completion to encryption and storage
- [ ] **Add progress indicators** - Show loading states during encryption/save operations

## Phase 3: Startup Screen & Wallet Selection
- [ ] **Create startup screen** - New screen that appears before the current wizard
- [ ] **Implement wallet discovery** - Scan for existing wallet files and display them
- [ ] **Create wallet selection list** - UI component to choose from saved wallets
- [ ] **Build password prompt modal** - Modal for entering password to decrypt selected wallet
- [ ] **Add wallet loading logic** - Decrypt and load selected wallet into app state

## Phase 4: Security & Error Handling
- [ ] **Implement password validation** - Check password strength and requirements
- [ ] **Add failed attempt handling** - Rate limiting and temporary lockouts
- [ ] **Secure memory management** - Clear sensitive data from memory after use
- [ ] **Handle corrupted wallet files** - Graceful error handling with recovery options
- [ ] **Add file integrity checks** - Verify wallet file hasn't been tampered with

## Phase 5: Multiple Wallet Management
- [ ] **Support multiple saved wallets** - Allow users to save and manage multiple wallets
- [ ] **Add wallet deletion feature** - Secure deletion with confirmation prompts
- [ ] **Create wallet management screen** - UI for viewing, renaming, and deleting saved wallets
- [ ] **Implement wallet metadata display** - Show creation dates and labels in wallet list

## Phase 6: UX Polish & Advanced Features
- [ ] **Add keyboard shortcuts** - Quick access to wallet operations
- [ ] **Implement backup warnings** - Remind users to backup wallet files and passwords
- [ ] **Add help text and tooltips** - Context-sensitive guidance throughout the flow
- [ ] **Create configuration options** - Allow users to customize storage location
- [ ] **Add comprehensive logging** - Log operations without exposing sensitive data

---

## Instructions for AI Agent
1. Work on ONE task at a time, starting with Phase 1, Task 1
2. When you complete a task, update the checkbox from `[ ]` to `[x]`
3. Edit this file to track your progress
4. Only move to the next task after the current one is fully completed and tested
5. If you encounter blockers, document them in the task description before moving on 