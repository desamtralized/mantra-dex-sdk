# AI Agent Prompt: Test Suite Refactoring

## Objective
You are tasked with refactoring the Rust test suite to eliminate 40-50% of identified redundancy while maintaining 100% test coverage. Follow the detailed plan in `TEST_REFACTOR_PLAN.md`.

## Context
The codebase is a Mantra DEX SDK with 11 test files containing significant redundancy:
- **Pool operations** tested redundantly across 4+ files  
- **Error handling patterns** duplicated 15+ times
- **Test setup code** repeated in every file
- **Fee validation** scattered across multiple files
- **Feature toggles** with repetitive enable/disable patterns

## Instructions
1. **Work systematically** through the phases in `TEST_REFACTOR_PLAN.md`
2. **Start with Phase 1** (enhance utilities) before consolidating tests
3. **Maintain test coverage** - every removed test's functionality must be preserved elsewhere
4. **Use centralized utilities** - eliminate duplicate setup code
5. **Consolidate similar tests** - merge redundant test scenarios
6. **Preserve unique test cases** - don't remove tests that cover unique scenarios
7. **Update imports** when moving/removing test functions
8. **Run tests frequently** to ensure nothing breaks

## Key Files to Focus On (in order):
1. `tests/utils.rs` - Enhance with common utilities
2. `tests/integration_test.rs` - Major consolidation needed (remove unit-test-like scenarios)  
3. `tests/swap_test.rs` - Remove pool query duplicates
4. `tests/pool_status_test.rs` - Remove operation duplicates, keep status tests
5. `tests/client_test.rs` - Keep only client-specific tests
6. `tests/feature_toggle_test.rs` - Consolidate repetitive enable/disable tests

## Success Criteria
- [ ] Reduce test codebase by 30-40% lines
- [ ] All tests still pass
- [ ] No loss of test coverage for unique scenarios
- [ ] Centralized test utilities used throughout
- [ ] Faster test execution time

## What NOT to do
- Don't remove tests covering unique edge cases
- Don't break existing test functionality
- Don't remove the dedicated test files (fee_validation_test.rs, wallet_test.rs, config_test.rs)
- Don't change test logic, only consolidate duplicates

Execute the tasks systematically and check off completed items in the plan document. 