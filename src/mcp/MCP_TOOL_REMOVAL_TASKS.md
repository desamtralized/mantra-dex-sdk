# MCP Tool Removal Task List

## Overview
Remove 17 advanced analytics and monitoring tools from the Mantra DEX MCP server to simplify the codebase and focus on core functionality.

## Tools to Remove
```
validate_pool_status
provide_liquidity_unchecked
validate_swap_parameters
validate_swap_result
get_swap_execution_summary
monitor_swap_transaction
get_transaction_monitor_status
cancel_transaction_monitor
list_transaction_monitors
cleanup_transaction_monitors
get_swap_history
get_swap_statistics
export_swap_history
track_swap_execution
analyze_swap_performance
generate_trading_report
calculate_impermanent_loss
```

## Task Checklist

### 1. Remove Tool Definitions from `get_available_tools()` method
- [x] Remove `validate_pool_status` tool definition
- [x] Remove `provide_liquidity_unchecked` tool definition
- [x] Remove `validate_swap_parameters` tool definition
- [x] Remove `validate_swap_result` tool definition
- [x] Remove `get_swap_execution_summary` tool definition
- [x] Remove `monitor_swap_transaction` tool definition
- [x] Remove `get_transaction_monitor_status` tool definition
- [x] Remove `cancel_transaction_monitor` tool definition
- [x] Remove `list_transaction_monitors` tool definition
- [x] Remove `cleanup_transaction_monitors` tool definition
- [x] Remove `get_swap_history` tool definition
- [x] Remove `get_swap_statistics` tool definition
- [x] Remove `export_swap_history` tool definition
- [x] Remove `track_swap_execution` tool definition
- [x] Remove `analyze_swap_performance` tool definition
- [x] Remove `generate_trading_report` tool definition
- [x] Remove `calculate_impermanent_loss` tool definition

### 2. Remove Tool Handlers from `handle_tool_call()` method
- [x] Remove `"validate_pool_status"` case and handler call
- [x] Remove `"provide_liquidity_unchecked"` case and handler call
- [x] Remove `"validate_swap_parameters"` case and handler call
- [x] Remove `"validate_swap_result"` case and handler call
- [x] Remove `"get_swap_execution_summary"` case and handler call
- [x] Remove `"monitor_swap_transaction"` case and handler call
- [x] Remove `"get_transaction_monitor_status"` case and handler call
- [x] Remove `"cancel_transaction_monitor"` case and handler call
- [x] Remove `"list_transaction_monitors"` case and handler call
- [x] Remove `"cleanup_transaction_monitors"` case and handler call
- [x] Remove `"get_swap_history"` case and handler call
- [x] Remove `"get_swap_statistics"` case and handler call
- [x] Remove `"export_swap_history"` case and handler call
- [x] Remove `"track_swap_execution"` case and handler call
- [x] Remove `"analyze_swap_performance"` case and handler call
- [x] Remove `"generate_trading_report"` case and handler call
- [x] Remove `"calculate_impermanent_loss"` case and handler call

### 3. Remove Handler Method Implementations
- [x] Remove `handle_validate_pool_status()` method
- [x] Remove `handle_provide_liquidity_unchecked()` method
- [x] Remove `handle_validate_swap_parameters()` method
- [x] Remove `handle_validate_swap_result()` method
- [x] Remove `handle_get_swap_execution_summary()` method
- [x] Remove `handle_monitor_swap_transaction()` method
- [x] Remove `handle_get_transaction_monitor_status()` method
- [x] Remove `handle_cancel_transaction_monitor()` method
- [x] Remove `handle_list_transaction_monitors()` method
- [x] Remove `handle_cleanup_transaction_monitors()` method
- [x] Remove `handle_get_swap_history()` method
- [x] Remove `handle_get_swap_statistics()` method
- [x] Remove `handle_export_swap_history()` method
- [x] Remove `handle_track_swap_execution()` method
- [x] Remove `handle_analyze_swap_performance()` method
- [x] Remove `handle_generate_trading_report()` method
- [x] Remove `handle_calculate_impermanent_loss()` method

### 4. Remove Supporting Methods and Infrastructure
- [x] Remove `perform_swap_result_validation()` method
- [x] Remove `generate_swap_execution_summary()` method
- [x] Remove `validate_swap_parameters_internal()` method
- [x] Remove `get_swap_history_from_blockchain()` method
- [x] Remove `filter_and_sort_swap_history()` method
- [x] Remove `format_swap_record()` method
- [x] Remove `calculate_time_period_boundaries()` method
- [x] Remove `get_swap_data_for_period()` method
- [x] Remove `calculate_pool_breakdown()` method
- [x] Remove `calculate_asset_breakdown()` method
- [x] Remove `calculate_performance_metrics()` method
- [x] Remove `calculate_trend_analysis()` method
- [x] Remove `store_swap_tracking_record()` method
- [x] Remove `get_swap_history_for_export()` method
- [x] Remove `format_swap_history_as_json()` method
- [x] Remove `format_swap_history_as_csv()` method
- [x] Remove `format_swap_history_as_tsv()` method
- [x] Remove `compress_data()` method
- [x] Remove `get_swap_data_for_analysis()` method
- [x] Remove `analyze_gas_performance()` method
- [x] Remove `analyze_slippage_performance()` method
- [x] Remove `analyze_timing_performance()` method
- [x] Remove `generate_performance_recommendations()` method
- [x] Remove `handle_trades_history_resource()` method
- [x] Remove `handle_trades_pending_resource()` method
- [x] Remove `handle_liquidity_positions_resource()` method
- [x] Remove `handle_estimate_lp_withdrawal_amounts()` method
- [x] Remove `format_trading_report_summary()` method
- [x] Remove `format_trading_report_detailed()` method

### 5. Remove Data Structures and Types
- [x] Remove `SwapTrackingRecord` struct
- [x] Remove `SwapAnalyticsData` struct
- [x] Review and remove unused `TransactionMonitor` related types if no longer needed
- [x] Review and remove unused `TransactionMonitorConfig` if no longer needed
- [x] Review and remove unused `TransactionStatus` enum if no longer needed
- [x] Review and remove unused `TransactionMonitorManager` if no longer needed

### 6. Clean Up State Management
- [x] Remove `transaction_monitor_manager` field from `McpServerStateData` if no longer needed (SKIPPED - still used by core functionality)
- [x] Update `McpServerStateData::new()` constructor to remove transaction monitor manager initialization (SKIPPED - still needed)
- [x] Review and clean up any transaction monitoring related initialization code (SKIPPED - still needed)

### 7. Update Dependencies
- [x] Review `Cargo.toml` for any dependencies that were only used by removed tools
- [ ] Remove unused dependencies if any (like regex for pool ID extraction)
- [ ] Update import statements to remove unused imports

### 8. Documentation Updates
- [ ] Update any documentation that references the removed tools
- [ ] Update README files if they mention the removed functionality
- [ ] Update any example usage files

### 9. Testing and Validation
- [ ] Ensure the server compiles successfully after removals
- [ ] Test that remaining tools still function correctly
- [ ] Verify that the MCP protocol still works properly
- [ ] Test with Cursor integration to ensure compatibility

### 10. Final Cleanup
- [ ] Run `cargo clippy` to check for any warnings
- [ ] Run `cargo fmt` to format the code
- [ ] Review the diff to ensure nothing unintended was removed
- [ ] Test build with `cargo build --release --bin mcp-server --features mcp`

## Notes
- Focus on removing complete functionality rather than leaving partial implementations
- Ensure that removing these tools doesn't break any core functionality
- The remaining tools should provide all essential DEX operations
- Transaction monitoring infrastructure can be removed entirely if not needed by other tools

## Estimated Impact
- **Code Reduction**: ~3000+ lines of code removed
- **Complexity Reduction**: Significant reduction in server complexity
- **Maintenance**: Easier to maintain and understand
- **Performance**: Potentially better performance due to reduced overhead 