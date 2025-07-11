use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use super::script_parser::{ScriptParseError, StepAction, TestScript, TestStep};
use super::sdk_adapter::McpSdkAdapter;

/// Script execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecutionResult {
    /// Script name
    pub script_name: String,
    /// Overall execution status
    pub status: ExecutionStatus,
    /// Execution start time
    pub start_time: String,
    /// Execution end time
    pub end_time: Option<String>,
    /// Total execution duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Individual step results
    pub step_results: Vec<StepExecutionResult>,
    /// Summary of execution
    pub summary: ExecutionSummary,
    /// Error details if failed
    pub error: Option<String>,
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    /// Script is running
    Running,
    /// Script completed successfully
    Success,
    /// Script failed
    Failed,
    /// Script was cancelled
    Cancelled,
    /// Script timed out
    TimedOut,
}

/// Individual step execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionResult {
    /// Step number
    pub step_number: usize,
    /// Step description
    pub description: String,
    /// Step execution status
    pub status: ExecutionStatus,
    /// Step start time
    pub start_time: String,
    /// Step end time
    pub end_time: Option<String>,
    /// Step duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Step result data
    pub result_data: Option<Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Expected vs actual outcome
    pub outcome_validation: Option<OutcomeValidation>,
}

/// Outcome validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeValidation {
    /// Expected outcome
    pub expected: String,
    /// Actual outcome
    pub actual: String,
    /// Whether the outcome matched expectations
    pub matches: bool,
    /// Additional validation notes
    pub notes: Option<String>,
}

/// Execution summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Total steps executed
    pub total_steps: usize,
    /// Successful steps
    pub successful_steps: usize,
    /// Failed steps
    pub failed_steps: usize,
    /// Skipped steps
    pub skipped_steps: usize,
    /// Pass rate percentage
    pub pass_rate: f64,
    /// Key metrics extracted from execution
    pub metrics: HashMap<String, Value>,
}

/// Script execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptExecutionConfig {
    /// Maximum execution time for entire script (seconds)
    pub max_script_timeout: u64,
    /// Default timeout for individual steps (seconds)
    pub default_step_timeout: u64,
    /// Whether to continue execution after a step fails
    pub continue_on_failure: bool,
    /// Whether to validate expected outcomes
    pub validate_outcomes: bool,
    /// Whether to collect detailed metrics
    pub collect_metrics: bool,
}

impl Default for ScriptExecutionConfig {
    fn default() -> Self {
        Self {
            max_script_timeout: 300,  // 5 minutes
            default_step_timeout: 30, // 30 seconds
            continue_on_failure: false,
            validate_outcomes: true,
            collect_metrics: true,
        }
    }
}

/// Script execution errors
#[derive(Debug, thiserror::Error)]
pub enum ScriptExecutionError {
    #[error("Script execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Step execution failed: {0}")]
    StepFailed(String),
    #[error("Script timed out")]
    ScriptTimeout,
    #[error("Step timed out: {0}")]
    StepTimeout(String),
    #[error("SDK adapter error: {0}")]
    SdkError(String),
    #[error("Invalid action parameters: {0}")]
    InvalidParameters(String),
    #[error("Script validation failed: {0}")]
    ValidationFailed(String),
}

/// Script runner implementation
pub struct ScriptRunner {
    /// SDK adapter for executing MCP tools
    sdk_adapter: Arc<McpSdkAdapter>,
    /// Execution configuration
    config: ScriptExecutionConfig,
    /// Current execution state
    execution_state: Option<ScriptExecutionState>,
}

/// Internal execution state
#[derive(Debug)]
struct ScriptExecutionState {
    /// Current script being executed
    script: TestScript,
    /// Current step index
    current_step: usize,
    /// Execution start time
    start_time: Instant,
    /// Step results so far
    step_results: Vec<StepExecutionResult>,
    /// Collected metrics
    metrics: HashMap<String, Value>,
}

impl ScriptRunner {
    /// Create a new script runner
    pub fn new(sdk_adapter: Arc<McpSdkAdapter>) -> Self {
        Self {
            sdk_adapter,
            config: ScriptExecutionConfig::default(),
            execution_state: None,
        }
    }

    /// Create a new script runner with custom configuration
    pub fn with_config(sdk_adapter: Arc<McpSdkAdapter>, config: ScriptExecutionConfig) -> Self {
        Self {
            sdk_adapter,
            config,
            execution_state: None,
        }
    }

    /// Execute a test script
    pub async fn execute_script(
        &mut self,
        script: TestScript,
    ) -> Result<ScriptExecutionResult, ScriptExecutionError> {
        let start_time = Instant::now();
        let start_time_str = chrono::Utc::now().to_rfc3339();

        info!("Starting script execution: {}", script.name);

        // Initialize execution state
        self.execution_state = Some(ScriptExecutionState {
            script: script.clone(),
            current_step: 0,
            start_time,
            step_results: Vec::new(),
            metrics: HashMap::new(),
        });

        // Execute with timeout
        let timeout_duration = Duration::from_secs(self.config.max_script_timeout);
        let execution_future = self.execute_script_internal();

        match timeout(timeout_duration, execution_future).await {
            Ok(result) => {
                let end_time = chrono::Utc::now().to_rfc3339();
                let duration_ms = start_time.elapsed().as_millis() as u64;

                match result {
                    Ok(step_results) => {
                        let summary = self.create_execution_summary(&step_results);

                        Ok(ScriptExecutionResult {
                            script_name: script.name,
                            status: ExecutionStatus::Success,
                            start_time: start_time_str,
                            end_time: Some(end_time),
                            duration_ms: Some(duration_ms),
                            step_results,
                            summary,
                            error: None,
                        })
                    }
                    Err(e) => {
                        let step_results = self
                            .execution_state
                            .as_ref()
                            .map(|s| s.step_results.clone())
                            .unwrap_or_default();
                        let summary = self.create_execution_summary(&step_results);

                        Ok(ScriptExecutionResult {
                            script_name: script.name,
                            status: ExecutionStatus::Failed,
                            start_time: start_time_str,
                            end_time: Some(end_time),
                            duration_ms: Some(duration_ms),
                            step_results,
                            summary,
                            error: Some(e.to_string()),
                        })
                    }
                }
            }
            Err(_) => {
                let end_time = chrono::Utc::now().to_rfc3339();
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let step_results = self
                    .execution_state
                    .as_ref()
                    .map(|s| s.step_results.clone())
                    .unwrap_or_default();
                let summary = self.create_execution_summary(&step_results);

                Ok(ScriptExecutionResult {
                    script_name: script.name,
                    status: ExecutionStatus::TimedOut,
                    start_time: start_time_str,
                    end_time: Some(end_time),
                    duration_ms: Some(duration_ms),
                    step_results,
                    summary,
                    error: Some("Script execution timed out".to_string()),
                })
            }
        }
    }

    /// Internal script execution logic
    async fn execute_script_internal(
        &mut self,
    ) -> Result<Vec<StepExecutionResult>, ScriptExecutionError> {
        let mut step_results = Vec::new();

        if let Some(ref state) = self.execution_state {
            let script = state.script.clone();

            // Execute each step
            for (index, step) in script.steps.iter().enumerate() {
                info!("Executing step {}: {}", step.step_number, step.description);

                let step_result = self.execute_step(step).await;

                // Update execution state
                if let Some(ref mut state) = self.execution_state {
                    state.current_step = index + 1;
                    state.step_results.push(step_result.clone());
                }

                step_results.push(step_result.clone());

                // Check if step failed and we shouldn't continue
                if step_result.status == ExecutionStatus::Failed && !self.config.continue_on_failure
                {
                    error!(
                        "Step {} failed and continue_on_failure is false, stopping execution",
                        step.step_number
                    );
                    break;
                }
            }
        }

        Ok(step_results)
    }

    /// Execute a single step
    async fn execute_step(&mut self, step: &TestStep) -> StepExecutionResult {
        let start_time = Instant::now();
        let start_time_str = chrono::Utc::now().to_rfc3339();

        debug!(
            "Executing step: {} - {}",
            step.step_number, step.description
        );

        // Determine timeout
        let step_timeout = step.timeout.unwrap_or(self.config.default_step_timeout);
        let timeout_duration = Duration::from_secs(step_timeout);

        // Execute step with timeout
        let execution_future = self.execute_step_action(&step.action, &step.parameters);

        match timeout(timeout_duration, execution_future).await {
            Ok(result) => {
                let end_time = chrono::Utc::now().to_rfc3339();
                let duration_ms = start_time.elapsed().as_millis() as u64;

                match result {
                    Ok(result_data) => {
                        let outcome_validation = if self.config.validate_outcomes {
                            step.expected_outcome
                                .as_ref()
                                .map(|expected| self.validate_outcome(expected, &result_data))
                        } else {
                            None
                        };

                        let status = if let Some(ref validation) = outcome_validation {
                            if validation.matches {
                                ExecutionStatus::Success
                            } else {
                                ExecutionStatus::Failed
                            }
                        } else {
                            ExecutionStatus::Success
                        };

                        StepExecutionResult {
                            step_number: step.step_number,
                            description: step.description.clone(),
                            status,
                            start_time: start_time_str,
                            end_time: Some(end_time),
                            duration_ms: Some(duration_ms),
                            result_data: Some(result_data),
                            error: None,
                            outcome_validation,
                        }
                    }
                    Err(e) => {
                        let end_time = chrono::Utc::now().to_rfc3339();
                        let duration_ms = start_time.elapsed().as_millis() as u64;

                        StepExecutionResult {
                            step_number: step.step_number,
                            description: step.description.clone(),
                            status: ExecutionStatus::Failed,
                            start_time: start_time_str,
                            end_time: Some(end_time),
                            duration_ms: Some(duration_ms),
                            result_data: None,
                            error: Some(e.to_string()),
                            outcome_validation: None,
                        }
                    }
                }
            }
            Err(_) => {
                let end_time = chrono::Utc::now().to_rfc3339();
                let duration_ms = start_time.elapsed().as_millis() as u64;

                StepExecutionResult {
                    step_number: step.step_number,
                    description: step.description.clone(),
                    status: ExecutionStatus::TimedOut,
                    start_time: start_time_str,
                    end_time: Some(end_time),
                    duration_ms: Some(duration_ms),
                    result_data: None,
                    error: Some(format!("Step timed out after {} seconds", step_timeout)),
                    outcome_validation: None,
                }
            }
        }
    }

    /// Execute a step action by calling the appropriate MCP tool
    async fn execute_step_action(
        &self,
        action: &StepAction,
        parameters: &HashMap<String, String>,
    ) -> Result<Value, ScriptExecutionError> {
        match action {
            StepAction::CheckBalance { assets } => {
                let filter = if assets.is_empty() {
                    None
                } else {
                    Some(assets.join(","))
                };

                self.sdk_adapter
                    .get_balances_filtered(filter)
                    .await
                    .map_err(|e| ScriptExecutionError::SdkError(e.to_string()))
            }

            StepAction::GetPools { filter } => self
                .sdk_adapter
                .get_pools_filtered(filter.clone(), None, None)
                .await
                .map_err(|e| ScriptExecutionError::SdkError(e.to_string())),

            StepAction::GetPool { pool_id } => self
                .sdk_adapter
                .get_pool_info(pool_id.clone())
                .await
                .map_err(|e| ScriptExecutionError::SdkError(e.to_string())),

            StepAction::ExecuteSwap {
                from_asset,
                to_asset,
                amount,
                slippage,
            } => {
                self.sdk_adapter
                    .execute_swap_simple(
                        from_asset.clone(),
                        to_asset.clone(),
                        amount.clone(),
                        slippage.clone(),
                        None, // pool_id
                        None, // min_output
                    )
                    .await
                    .map_err(|e| ScriptExecutionError::SdkError(e.to_string()))
            }

            StepAction::ProvideLiquidity {
                pool_id,
                asset_a_amount,
                asset_b_amount,
            } => {
                self.sdk_adapter
                    .provide_liquidity_simple(
                        pool_id.clone(),
                        asset_a_amount.clone(),
                        asset_b_amount.clone(),
                        None, // min_lp_tokens
                        None, // slippage
                    )
                    .await
                    .map_err(|e| ScriptExecutionError::SdkError(e.to_string()))
            }

            StepAction::WithdrawLiquidity { pool_id, lp_amount } => {
                self.sdk_adapter
                    .withdraw_liquidity_simple(
                        pool_id.clone(),
                        lp_amount.clone(),
                        None, // min_asset_a
                        None, // min_asset_b
                    )
                    .await
                    .map_err(|e| ScriptExecutionError::SdkError(e.to_string()))
            }

            StepAction::CreatePool {
                asset_a,
                asset_b,
                initial_price,
            } => {
                self.sdk_adapter
                    .create_pool_simple(
                        asset_a.clone(),
                        asset_b.clone(),
                        initial_price.clone(),
                        None, // pool_type
                        None, // fee_rate
                    )
                    .await
                    .map_err(|e| ScriptExecutionError::SdkError(e.to_string()))
            }

            StepAction::MonitorTransaction { tx_hash, timeout } => {
                self.sdk_adapter
                    .monitor_transaction(tx_hash.clone(), Some(*timeout))
                    .await
                    .map_err(|e| ScriptExecutionError::SdkError(e.to_string()))
            }

            StepAction::ValidateNetwork => self
                .sdk_adapter
                .validate_network_connectivity()
                .await
                .map_err(|e| ScriptExecutionError::SdkError(e.to_string())),

            StepAction::GetContracts => self
                .sdk_adapter
                .get_contract_addresses()
                .await
                .map_err(|e| ScriptExecutionError::SdkError(e.to_string())),

            StepAction::Custom {
                tool_name,
                parameters,
            } => {
                self.sdk_adapter
                    .execute_custom_tool(tool_name, parameters)
                    .await
                    .map_err(|e| ScriptExecutionError::SdkError(e.to_string()))
            }
        }
    }

    /// Validate step outcome against expected result
    fn validate_outcome(&self, expected: &str, actual: &Value) -> OutcomeValidation {
        let actual_str = actual.to_string();
        let expected_lower = expected.to_lowercase();
        let actual_lower = actual_str.to_lowercase();

        // Simple validation logic - can be enhanced
        let matches = if expected_lower.contains("success") {
            actual_lower.contains("success") || actual_lower.contains("ok")
        } else if expected_lower.contains("balance") {
            actual_lower.contains("balance") || actual_lower.contains("amount")
        } else if expected_lower.contains("pool") {
            actual_lower.contains("pool")
        } else if expected_lower.contains("swap") {
            actual_lower.contains("swap") || actual_lower.contains("trade")
        } else {
            // Default: check if expected text is contained in actual
            actual_lower.contains(&expected_lower)
        };

        OutcomeValidation {
            expected: expected.to_string(),
            actual: actual_str,
            matches,
            notes: None,
        }
    }

    /// Create execution summary
    fn create_execution_summary(&self, step_results: &[StepExecutionResult]) -> ExecutionSummary {
        let total_steps = step_results.len();
        let successful_steps = step_results
            .iter()
            .filter(|r| r.status == ExecutionStatus::Success)
            .count();
        let failed_steps = step_results
            .iter()
            .filter(|r| r.status == ExecutionStatus::Failed)
            .count();
        let skipped_steps = total_steps - successful_steps - failed_steps;

        let pass_rate = if total_steps > 0 {
            (successful_steps as f64 / total_steps as f64) * 100.0
        } else {
            0.0
        };

        let mut metrics = HashMap::new();
        metrics.insert(
            "total_steps".to_string(),
            Value::Number(serde_json::Number::from(total_steps)),
        );
        metrics.insert(
            "successful_steps".to_string(),
            Value::Number(serde_json::Number::from(successful_steps)),
        );
        metrics.insert(
            "failed_steps".to_string(),
            Value::Number(serde_json::Number::from(failed_steps)),
        );
        metrics.insert(
            "pass_rate".to_string(),
            Value::Number(
                serde_json::Number::from_f64(pass_rate).unwrap_or(serde_json::Number::from(0)),
            ),
        );

        ExecutionSummary {
            total_steps,
            successful_steps,
            failed_steps,
            skipped_steps,
            pass_rate,
            metrics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::script_parser::ScriptParser;

    #[tokio::test]
    async fn test_script_execution_result_creation() {
        let script_content = r#"
# Test Script: Simple Test

## Setup
- Network: mantra-dukong

## Steps
1. **Validate network** connectivity
2. **Get contracts** addresses

## Expected Results
- Network should be accessible
- Contracts should be available
"#;

        let script = ScriptParser::parse_content(script_content).unwrap();
        assert_eq!(script.name, "Simple Test");
        assert_eq!(script.steps.len(), 2);
    }
}
