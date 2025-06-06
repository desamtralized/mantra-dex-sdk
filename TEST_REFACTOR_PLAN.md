# Test Suite Refactoring Plan
## Objective: Reduce 40-50% redundancy while maintaining test coverage

### Phase 1: Enhance Test Utilities and Fixtures
**Goal**: Centralize common test setup and utilities

#### 1.1 Enhance `tests/utils.rs`
- [x] Add `should_execute_writes()` function (currently duplicated in 3+ files)
- [x] Create `TestFixtures` struct with client, pool_id, test_assets, wallet
- [x] Add `setup_test_environment()` function that returns TestFixtures
- [x] Create `create_test_assets()` helper for common asset combinations
- [x] Add `create_small_test_amounts()` for consistent test amounts
- [x] Create `get_test_denoms()` helper for token denominations
- [x] Add common error handling utilities:
  - `handle_expected_contract_error<T>(result: Result<T, Error>) -> bool`
  - `assert_transaction_success(response: &TxResponse)`
  - `expect_contract_error_containing(result: Result<T, Error>, substring: &str)`

#### 1.2 Create Test Constants Module
- [x] Add `tests/constants.rs` with:
  - Standard test amounts (SMALL_AMOUNT, MEDIUM_AMOUNT, etc.)
  - Common fee percentages
  - Test timeout durations
  - Standard slippage values

### Phase 2: Consolidate Pool Operation Tests
**Goal**: Eliminate 30% redundancy in pool operation testing

#### 2.1 Refactor `tests/swap_test.rs`
- [x] Remove `test_list_all_pools()` (redundant with client_test.rs)
- [x] Consolidate `test_provide_liquidity()` and `test_withdraw_liquidity()` into single comprehensive test
- [x] Remove `test_get_pool()` (redundant with client_test.rs)
- [x] Keep only `test_swap_operation()` and `test_simulate_swap()` as core swap tests
- [x] Add parameterized test for different swap scenarios

#### 2.2 Refactor `tests/client_test.rs`
- [x] Remove `test_client_query_pool()` (redundant with pool operation tests)
- [x] Consolidate `test_client_query_pools()` and pool creation logic
- [x] Remove `test_client_simulate_swap()` (move to swap_test.rs if needed)
- [x] Keep client-specific tests only (creation, wallet integration, balances)
- [x] Remove `test_pool_creation_if_needed()` (move to utils if needed)

#### 2.3 Refactor `tests/pool_status_test.rs`
- [x] Remove duplicate swap/liquidity tests with status validation
- [x] Keep only pool status enum tests and status validation tests
- [x] Consolidate status validation tests into fewer comprehensive tests
- [x] Remove redundant pool operation tests (swap, provide/withdraw liquidity)

### Phase 3: Consolidate Integration Tests
**Goal**: Remove unit-test-like scenarios from integration tests

#### 3.1 Refactor `tests/integration_test.rs`
- [x] Remove `test_end_to_end_provide_liquidity_with_new_parameters()` (unit test, not integration)
- [x] Remove `test_end_to_end_swap_with_new_parameters()` (unit test, not integration)  
- [x] Remove `test_backward_compatibility_optional_parameters()` (unit test)
- [x] Remove `test_backward_compatibility_claim_rewards()` (move to farm_manager_test.rs)
- [x] Remove fee validation tests (keep in fee_validation_test.rs only)
- [x] Remove error handling tests (unit tests, not integration)
- [x] Keep only true end-to-end workflow tests:
  - `test_comprehensive_pool_operations_flow()`
  - `test_epoch_validation_and_rewards()`
  - `test_parameter_migration_validation()`
- [x] Consolidate remaining tests into 2-3 comprehensive integration scenarios

### Phase 4: Optimize Fee Validation Tests
**Goal**: Eliminate 10% redundancy in fee testing

#### 4.1 Keep `tests/fee_validation_test.rs` as primary
- [x] Ensure all fee scenarios are covered in this dedicated file
- [x] Add any missing edge cases from other files
- [x] Remove fee validation from integration_test.rs
- [x] Remove fee validation from other test files

### Phase 5: Consolidate Feature Toggle Tests
**Goal**: Reduce repetitive feature toggle testing

#### 5.1 Refactor `tests/feature_toggle_test.rs`
- [x] Create parameterized test for enable/disable operations:
  - `test_pool_feature_toggle(feature_type: FeatureType, enable: bool)`
- [x] Consolidate individual enable/disable tests into 3 comprehensive tests:
  - `test_individual_feature_toggles()`
  - `test_bulk_feature_toggles()`  
  - `test_feature_toggle_error_handling()`
- [x] Remove duplicate tests and keep only unique scenarios
- [x] Remove feature toggle tests from integration_test.rs (only legitimate integration test remains)

### Phase 6: Optimize Farm Manager Tests
**Goal**: Streamline farm/epoch testing

#### 6.1 Refactor `tests/farm_manager_test.rs`
- [x] Consolidate claim reward tests into 2 tests:
  - `test_claim_rewards_all_methods()`
  - `test_query_rewards_all_methods()`
- [ ] Remove duplicate epoch validation (keep in dedicated test)
- [ ] Consolidate configuration checks
- [ ] Remove redundant method signature tests

### Phase 7: Consolidate Wallet Tests
**Goal**: Eliminate wallet testing redundancy

#### 7.1 Refactor `tests/wallet_test.rs`
- [ ] Keep this as the primary wallet testing file
- [ ] Remove wallet creation tests from other files
- [ ] Remove balance checking tests from other files
- [ ] Consolidate wallet validation logic

### Phase 8: Optimize Configuration Tests
**Goal**: Centralize configuration testing

#### 8.1 Keep `tests/config_test.rs` as primary
- [ ] Remove configuration validation from other test files
- [ ] Ensure all config scenarios are covered here
- [ ] Remove redundant network config creation tests

### Phase 9: Consolidate Migration Tests
**Goal**: Streamline migration validation

#### 9.1 Refactor `tests/migration_validation_test.rs`
- [ ] Remove duplicate parameter migration tests (covered elsewhere)
- [ ] Focus on actual migration scenarios vs unit tests
- [ ] Consolidate performance regression tests
- [ ] Remove redundant dependency compatibility tests

### Phase 10: Error Handling Optimization
**Goal**: Eliminate repetitive error handling patterns

#### 10.1 Create Common Error Handling
- [ ] Update all test files to use common error handling utilities from utils.rs
- [ ] Remove repetitive match patterns across files
- [ ] Standardize error assertion patterns
- [ ] Add error handling for specific test scenarios

### Phase 11: Final Cleanup and Validation
**Goal**: Ensure test coverage is maintained

#### 11.1 Validation Tasks
- [ ] Run full test suite to ensure no functionality is lost
- [ ] Verify test coverage hasn't decreased
- [ ] Check that all edge cases are still covered
- [ ] Ensure integration between components is still tested
- [ ] Update documentation if needed

#### 11.2 Quality Assurance
- [ ] Remove any remaining dead code
- [ ] Ensure consistent naming conventions
- [ ] Verify all tests use centralized utilities
- [ ] Check for any remaining redundant imports
- [ ] Validate that test execution time has improved

### Success Metrics
- [ ] **Target**: Reduce test files by 30-40% lines of code
- [ ] **Target**: Eliminate duplicate test setup code
- [ ] **Target**: Maintain 100% of current test coverage
- [ ] **Target**: Improve test execution time by 15-20%
- [ ] **Target**: Reduce maintenance burden for future test additions

### Files to Modify/Remove Priority List

**High Priority (Most Redundant)**:
1. `tests/integration_test.rs` - Major consolidation needed
2. `tests/swap_test.rs` - Remove duplicates
3. `tests/pool_status_test.rs` - Remove operation duplicates
4. `tests/client_test.rs` - Remove pool operation duplicates

**Medium Priority**:
5. `tests/feature_toggle_test.rs` - Consolidate similar tests
6. `tests/farm_manager_test.rs` - Streamline  
7. `tests/migration_validation_test.rs` - Focus on true migrations

**Low Priority (Less Redundant)**:
8. `tests/fee_validation_test.rs` - Keep as primary, remove from others
9. `tests/wallet_test.rs` - Keep as primary
10. `tests/config_test.rs` - Keep as primary
11. `tests/utils.rs` - Enhance, don't reduce

### Expected Outcome
- **Reduced redundancy**: 40-50% reduction in duplicate test code
- **Improved maintainability**: Centralized utilities and fixtures
- **Better organization**: Clear separation of concerns between test files
- **Faster execution**: Fewer redundant operations
- **Easier debugging**: Standardized error handling and assertions
``` 
</rewritten_file>