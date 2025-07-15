use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, error, info};

use super::script_parser::{StepAction, TestScript, TestStep};
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
        _parameters: &HashMap<String, String>,
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
                pool_id,
            } => {
                self.sdk_adapter
                    .execute_swap_simple(
                        from_asset.clone(),
                        to_asset.clone(),
                        amount.clone(),
                        slippage.clone(),
                        pool_id.clone(),
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
                        None, // liquidity_slippage
                        None, // swap_slippage
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

    /// Validate step outcome against expected result with enhanced logic
    fn validate_outcome(&self, expected: &str, actual: &Value) -> OutcomeValidation {
        let actual_str = actual.to_string();
        let mut validation_notes = Vec::new();
        
        // Try different validation strategies in order of specificity
        let (matches, strategy_used) = self.try_validation_strategies(expected, actual, &mut validation_notes);
        
        OutcomeValidation {
            expected: expected.to_string(),
            actual: actual_str,
            matches,
            notes: if validation_notes.is_empty() {
                Some(format!("Validated using: {}", strategy_used))
            } else {
                Some(format!("{} | Notes: {}", strategy_used, validation_notes.join("; ")))
            },
        }
    }
    
    /// Try multiple validation strategies in order of sophistication
    fn try_validation_strategies(&self, expected: &str, actual: &Value, notes: &mut Vec<String>) -> (bool, String) {
        // Strategy 1: Regex pattern matching
        if let Some(matches) = self.validate_with_regex(expected, actual, notes) {
            return (matches, "regex pattern".to_string());
        }
        
        // Strategy 2: Structured data validation (JSON comparison)
        if let Some(matches) = self.validate_structured_data(expected, actual, notes) {
            return (matches, "structured data".to_string());
        }
        
        // Strategy 3: Numeric comparison
        if let Some(matches) = self.validate_numeric_comparison(expected, actual, notes) {
            return (matches, "numeric comparison".to_string());
        }
        
        // Strategy 4: Domain-specific DEX validation
        if let Some(matches) = self.validate_dex_specific(expected, actual, notes) {
            return (matches, "DEX domain rules".to_string());
        }
        
        // Strategy 5: Enhanced string matching (fallback)
        let matches = self.validate_enhanced_string_matching(expected, actual, notes);
        (matches, "enhanced string matching".to_string())
    }
    
    /// Validate using regex patterns
    fn validate_with_regex(&self, expected: &str, actual: &Value, notes: &mut Vec<String>) -> Option<bool> {
        // Check if expected is a regex pattern (starts with regex: or /)
        let pattern = if expected.starts_with("regex:") {
            &expected[6..]
        } else if expected.starts_with('/') && expected.ends_with('/') && expected.len() > 2 {
            &expected[1..expected.len()-1]
        } else {
            return None;
        };
        
        match Regex::new(pattern) {
            Ok(re) => {
                let actual_str = match actual {
                    Value::String(s) => s.clone(),
                    _ => actual.to_string(),
                };
                let matches = re.is_match(&actual_str);
                notes.push(format!("regex pattern: {}", pattern));
                Some(matches)
            }
            Err(e) => {
                notes.push(format!("invalid regex pattern: {}", e));
                None
            }
        }
    }
    
    /// Validate structured data (JSON objects)
    fn validate_structured_data(&self, expected: &str, actual: &Value, notes: &mut Vec<String>) -> Option<bool> {
        // Try to parse expected as JSON
        if let Ok(expected_json) = serde_json::from_str::<Value>(expected) {
            let matches = match (&expected_json, actual) {
                // Both are objects - check key-value pairs
                (Value::Object(exp_obj), Value::Object(act_obj)) => {
                    self.validate_json_objects(exp_obj, act_obj, notes)
                }
                // Both are arrays - check elements
                (Value::Array(exp_arr), Value::Array(act_arr)) => {
                    self.validate_json_arrays(exp_arr, act_arr, notes)
                }
                // Direct value comparison
                (exp, act) => {
                    let matches = exp == act;
                    if !matches {
                        notes.push(format!("JSON values differ: expected {:?}, got {:?}", exp, act));
                    }
                    matches
                }
            };
            notes.push("JSON structure comparison".to_string());
            Some(matches)
        } else {
            None
        }
    }
    
    /// Validate JSON objects by checking key-value pairs
    fn validate_json_objects(&self, expected: &serde_json::Map<String, Value>, actual: &serde_json::Map<String, Value>, notes: &mut Vec<String>) -> bool {
        let mut all_match = true;
        
        for (key, exp_value) in expected {
            match actual.get(key) {
                Some(act_value) => {
                    if exp_value != act_value {
                        notes.push(format!("key '{}' differs: expected {:?}, got {:?}", key, exp_value, act_value));
                        all_match = false;
                    }
                }
                None => {
                    notes.push(format!("missing key: '{}'", key));
                    all_match = false;
                }
            }
        }
        
        all_match
    }
    
    /// Validate JSON arrays
    fn validate_json_arrays(&self, expected: &[Value], actual: &[Value], notes: &mut Vec<String>) -> bool {
        if expected.len() != actual.len() {
            notes.push(format!("array length differs: expected {}, got {}", expected.len(), actual.len()));
            return false;
        }
        
        for (i, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
            if exp != act {
                notes.push(format!("array element {} differs: expected {:?}, got {:?}", i, exp, act));
                return false;
            }
        }
        
        true
    }
    
    /// Validate numeric comparisons
    fn validate_numeric_comparison(&self, expected: &str, actual: &Value, notes: &mut Vec<String>) -> Option<bool> {
        // Check for numeric comparison operators
        let patterns = [
            (r"^>=\s*(\d+(?:\.\d+)?)$", ">="),
            (r"^<=\s*(\d+(?:\.\d+)?)$", "<="),
            (r"^>\s*(\d+(?:\.\d+)?)$", ">"),
            (r"^<\s*(\d+(?:\.\d+)?)$", "<"),
            (r"^=\s*(\d+(?:\.\d+)?)$", "="),
            (r"^(\d+(?:\.\d+)?)\s*-\s*(\d+(?:\.\d+)?)$", "range"),
        ];
        
        for (pattern, op) in &patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(captures) = re.captures(expected) {
                    return self.perform_numeric_comparison(op, &captures, actual, notes);
                }
            }
        }
        
        None
    }
    
    /// Perform the actual numeric comparison
    fn perform_numeric_comparison(&self, operator: &str, captures: &regex::Captures, actual: &Value, notes: &mut Vec<String>) -> Option<bool> {
        let actual_num = match actual {
            Value::Number(n) => n.as_f64()?,
            Value::String(s) => s.parse::<f64>().ok()?,
            _ => return None,
        };
        
        let result = match operator {
            ">=" => {
                let threshold = captures.get(1)?.as_str().parse::<f64>().ok()?;
                actual_num >= threshold
            }
            "<=" => {
                let threshold = captures.get(1)?.as_str().parse::<f64>().ok()?;
                actual_num <= threshold
            }
            ">" => {
                let threshold = captures.get(1)?.as_str().parse::<f64>().ok()?;
                actual_num > threshold
            }
            "<" => {
                let threshold = captures.get(1)?.as_str().parse::<f64>().ok()?;
                actual_num < threshold
            }
            "=" => {
                let threshold = captures.get(1)?.as_str().parse::<f64>().ok()?;
                (actual_num - threshold).abs() < f64::EPSILON
            }
            "range" => {
                let min = captures.get(1)?.as_str().parse::<f64>().ok()?;
                let max = captures.get(2)?.as_str().parse::<f64>().ok()?;
                actual_num >= min && actual_num <= max
            }
            _ => return None,
        };
        
        notes.push(format!("numeric comparison: {} {} with actual value {}", operator, captures.get(0)?.as_str(), actual_num));
        Some(result)
    }
    
    /// Validate DEX-specific domain rules
    fn validate_dex_specific(&self, expected: &str, actual: &Value, notes: &mut Vec<String>) -> Option<bool> {
        let expected_lower = expected.to_lowercase();
        let actual_str = actual.to_string().to_lowercase();
        
        // DEX operation success patterns
        if expected_lower.contains("success") || expected_lower.contains("successful") {
            let success_indicators = [
                "success", "successful", "completed", "ok", "confirmed",
                "transaction_hash", "tx_hash", "block_height"
            ];
            let matches = success_indicators.iter().any(|&indicator| actual_str.contains(indicator));
            notes.push("DEX success validation".to_string());
            return Some(matches);
        }
        
        // Balance/amount validation
        if expected_lower.contains("balance") || expected_lower.contains("amount") {
            let balance_indicators = ["balance", "amount", "quantity", "value", "uom"];
            let has_balance_field = balance_indicators.iter().any(|&indicator| actual_str.contains(indicator));
            
            // Also check for numeric values
            let has_numeric = Regex::new(r"\d+").unwrap().is_match(&actual_str);
            
            let matches = has_balance_field || has_numeric;
            notes.push("balance/amount validation".to_string());
            return Some(matches);
        }
        
        // Pool operation validation
        if expected_lower.contains("pool") {
            let pool_indicators = ["pool", "liquidity", "lp_token", "pool_id"];
            let matches = pool_indicators.iter().any(|&indicator| actual_str.contains(indicator));
            notes.push("pool operation validation".to_string());
            return Some(matches);
        }
        
        // Swap/trade validation
        if expected_lower.contains("swap") || expected_lower.contains("trade") {
            let swap_indicators = ["swap", "trade", "exchange", "from_token", "to_token", "slippage"];
            let matches = swap_indicators.iter().any(|&indicator| actual_str.contains(indicator));
            notes.push("swap/trade validation".to_string());
            return Some(matches);
        }
        
        // Address validation
        if expected_lower.contains("address") {
            let address_pattern = Regex::new(r"[a-z0-9]{39,}").unwrap();
            let actual_check = match actual {
                Value::String(s) => s.clone(),
                _ => actual.to_string(),
            };
            let matches = address_pattern.is_match(&actual_check);
            notes.push("address validation".to_string());
            return Some(matches);
        }
        
        None
    }
    
    /// Enhanced string matching with fuzzy logic
    fn validate_enhanced_string_matching(&self, expected: &str, actual: &Value, notes: &mut Vec<String>) -> bool {
        let expected_lower = expected.to_lowercase();
        
        // Extract string value directly if it's a JSON string to avoid quotes
        let actual_str = match actual {
            Value::String(s) => s.clone(),
            _ => actual.to_string(),
        };
        let actual_lower = actual_str.to_lowercase();
        
        // Exact match
        if expected_lower == actual_lower {
            notes.push("exact match".to_string());
            return true;
        }
        
        // Contains match
        if actual_lower.contains(&expected_lower) {
            notes.push("substring match".to_string());
            return true;
        }
        
        // Word boundary matching
        let expected_words: Vec<&str> = expected_lower.split_whitespace().collect();
        let actual_words: Vec<&str> = actual_lower.split_whitespace().collect();
        
        let word_matches = expected_words.iter()
            .filter(|&word| actual_words.contains(word))
            .count();
        
        let word_match_ratio = word_matches as f64 / expected_words.len() as f64;
        
        if word_match_ratio >= 0.5 {
            notes.push(format!("word match ratio: {:.2}", word_match_ratio));
            return true;
        }
        
        // Fuzzy matching for common variations
        let synonyms = [
            ("ok", "success"),
            ("successful", "success"),
            ("completed", "success"),
            ("confirmed", "success"),
            ("amount", "balance"),
            ("quantity", "balance"),
            ("value", "balance"),
            ("trade", "swap"),
            ("exchange", "swap"),
        ];
        
        for (syn1, syn2) in &synonyms {
            if (expected_lower.contains(syn1) && actual_lower.contains(syn2)) ||
               (expected_lower.contains(syn2) && actual_lower.contains(syn1)) {
                notes.push(format!("synonym match: {} <-> {}", syn1, syn2));
                return true;
            }
        }
        
        notes.push("no match found".to_string());
        false
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

    #[tokio::test]
    async fn test_enhanced_validation_regex_patterns() {
        use super::*;
        use crate::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
        use std::sync::Arc;
        
        let config = ConnectionPoolConfig::default();
        let adapter = Arc::new(McpSdkAdapter::new(config));
        let runner = ScriptRunner::new(adapter);
        
        // Test regex validation
        let expected = "regex:^[a-z]+$";
        let actual = serde_json::Value::String("hello".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("regex pattern"));
        
        // Test regex with slash delimiters
        let expected = "/^\\d+$/";
        let actual = serde_json::Value::String("12345".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        
        // Test failed regex match
        let expected = "regex:^\\d+$";
        let actual = serde_json::Value::String("abc".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(!result.matches);
    }

    #[tokio::test]
    async fn test_enhanced_validation_structured_data() {
        use super::*;
        use crate::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
        use std::sync::Arc;
        
        let config = ConnectionPoolConfig::default();
        let adapter = Arc::new(McpSdkAdapter::new(config));
        let runner = ScriptRunner::new(adapter);
        
        // Test JSON object validation
        let expected = r#"{"status": "success", "amount": "1000"}"#;
        let actual = serde_json::json!({"status": "success", "amount": "1000", "extra": "field"});
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("JSON structure"));
        
        // Test JSON array validation
        let expected = r#"["item1", "item2"]"#;
        let actual = serde_json::json!(["item1", "item2"]);
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        
        // Test JSON mismatch
        let expected = r#"{"status": "failed"}"#;
        let actual = serde_json::json!({"status": "success"});
        let result = runner.validate_outcome(expected, &actual);
        assert!(!result.matches);
    }

    #[tokio::test]
    async fn test_enhanced_validation_numeric_comparison() {
        use super::*;
        use crate::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
        use std::sync::Arc;
        
        let config = ConnectionPoolConfig::default();
        let adapter = Arc::new(McpSdkAdapter::new(config));
        let runner = ScriptRunner::new(adapter);
        
        // Test greater than
        let expected = "> 100";
        let actual = serde_json::Value::Number(serde_json::Number::from(150));
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("numeric comparison"));
        
        // Test range validation
        let expected = "100 - 200";
        let actual = serde_json::Value::Number(serde_json::Number::from(150));
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        
        // Test less than equal
        let expected = "<= 50";
        let actual = serde_json::Value::Number(serde_json::Number::from(60));
        let result = runner.validate_outcome(expected, &actual);
        assert!(!result.matches);
        
        // Test string numeric parsing
        let expected = ">= 1000";
        let actual = serde_json::Value::String("1500".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
    }

    #[tokio::test]
    async fn test_enhanced_validation_dex_specific() {
        use super::*;
        use crate::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
        use std::sync::Arc;
        
        let config = ConnectionPoolConfig::default();
        let adapter = Arc::new(McpSdkAdapter::new(config));
        let runner = ScriptRunner::new(adapter);
        
        // Test DEX success validation
        let expected = "successful swap";
        let actual = serde_json::json!({"transaction_hash": "abc123", "status": "confirmed"});
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("DEX success"));
        
        // Test balance validation
        let expected = "balance check";
        let actual = serde_json::json!({"balance": "1000", "denom": "uom"});
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("balance/amount"));
        
        // Test pool validation
        let expected = "pool operation";
        let actual = serde_json::json!({"pool_id": "123", "liquidity": "5000"});
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("pool operation"));
        
        // Test swap validation
        let expected = "swap executed";
        let actual = serde_json::json!({"from_token": "uom", "to_token": "uatom", "slippage": "0.5"});
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("swap/trade"));
        
        // Test address validation
        let expected = "valid address";
        let actual = serde_json::json!("mantra1abcdefghijklmnopqrstuvwxyz1234567890abcdefg");
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("address validation"));
    }

    #[tokio::test]
    async fn test_enhanced_validation_string_matching() {
        use super::*;
        use crate::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
        use std::sync::Arc;
        
        let config = ConnectionPoolConfig::default();
        let adapter = Arc::new(McpSdkAdapter::new(config));
        let runner = ScriptRunner::new(adapter);
        
        // Test exact match
        let expected = "exact match";
        let actual = serde_json::Value::String("exact match".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("exact match"));
        
        // Test substring match - expected is contained in actual
        let expected = "completed";
        let actual = serde_json::Value::String("operation completed successfully".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("substring match"));
        
        // Test word ratio matching - different words but many matches
        let expected = "swap operation completed";
        let actual = serde_json::Value::String("trade operation was completed successfully".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        // Should match as DEX specific first due to "swap" keyword
        assert!(result.notes.as_ref().unwrap().contains("DEX domain rules") || result.notes.as_ref().unwrap().contains("word match ratio"));
        
        // Test synonym matching
        let expected = "ok";
        let actual = serde_json::Value::String("operation was successful".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("synonym match"));
        
        // Test no match
        let expected = "failure";
        let actual = serde_json::Value::String("great success".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(!result.matches);
        assert!(result.notes.as_ref().unwrap().contains("no match found"));
    }

    #[tokio::test]
    async fn test_validation_strategy_priority() {
        use super::*;
        use crate::mcp::sdk_adapter::{ConnectionPoolConfig, McpSdkAdapter};
        use std::sync::Arc;
        
        let config = ConnectionPoolConfig::default();
        let adapter = Arc::new(McpSdkAdapter::new(config));
        let runner = ScriptRunner::new(adapter);
        
        // Test that regex takes priority over string matching
        let expected = "regex:^success$";
        let actual = serde_json::Value::String("success".to_string());
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("regex pattern"));
        
        // Test that structured data takes priority over string matching
        let expected = r#"{"status": "ok"}"#;
        let actual = serde_json::json!({"status": "ok"});
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("structured data"));
        
        // Test that numeric comparison takes priority over string matching
        let expected = "> 0";
        let actual = serde_json::Value::Number(serde_json::Number::from(42));
        let result = runner.validate_outcome(expected, &actual);
        assert!(result.matches);
        assert!(result.notes.as_ref().unwrap().contains("numeric comparison"));
    }
}
