# Mantra DEX SDK - Wallet Persistence & Encryption Feature

## Product Requirements Document (PRD)

**Version:** 1.0  
**Date:** December 2024  
**Author:** AI Development Team  

---

## 1. Executive Summary

This feature enables users to securely save their wallets after first-time setup and automatically load them on subsequent application launches. The feature provides encrypted storage using industry-standard AES-256-GCM encryption with Argon2 key derivation, eliminating the need for users to re-enter mnemonics every time they use the application.

## 2. Problem Statement

Currently, users must:
- Complete the setup wizard on every application launch
- Re-enter their mnemonic phrase each time
- Risk security exposure by keeping mnemonic phrases in easily accessible locations
- Experience friction that reduces application usability

This creates a poor user experience and potential security risks.

## 3. Objectives

### Primary Objectives
- **User Experience**: Reduce friction by allowing one-time wallet setup
- **Security**: Implement military-grade encryption for mnemonic storage
- **Convenience**: Auto-load previously saved wallets on startup
- **Choice**: Allow users to create/recover new wallets even with saved wallets

### Success Metrics
- Users can save wallets with 100% success rate
- Encrypted wallet files are unreadable without correct password
- Application startup time with saved wallets < 2 seconds
- Zero security vulnerabilities in encryption implementation

## 4. User Stories

### Primary User Stories

**US-001: First-Time Wallet Save**
- **As a** new user completing wallet setup for the first time
- **I want to** be prompted to save my wallet with a password
- **So that** I don't need to re-enter my mnemonic on future launches

**US-002: Returning User Authentication**
- **As a** returning user with a saved wallet
- **I want to** enter my password to unlock my saved wallet
- **So that** I can quickly access the application

**US-003: Multiple Wallet Management**
- **As a** user with multiple wallets
- **I want to** see a list of my saved wallets and choose which one to use
- **So that** I can easily switch between different wallets

**US-004: New Wallet Creation with Existing Wallets**
- **As a** user with existing saved wallets
- **I want to** create or recover a new wallet
- **So that** I can manage multiple accounts

### Secondary User Stories

**US-005: Wallet Security Management**
- **As a** security-conscious user
- **I want to** choose strong passwords with validation
- **So that** my wallet remains secure

**US-006: Wallet Backup and Export**
- **As a** user concerned about data loss
- **I want to** export or backup my wallet metadata
- **So that** I can recover access if needed

## 5. Functional Requirements

### 5.1 Wallet Saving Flow

**FR-001: Save Wallet Prompt**
- After successful wallet setup (mnemonic entry/generation), display save wallet modal
- Modal should include:
  - "Save wallet for future use?" prompt
  - Wallet name/label input field
  - Password creation field with confirmation
  - Password strength indicator
  - Save/Skip buttons

**FR-002: Encryption Implementation**
- Use AES-256-GCM for mnemonic encryption
- Use Argon2id for key derivation from password
- Generate cryptographically secure salts and nonces
- Store encrypted data in JSON format

**FR-003: File Storage**
- Save encrypted wallets in `~/.mantra_dex/wallets/` directory
- Use `.wallet` file extension
- Set restrictive file permissions (0o600 on Unix systems)
- Include wallet metadata (name, address, last accessed)

### 5.2 Startup Flow

**FR-004: Wallet Detection**
- On application startup, scan for existing wallet files
- If wallets found, display wallet selection screen
- If no wallets found, show current setup wizard

**FR-005: Wallet Selection Interface**
- Display list of saved wallets with:
  - Wallet name/label
  - Associated address (first 8 and last 8 characters)
  - Last accessed timestamp
- Sort by most recently accessed first
- Include "Create New Wallet" and "Recover Wallet" options

**FR-006: Password Authentication**
- Display password prompt for selected wallet
- Validate password against stored hash
- Load and decrypt mnemonic on successful authentication
- Show error message for incorrect password

### 5.3 Security Features

**FR-007: Password Validation**
- Minimum 12 characters
- Require mix of uppercase, lowercase, numbers, and symbols
- Display real-time password strength indicator
- Prevent common/weak passwords

**FR-008: Security Measures**
- Clear sensitive data from memory after use
- Implement password attempt limiting (3 attempts, 30-second lockout)
- Log security events (failed attempts, successful logins)
- Secure memory management for cryptographic operations

### 5.4 User Interface Requirements

**FR-009: Startup Screen**
- New screen that appears before current wizard (if wallets exist)
- Clean, intuitive interface showing wallet options
- Clear visual hierarchy and focus indicators
- Responsive design for different terminal sizes

**FR-010: Wallet Save Modal**
- Modal overlay during wallet setup completion
- Form validation with real-time feedback
- Progress indicators for encryption operations
- Clear success/error messaging

## 6. Non-Functional Requirements

### 6.1 Security
- **Encryption**: AES-256-GCM with Argon2id key derivation
- **File Permissions**: Restrictive access (0o600 on Unix)
- **Memory Security**: Clear sensitive data after use
- **Password Policy**: Strong password requirements enforced

### 6.2 Performance
- **Encryption Time**: < 500ms for wallet save operation
- **Decryption Time**: < 300ms for wallet load operation
- **Startup Time**: < 2 seconds with saved wallets
- **Memory Usage**: Minimal impact on application memory footprint

### 6.3 Usability
- **Intuitive Flow**: Clear user journey from setup to daily use
- **Error Handling**: Clear, actionable error messages
- **Recovery Options**: Users can always create new wallets
- **Accessibility**: Full keyboard navigation support

### 6.4 Reliability
- **Data Integrity**: Encrypted files must be recoverable
- **Error Recovery**: Graceful handling of corrupted wallet files
- **Backup Strategy**: Users can export/backup wallet metadata
- **Cross-Platform**: Works on Linux, macOS, and Windows

## 7. Technical Architecture

### 7.1 Current State Analysis
- Existing `WalletStorage` struct in `src/wallet/storage.rs` provides encryption foundation
- Current wizard flow in `src/tui/screens/wizard.rs` handles mnemonic input
- Application state management in `src/tui/app.rs` coordinates wallet operations
- Encryption dependencies (`aes-gcm`, `argon2`) need to be added to `Cargo.toml`

### 7.2 Required Modifications

**Code Areas Affected:**
1. **Application Startup** (`src/bin/main.rs`, `src/bin/tui.rs`)
   - Add wallet detection and selection logic
   - Modify startup flow to show wallet screen before wizard

2. **Wizard Flow** (`src/tui/screens/wizard.rs`)
   - Add wallet save modal after successful setup
   - Integrate password creation and wallet naming

3. **Application State** (`src/tui/app.rs`)
   - Add wallet selection screen state
   - Integrate wallet storage operations
   - Handle password authentication

4. **UI Components** (`src/tui/components/`)
   - Create wallet selection list component
   - Create password input with strength indicator
   - Create wallet save modal component

5. **Dependencies** (`Cargo.toml`)
   - Add `aes-gcm` and `argon2` crates
   - Update feature flags as needed

### 7.3 New Components Required
- **Wallet Selection Screen** (`src/tui/screens/wallet_selection.rs`)
- **Password Input Component** (`src/tui/components/password_input.rs`)
- **Wallet Save Modal** (`src/tui/components/wallet_save_modal.rs`)
- **Password Strength Indicator** (`src/tui/components/password_strength.rs`)

## 8. Implementation Phases

### Phase 1: Foundation (Week 1)
- Add encryption dependencies to `Cargo.toml`
- Extend `WalletStorage` with additional methods
- Create basic UI components for password input and wallet selection

### Phase 2: Core Functionality (Week 2)
- Implement wallet save modal in wizard flow
- Create wallet selection startup screen
- Integrate password authentication logic

### Phase 3: Enhanced UX (Week 3)
- Add password strength validation and indicators
- Implement security features (attempt limiting, secure memory)
- Polish UI/UX and better error handling

### Phase 4: Testing & Security (Week 4)
- Comprehensive security testing
- End-to-end user flow testing
- Performance optimization
- Documentation updates

## 9. Risk Analysis

### Technical Risks
- **Encryption Implementation**: Risk of security vulnerabilities
  - *Mitigation*: Use established crypto libraries, security audit
- **File System Permissions**: Cross-platform compatibility issues
  - *Mitigation*: Platform-specific permission handling
- **Memory Security**: Sensitive data lingering in memory
  - *Mitigation*: Explicit memory clearing, secure allocators

### User Experience Risks
- **Password Recovery**: Users forgetting passwords
  - *Mitigation*: Clear warnings, export options
- **Complex UI**: Overwhelming new users
  - *Mitigation*: Progressive disclosure, clear help text
- **Migration**: Existing users adaptation
  - *Mitigation*: Backward compatibility, migration guides

## 10. Success Criteria

### Must-Have (MVP)
- [ ] Users can save wallets after first-time setup
- [ ] Users can authenticate with saved wallets on startup
- [ ] Encryption is secure and industry-standard
- [ ] Basic wallet selection interface works

### Should-Have (V1.0)
- [ ] Password strength validation and indicators
- [ ] Multiple wallet management
- [ ] Security features (attempt limiting, logging)
- [ ] Polished UI/UX with proper error handling