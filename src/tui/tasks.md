# TUI Navigation and Functionality Tasks

## AI Agent Prompt for Task Execution

**ROLE**: You are a Rust TUI development specialist working on the Mantra DEX SDK. You have deep expertise in ratatui, terminal UI patterns, focus management, and blockchain integration.

**INSTRUCTIONS**: 
1. Work on ONE task at a time from the checklist below
2. Read the task description, implementation guidance, and file references carefully
3. Examine the current code state using file reading and code search tools
4. Implement the required changes using proper edit tools
5. Update the task checkbox from [ ] to [x] when completed
6. Provide a brief summary of changes made
7. NEVER run `cargo run` - always ask the user to test the TUI manually

**RULES**:
- Follow the two-level navigation model (Tab Mode vs Content Mode) strictly
- Maintain the `focus_handled` guard pattern in event handling
- Register all interactive elements with the FocusManager when entering content mode
- Use yellow borders for focus indicators, green for actions, red for destructive actions
- Cache blockchain data in AppState to avoid repeated queries
- Follow existing code patterns and architectural conventions
- Test manually only - do not create automated TUI tests
- Reference workspace rules and existing documentation before making changes
- Ask user to test TUI functionality after each major change

---

This document outlines critical navigation and functionality issues in the Mantra DEX TUI that need to be addressed. Each task includes implementation guidance and relevant file references.

## Navigation Issues

### [x] Fix Tab Navigation Cycle on Main Screen
**Issue**: Tab navigation doesn't complete the full screen cycle properly. When on Dashboard (Tab 1) and pressing Tab repeatedly:
- Tab 1 → Tab 2 (Pools) ✓
- Tab 2 → Tab 3 (Swap) ✓  
- Tab 3 → Should go to Tab 4 (Liquidity) but instead enters Swap screen content mode ❌

**Root Cause**: The focus manager is likely transitioning to content mode instead of continuing tab-level navigation.

**Implementation Guidance**:
- Check `NavigationLevel::Screen` handling in `src/tui/app.rs` around line 2680
- Verify the `focus_handled` guard in `App::handle_event` isn't interfering with tab cycling
- Ensure the swap screen doesn't automatically enter content mode during tab navigation
- Reference the two-level focus model: Tab Mode should stay in Tab Mode until Enter is pressed

**COMPLETED**: Fixed by modifying swap screen event handler in `src/tui/app.rs` to respect navigation mode before intercepting Tab events. The swap screen now only handles Tab/BackTab events when in `WithinScreen` mode, allowing proper screen-level tab cycling when in `ScreenLevel` mode.

**Files Modified**:
- `src/tui/app.rs` - Added navigation mode checks in `handle_swap_screen_event()` for Tab/BackTab events

### [x] Fix Settings Screen Navigation
**Issue**: Content within the Settings screen (Tab 8) is not navigatable via keyboard.

**Implementation Guidance**:
- Review settings screen focus registration in `src/tui/screens/settings.rs`
- Ensure all interactive elements are registered with the FocusManager
- Follow the pattern used in other screens for content-level navigation
- Add proper focus indicators for settings options

**COMPLETED**: Enhanced settings screen navigation by implementing comprehensive focus management. Key changes:

1. **Enhanced Focus Registration**: Updated `initialize_focus_for_screen` in `src/tui/app.rs` to register all focusable components in the settings screen including:
   - Section navigation list
   - Network section components (environment dropdown, text inputs for name, RPC, gas settings)  
   - Wallet section components (import mode toggle, mnemonic input, show/hide toggle)
   - Display section components (theme dropdown, refresh interval inputs, auto-refresh toggle)
   - Action buttons (save, reset)

2. **Improved Event Handling**: Enhanced `handle_settings_screen_event` in `src/tui/app.rs` to:
   - Respect navigation mode (WithinScreen vs ScreenLevel)
   - Handle section navigation when focused on section list
   - Support text input editing mode activation
   - Handle all button types (toggles, dropdowns, actions)
   - Properly handle Escape key to exit content mode
   - Support backspace for all text input fields

3. **Added Helper Functions**: Created helper functions in `src/tui/screens/settings.rs`:
   - `initialize_settings_screen_focus()` - for focus initialization
   - `get_focusable_components_for_section()` - returns focusable components per section
   - `next_field()` / `previous_field()` - navigate within form fields
   - `is_current_field_editable()` - check if field can be edited
   - `get_current_field_id()` - get current field identifier

**Files Modified**:
- `src/tui/app.rs` - Enhanced focus registration and event handling
- `src/tui/screens/settings.rs` - Added focus management helper functions

**Files to Review**:
- `src/tui/screens/settings.rs` - Settings screen implementation
- Reference other screens like `src/tui/screens/dashboard.rs` for proper focus patterns

## Screen-Specific Issues

### [ ] Remove Multi-hop Screen (Tab 4) Entirely
**Issue**: Multi-hop functionality should be removed from the navigation.

**Implementation Steps**:
1. Remove multi-hop screen from the navigation enum in `src/tui/app.rs`
2. Remove the screen implementation file `src/tui/screens/multihop.rs`
3. Update tab numbering so Liquidity becomes Tab 4, Rewards becomes Tab 5, etc.
4. Update help text and navigation references in `src/tui/ui.rs`
5. Remove multi-hop related imports and references

**Files to Modify**:
- `src/tui/app.rs` - Remove screen enum variant and navigation
- `src/tui/screens/mod.rs` - Remove module reference
- `src/tui/ui.rs` - Update navigation bar and help text
- `src/tui/events.rs` - Remove multi-hop event handling
- Delete: `src/tui/screens/multihop.rs`

### [ ] Fix Pools Screen Navigation and Display
**Issue**: Multiple problems with the Pools screen (Tab 2):
1. Content is not accessible through tab navigation
2. Pool Type is not displayed in the list
3. Status shows generic "available" instead of granular feature status

**Implementation Guidance**:

#### Part A: Enable Tab Navigation
- Register pool list and other interactive elements with FocusManager
- Implement proper focus handling for pool selection
- Add visual focus indicators (yellow borders)

#### Part B: Add Pool Type Display
- Query pool type information from the pool manager contract
- Display pool type in the pools table (e.g., "XYK", "Stable", "Weighted")
- Update table columns in the pools screen rendering

#### Part C: Implement Granular Status Display
- Query individual feature flags from pool manager contract:
  - `withdraws_enabled`
  - `deposits_enabled` 
  - `swaps_enabled`
- Display status as icons or abbreviated text (e.g., "W+D+S" for all enabled)
- Color-code status indicators (green=enabled, red=disabled)

**Smart Contract Integration**:
- Use pool manager contract address from `config/contracts.toml`: `mantra1vwj600jud78djej7ttq44dktu4wr3t2yrrsjgmld8v3jq8mud68q5w7455`
- Query pool features via CosmWasm queries in `src/client.rs`
- Cache pool feature status in `AppState` to avoid repeated queries

**Files to Review**:
- `src/tui/screens/pools.rs` - Main pools screen implementation
- `src/client.rs` - Add pool feature query methods
- `config/contracts.toml` - Pool manager contract address
- `src/tui/app.rs` - Cache pool feature data in AppState

### [ ] Fix Liquidity Screen Initial Pool Logic
**Issue**: When adding initial liquidity to a pool (when asset balances are 0), the screen automatically calculates the second field, but there's no established proportion yet.

**Expected Behavior**: For initial liquidity provision, both fields should be independently editable since there's no pool ratio to maintain.

**Implementation Guidance**:
- Check if pool has zero total liquidity before applying ratio calculations
- Query pool asset balances from the blockchain
- Disable automatic field calculation when `total_liquidity == 0`
- Allow independent input for both asset amounts during initial provision
- Show clear UI indication when providing initial liquidity vs. proportional liquidity

**Files to Review**:
- `src/tui/screens/liquidity.rs` - Liquidity provision logic
- `src/client.rs` - Pool balance query methods

### [ ] Fix Admin Feature Controls Navigation and Functionality
**Issue**: On Admin screen (Tab 7) → Feature Controls:
1. Cannot control individual features for pools
2. Pool list is not navigatable
3. Cannot select individual pools

**Implementation Guidance**:
- Implement navigatable pool list with focus indicators
- Add individual toggle controls for each feature:
  - Withdrawals enabled/disabled
  - Deposits enabled/disabled  
  - Swaps enabled/disabled
- Implement feature toggle functionality via admin contract calls
- Add confirmation dialogs for feature changes
- Show current feature status clearly

**Smart Contract Integration**:
- Use appropriate admin contract methods for feature management
- Implement proper permission checks (admin-only operations)
- Handle transaction confirmation and error states

**Files to Review**:
- `src/tui/screens/admin.rs` - Admin screen implementation
- `src/client.rs` - Add admin contract interaction methods

## Implementation Notes

### Focus Management Best Practices
- All screens must register focusable elements when entering content mode
- Use the unified focus system from `src/tui/utils/focus_manager.rs`
- Maintain the `focus_handled` guard pattern to prevent focus leaks
- Follow the two-level navigation model (Tab Mode vs Content Mode)

### Event Handling Guidelines
- Respect current `NavigationLevel` when processing key events
- Prevent global focus moves while dropdowns or tables are active
- Implement proper async handling for blockchain operations

### Testing Approach
- **Do not implement automated TUI tests** - test manually through the interface
- Focus testing efforts on SDK functionality only
- Test navigation flows manually after each fix
- Never run 'cargo run' to test the TUI, always ask the user to run it and then test the TUI manually

### Code References
- See `FOCUS_FIXES.md` for recent navigation improvements
- Review `RATATUI_TASKS.md` for overall TUI development roadmap
- Follow patterns established in `src/tui/screens/settings.rs` for proper focus handling

## Priority Order
1. Fix tab navigation cycle (critical UX issue)
2. Remove multi-hop screen (cleanup task)
3. Fix pools screen navigation and display (core functionality)
4. Fix settings navigation (user experience)
5. Fix admin feature controls (admin functionality)
6. Fix liquidity initial pool logic (edge case handling) 