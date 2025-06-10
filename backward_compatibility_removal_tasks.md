# Backward Compatibility Removal Task List

This document outlines the tasks required to remove all backward compatibility features and tests from the MANTRA DEX SDK v3.0.0. These features were added to ease migration from v2.x but should be removed in the next major version.

## 🎯 Overview
The v3.0.0 SDK introduced several backward compatibility features to help users migrate from v2.x. These include deprecated methods, parameter names, and legacy API patterns that should be removed for cleaner code and better maintainability.

## ✅ Current Status
**NEARLY COMPLETE** - All backward compatibility code has been successfully removed from the codebase!

- ✅ All deprecated methods removed
- ✅ All backward compatibility tests removed  
- ✅ All documentation cleaned up
- ✅ Version bumped to v4.0.0
- ✅ Changelog created with migration guide
- ✅ Code compiles successfully with no errors
- ⏳ Ready for release tagging (final step)

---

## 📋 Core Client Methods - Deprecated Features

### Deprecated Methods in `src/client.rs`

- [x] **Remove `update_global_features()` method** (Lines ~1035-1055)
  - Method is marked as `#[deprecated]` since v3.0.0
  - Replace with per-pool `update_pool_features()` calls
  - Remove deprecation attribute and entire method

- [x] **Remove backward compatibility convenience methods for farm rewards**
  - [x] Remove `claim_rewards_all()` method (~Line 793-798)
  - [x] Remove `claim_rewards_until_epoch()` method (~Line 809-814)
  - [x] Remove `query_all_rewards()` method (~Line 869-871)
  - [x] Remove `query_rewards_until_epoch()` method (~Line 876-882)

- [x] **Clean up `claim_rewards()` method**
  - Remove backward compatibility comments mentioning v2.x parameterless claim
  - Simplify logic by removing the None case handling for epoch parameter
  - Make `until_epoch` parameter required instead of optional

- [x] **Clean up `query_rewards()` method**
  - Remove backward compatibility comments
  - Simplify logic by removing the None case handling for epoch parameter
  - Make `until_epoch` parameter required instead of optional

---

## 📋 Documentation and Comments Cleanup

### Parameter Renaming Documentation in `src/client.rs`

- [x] **Remove v3.0.0 breaking change documentation**
  - [x] Remove "v3.0.0 Breaking Changes" comments in `swap()` method (~Line 455)
  - [x] Remove "v3.0.0 Breaking Changes" comments in `provide_liquidity()` method (~Line 505-506)
  - [x] Remove references to old parameter names (`max_spread`, `slippage_tolerance`)

- [x] **Clean up method documentation**
  - [x] Remove "Backward Compatibility" sections from method docs
  - [x] Remove references to v2.x behavior in comments
  - [x] Simplify parameter descriptions without mentioning old names
  - [x] Remove version references (v3.0.0) from client documentation

### Update README.md

- [x] **Remove outdated API examples** (`README.md` Lines ~178-179)
  - Remove examples showing old parameter names (`max_spread`, `slippage_tolerance`)
  - Update with current v3.0.0+ API signatures only

---

## 📋 Test Files - Backward Compatibility Tests

### `tests/feature_toggle_test.rs`

- [x] **Remove `test_backward_compatibility_global_features()` test** (~Line 269-297)
  - Entire test function testing deprecated `update_global_features()` method
  - Remove `#[allow(deprecated)]` attributes

- [x] **Clean up `test_feature_toggle_method_signatures()` test** (~Line 347-350)
  - Remove testing of `update_global_features()` method
  - Remove `#[allow(deprecated)]` attributes

### `tests/integration_test.rs`

- [x] **Remove `test_backward_compatibility_optional_parameters()` test** (~Line 82-106)
  - Test for v2.x style None parameter handling
  - Remove entire test function

- [x] **Remove `test_backward_compatibility_claim_rewards()` test** (~Line 109-148)
  - Test for parameterless claim rewards (v2.x style)
  - Remove entire test function

- [x] **Clean up `test_parameter_migration_validation()` test** (~Line 362-400)
  - Remove comments about old parameter names
  - Focus only on current API validation
  - Renamed to `test_parameter_validation()` and cleaned up migration references

- [x] **Update `test_end_to_end_swap_with_new_parameters()` test** (~Line 51-80)
  - Remove comments referencing "new" parameters (they're standard now)
  - Remove mention of `max_spread` in comments
  - Renamed to `test_end_to_end_swap()` and cleaned up "new parameter" references

### `tests/migration_validation_test.rs`

- [x] **Remove migration-specific tests**
  - [x] Remove `test_api_parameter_migration()` test (~Line 13-89)
  - [x] Remove `test_response_structure_migration()` test (~Line 93-136)
  - [x] Remove `test_epoch_functionality_migration()` test (~Line 434-483)
  - [x] Remove `test_per_pool_feature_toggle_migration()` test (~Line 486-526)

- [x] **Remove dependency compatibility tests**
  - [x] Remove `test_dependency_compatibility()` test (~Line 138-206)
  - [x] Remove `test_performance_regression()` test (~Line 208-286)
  - Note: These tests were found in integration_test.rs and have been removed

- [x] **Consider removing entire file**
  - The entire file seems focused on migration validation
  - Evaluate if any tests should be moved to other test files or removed entirely
  - Note: File was already removed from the codebase

### `tests/farm_manager_test.rs`

- [x] **Remove backward compatibility tests**
  - [x] Remove `test_claim_rewards_backward_compatibility()` test (~Line 31-61)
  - [x] Update `test_query_rewards()` test (~Line 118-148) to remove backward compatibility testing
  - [x] Clean up `test_claim_method_signatures()` test (~Line 275-299) to remove backward compatibility methods

### Other Test Files

- [x] **Additional test cleanup performed**
  - [x] Cleaned up `test_end_to_end_provide_liquidity_with_new_parameters()` test
  - [x] Renamed to `test_end_to_end_provide_liquidity()` and removed "new parameter" references
  - [x] Updated test comments to remove migration-related language

- [x] **Update `example_usage.rs`**
  - [x] Remove backward compatibility examples (~Line 164-165, 172-173)
  - [x] Remove comments mentioning v2.x parameter names (~Line 47)
  - [x] Update examples to use current API only
  - [x] Remove version references (v3.0.0) from comments and documentation

---

## 📋 Error Handling and Types

### `src/error.rs`

- [x] **Clean up error documentation**
  - Remove "v3.0.0 New" annotations from error types (~Line 47-52)
  - Update documentation to reflect current state without version references

---

## 📋 Configuration and Dependencies

### Review Cargo.toml

- [x] **Verify dependency versions**
  - Ensure no backward compatibility dependencies are included
  - Check if any dependencies can be updated/simplified

### Update Version Numbers

- [x] **Bump version to v4.0.0** (or appropriate next major version)
  - Update `Cargo.toml` version
  - Update any version references in documentation
  - Consider this a breaking change release

---

## 📋 Validation and Testing

### Post-Removal Testing

- [x] **Run full test suite after removals**
  - Ensure all remaining tests pass
  - Verify no compilation errors after removing deprecated code

- [x] **Update integration tests**
  - Ensure integration tests only use current API
  - Remove any test utilities that supported backward compatibility

- [x] **Performance testing**
  - Verify performance hasn't regressed after cleanup
  - Ensure simplified code paths perform as expected

### Documentation Updates

- [x] **Update internal documentation**
  - Remove migration guides
  - Update API documentation
  - Clean up inline code comments

- [x] **Update external documentation**
  - Update README with clean API examples
  - Remove migration-related documentation
  - Ensure examples use current best practices

---

## 📋 Final Cleanup

### Code Quality

- [x] **Run linting and formatting**
  - `cargo fmt` to format code
  - `cargo clippy` to check for issues
  - Remove any unused imports after cleanup

- [x] **Dead code analysis**
  - Use `cargo +nightly udeps` to find unused dependencies
  - Remove any helper functions that are no longer needed
  - Clean up any constants or types only used by removed code

### Release Preparation

- [x] **Create changelog entry**
  - Document all breaking changes
  - List removed backward compatibility features
  - Provide migration guide for users still on older versions

- [ ] **Tag release** (Ready for release)
  - Create git tag for new major version
  - Update release notes
  - Consider deprecation timeline for users
  - Note: All other tasks completed, ready for release tagging

---

## ⚠️ Important Notes

- **This is a breaking change**: Removing backward compatibility will break existing code that relies on deprecated methods
- **Version bump required**: This should be released as a new major version (e.g., v4.0.0)
- **Migration guide**: Consider providing a migration guide for users upgrading from v3.0.0 to the new version
- **Testing**: Thoroughly test after each removal to ensure no unintended side effects

## 🎯 Success Criteria

- [x] All deprecated methods removed
- [x] All backward compatibility tests removed
- [x] All version-specific documentation cleaned up
- [x] Full test suite passes
- [x] Code compiles without warnings
- [x] Performance maintained or improved
- [x] Clean, maintainable codebase ready for future development 