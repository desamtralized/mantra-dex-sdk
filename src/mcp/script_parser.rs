use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info, warn};
use regex::Regex;

/// Represents a parsed test script with all its components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScript {
    /// Script name/title
    pub name: String,
    /// Script description
    pub description: Option<String>,
    /// Setup configuration
    pub setup: ScriptSetup,
    /// Ordered list of test steps
    pub steps: Vec<TestStep>,
    /// Expected results
    pub expected_results: Vec<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Script setup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSetup {
    /// Network to use (e.g., "mantra-dukong")
    pub network: String,
    /// Wallet configuration
    pub wallet: WalletConfig,
    /// Additional setup parameters
    pub parameters: HashMap<String, String>,
}

/// Wallet configuration for script execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Wallet type (e.g., "test", "mnemonic", "private_key")
    pub wallet_type: String,
    /// Wallet identifier or path
    pub identifier: Option<String>,
    /// Required minimum balances for assets
    pub minimum_balances: HashMap<String, String>,
}

/// Individual test step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    /// Step number
    pub step_number: usize,
    /// Step description
    pub description: String,
    /// Action to perform
    pub action: StepAction,
    /// Step parameters
    pub parameters: HashMap<String, String>,
    /// Expected outcome
    pub expected_outcome: Option<String>,
    /// Timeout for this step (seconds)
    pub timeout: Option<u64>,
}

/// Available step actions that map to MCP tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepAction {
    /// Check wallet balance
    CheckBalance { assets: Vec<String> },
    /// Get available pools
    GetPools { filter: Option<String> },
    /// Get specific pool information
    GetPool { pool_id: String },
    /// Execute a token swap
    ExecuteSwap {
        from_asset: String,
        to_asset: String,
        amount: String,
        slippage: String,
    },
    /// Provide liquidity to a pool
    ProvideLiquidity {
        pool_id: String,
        asset_a_amount: String,
        asset_b_amount: String,
    },
    /// Withdraw liquidity from a pool
    WithdrawLiquidity { pool_id: String, lp_amount: String },
    /// Create a new pool
    CreatePool {
        asset_a: String,
        asset_b: String,
        initial_price: String,
    },
    /// Monitor a transaction
    MonitorTransaction { tx_hash: String, timeout: u64 },
    /// Validate network connectivity
    ValidateNetwork,
    /// Get contract addresses
    GetContracts,
    /// Custom action with arbitrary parameters
    Custom {
        tool_name: String,
        parameters: HashMap<String, String>,
    },
}

/// Script parsing errors
#[derive(Debug, thiserror::Error)]
pub enum ScriptParseError {
    #[error("Failed to read script file: {0}")]
    FileReadError(#[from] std::io::Error),
    #[error("Invalid script format: {0}")]
    InvalidFormat(String),
    #[error("Missing required section: {0}")]
    MissingSection(String),
    #[error("Invalid step format at line {line}: {msg}")]
    InvalidStep { line: usize, msg: String },
    #[error("Unsupported action: {0}")]
    UnsupportedAction(String),
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

/// Script parser implementation
pub struct ScriptParser;

impl ScriptParser {
    /// Parse a markdown script file
    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<TestScript, ScriptParseError> {
        let content = fs::read_to_string(path)?;
        Self::parse_content(&content)
    }

    /// Parse script content from a string
    pub fn parse_content(content: &str) -> Result<TestScript, ScriptParseError> {
        let lines: Vec<&str> = content.lines().collect();
        let mut script = TestScript {
            name: String::new(),
            description: None,
            setup: ScriptSetup {
                network: "mantra-dukong".to_string(),
                wallet: WalletConfig {
                    wallet_type: "test".to_string(),
                    identifier: None,
                    minimum_balances: HashMap::new(),
                },
                parameters: HashMap::new(),
            },
            steps: Vec::new(),
            expected_results: Vec::new(),
            metadata: HashMap::new(),
        };

        let mut current_section = ScriptSection::None;
        let mut current_step_lines = Vec::new();
        let mut step_counter = 0;

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Handle section headers
            if trimmed.starts_with('#') {
                // Process any pending step
                if !current_step_lines.is_empty() {
                    step_counter += 1;
                    let step = Self::parse_step(step_counter, &current_step_lines, line_num)?;
                    script.steps.push(step);
                    current_step_lines.clear();
                }

                current_section = Self::parse_section_header(trimmed, &mut script)?;
                continue;
            }

            // Handle content based on current section
            match current_section {
                ScriptSection::None => {
                    // Skip content before first header
                    continue;
                }
                ScriptSection::Title => {
                    // Title is handled in parse_section_header
                    continue;
                }
                ScriptSection::Setup => {
                    Self::parse_setup_line(trimmed, &mut script.setup)?;
                }
                ScriptSection::Steps => {
                    if trimmed.starts_with("1.")
                        || trimmed.starts_with("2.")
                        || trimmed.starts_with("3.")
                        || trimmed.starts_with("4.")
                        || trimmed.starts_with("5.")
                        || trimmed.starts_with("6.")
                        || trimmed.starts_with("7.")
                        || trimmed.starts_with("8.")
                        || trimmed.starts_with("9.")
                        || trimmed.starts_with("10.")
                        || (trimmed.len() > 2
                            && trimmed.chars().nth(1) == Some('.')
                            && trimmed.chars().nth(0).unwrap().is_ascii_digit())
                    {
                        // New step detected, process previous step if any
                        if !current_step_lines.is_empty() {
                            step_counter += 1;
                            let step =
                                Self::parse_step(step_counter, &current_step_lines, line_num)?;
                            script.steps.push(step);
                            current_step_lines.clear();
                        }
                        current_step_lines.push(trimmed.to_string());
                    } else {
                        // Continuation of current step
                        current_step_lines.push(trimmed.to_string());
                    }
                }
                ScriptSection::ExpectedResults => {
                    if trimmed.starts_with('-') {
                        script
                            .expected_results
                            .push(trimmed[1..].trim().to_string());
                    } else {
                        script.expected_results.push(trimmed.to_string());
                    }
                }
                ScriptSection::Metadata => {
                    if let Some(colon_pos) = trimmed.find(':') {
                        let key = trimmed[..colon_pos].trim().to_string();
                        let value = trimmed[colon_pos + 1..].trim().to_string();
                        script.metadata.insert(key, value);
                    }
                }
            }
        }

        // Process final step if any
        if !current_step_lines.is_empty() {
            step_counter += 1;
            let step = Self::parse_step(step_counter, &current_step_lines, lines.len())?;
            script.steps.push(step);
        }

        // Validate script
        Self::validate_script(&script)?;

        Ok(script)
    }

    /// Parse section header and update script accordingly
    fn parse_section_header(
        header: &str,
        script: &mut TestScript,
    ) -> Result<ScriptSection, ScriptParseError> {
        let header_text = header.trim_start_matches('#').trim();

        if header_text.to_lowercase().contains("test script:") {
            let re = Regex::new(r"(?i)test script:").unwrap();
            script.name = re.replace(header_text, "").trim().to_string();
            return Ok(ScriptSection::Title);
        }

        match header_text.to_lowercase().as_str() {
            "setup" => Ok(ScriptSection::Setup),
            "steps" => Ok(ScriptSection::Steps),
            "expected results" => Ok(ScriptSection::ExpectedResults),
            "metadata" => Ok(ScriptSection::Metadata),
            _ => {
                if header.starts_with("# ") {
                    script.name = header_text.to_string();
                    Ok(ScriptSection::Title)
                } else {
                    Ok(ScriptSection::None)
                }
            }
        }
    }

    /// Parse setup section line
    fn parse_setup_line(line: &str, setup: &mut ScriptSetup) -> Result<(), ScriptParseError> {
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim().to_lowercase();
            let value = line[colon_pos + 1..].trim();

            match key.as_str() {
                "network" => {
                    setup.network = value.to_string();
                }
                "wallet" => {
                    setup.wallet.wallet_type = value.to_string();
                }
                "wallet_type" => {
                    setup.wallet.wallet_type = value.to_string();
                }
                "wallet_identifier" => {
                    setup.wallet.identifier = Some(value.to_string());
                }
                _ => {
                    setup.parameters.insert(key, value.to_string());
                }
            }
        } else if line.starts_with('-') {
            // Handle bullet points
            let content = line[1..].trim();
            if let Some(colon_pos) = content.find(':') {
                let key = content[..colon_pos].trim().to_lowercase();
                let value = content[colon_pos + 1..].trim();

                match key.as_str() {
                    "network" => setup.network = value.to_string(),
                    "wallet" => setup.wallet.wallet_type = value.to_string(),
                    _ => {
                        setup.parameters.insert(key, value.to_string());
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse a single test step
    fn parse_step(
        step_number: usize,
        lines: &[String],
        line_num: usize,
    ) -> Result<TestStep, ScriptParseError> {
        if lines.is_empty() {
            return Err(ScriptParseError::InvalidStep {
                line: line_num,
                msg: "Empty step".to_string(),
            });
        }

        let first_line = &lines[0];
        let description = Self::extract_step_description(first_line)?;
        let action = Self::parse_step_action(&description, lines)?;

        let mut parameters = HashMap::new();
        let mut expected_outcome = None;
        let mut timeout = None;

        // Parse additional lines for parameters and expected outcomes
        for line in &lines[1..] {
            if line.trim().starts_with("Expected:") {
                expected_outcome = Some(line.trim()[9..].trim().to_string());
            } else if line.trim().starts_with("Timeout:") {
                if let Ok(t) = line.trim()[8..].trim().parse::<u64>() {
                    timeout = Some(t);
                }
            } else if line.contains(':') {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    parameters.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                }
            }
        }

        Ok(TestStep {
            step_number,
            description,
            action,
            parameters,
            expected_outcome,
            timeout,
        })
    }

    /// Extract description from step line
    fn extract_step_description(line: &str) -> Result<String, ScriptParseError> {
        if let Some(dot_pos) = line.find('.') {
            let desc = line[dot_pos + 1..].trim();
            if desc.starts_with("**") && desc.contains("**") {
                // Extract text between ** markers
                let start = desc.find("**").unwrap() + 2;
                if let Some(end) = desc[start..].find("**") {
                    return Ok(desc[start..start + end].trim().to_string());
                }
            }
            Ok(desc.to_string())
        } else {
            Ok(line.trim().to_string())
        }
    }

    /// Parse step action from description
    fn parse_step_action(
        description: &str,
        lines: &[String],
    ) -> Result<StepAction, ScriptParseError> {
        let desc_lower = description.to_lowercase();

        if desc_lower.contains("check") && desc_lower.contains("balance") {
            let assets = Self::extract_assets_from_description(description);
            return Ok(StepAction::CheckBalance { assets });
        }

        if desc_lower.contains("get") && desc_lower.contains("pools") {
            let filter = Self::extract_filter_from_description(description);
            return Ok(StepAction::GetPools { filter });
        }

        if desc_lower.contains("get")
            && desc_lower.contains("pool")
            && !desc_lower.contains("pools")
        {
            let pool_id = Self::extract_pool_id_from_description(description);
            return Ok(StepAction::GetPool { pool_id });
        }

        if desc_lower.contains("execute") && desc_lower.contains("swap") {
            let (from_asset, to_asset, amount, slippage) =
                Self::extract_swap_params(description, lines)?;
            return Ok(StepAction::ExecuteSwap {
                from_asset,
                to_asset,
                amount,
                slippage,
            });
        }

        if desc_lower.contains("provide") && desc_lower.contains("liquidity") {
            let (pool_id, asset_a_amount, asset_b_amount) =
                Self::extract_liquidity_params(description, lines)?;
            return Ok(StepAction::ProvideLiquidity {
                pool_id,
                asset_a_amount,
                asset_b_amount,
            });
        }

        if desc_lower.contains("withdraw") && desc_lower.contains("liquidity") {
            let (pool_id, lp_amount) = Self::extract_withdrawal_params(description, lines)?;
            return Ok(StepAction::WithdrawLiquidity { pool_id, lp_amount });
        }

        if desc_lower.contains("create") && desc_lower.contains("pool") {
            let (asset_a, asset_b, initial_price) =
                Self::extract_pool_creation_params(description, lines)?;
            return Ok(StepAction::CreatePool {
                asset_a,
                asset_b,
                initial_price,
            });
        }

        if desc_lower.contains("monitor") && desc_lower.contains("transaction") {
            let (tx_hash, timeout) = Self::extract_monitor_params(description, lines)?;
            return Ok(StepAction::MonitorTransaction { tx_hash, timeout });
        }

        if desc_lower.contains("validate") && desc_lower.contains("network") {
            return Ok(StepAction::ValidateNetwork);
        }

        if desc_lower.contains("get") && desc_lower.contains("contract") {
            return Ok(StepAction::GetContracts);
        }

        // Default to custom action
        let mut parameters = HashMap::new();
        for line in lines {
            if line.contains(':') {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    parameters.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
                }
            }
        }

        Ok(StepAction::Custom {
            tool_name: "unknown".to_string(),
            parameters,
        })
    }

    /// Extract assets from description
    fn extract_assets_from_description(description: &str) -> Vec<String> {
        let mut assets = Vec::new();
        let desc_upper = description.to_uppercase();

        // Common asset patterns
        let asset_patterns = ["ATOM", "USDC", "USDT", "BTC", "ETH", "MANTRA", "OM"];

        for pattern in &asset_patterns {
            if desc_upper.contains(pattern) {
                assets.push(pattern.to_string());
            }
        }

        if assets.is_empty() {
            assets.push("ATOM".to_string());
        }

        assets
    }

    /// Extract filter from description
    fn extract_filter_from_description(description: &str) -> Option<String> {
        // Look for "find" or "filter" keywords
        if description.contains("find") || description.contains("filter") {
            // Extract the filter criteria
            if let Some(start) = description.find("find ") {
                let remaining = &description[start + 5..];
                if let Some(end) = remaining.find(" ") {
                    return Some(remaining[..end].to_string());
                }
            }
        }
        None
    }

    /// Extract pool ID from description
    fn extract_pool_id_from_description(description: &str) -> String {
        // Look for pool identifier patterns
        if description.contains("ATOM/USDC") {
            "ATOM/USDC".to_string()
        } else if description.contains("pool") {
            // Extract pool identifier after "pool"
            "1".to_string() // Default pool ID
        } else {
            "1".to_string()
        }
    }

    /// Extract swap parameters
    fn extract_swap_params(
        description: &str,
        lines: &[String],
    ) -> Result<(String, String, String, String), ScriptParseError> {
        let mut from_asset = "ATOM".to_string();
        let mut to_asset = "USDC".to_string();
        let mut amount = "10".to_string();
        let mut slippage = "1".to_string();

        // Parse from description
        if let Some(of_pos) = description.find(" of ") {
            let after_of = &description[of_pos + 4..];
            if let Some(space_pos) = after_of.find(' ') {
                let amount_and_asset = &after_of[..space_pos];
                if let Some(asset_start) = amount_and_asset.find(|c: char| c.is_alphabetic()) {
                    amount = amount_and_asset[..asset_start].trim().to_string();
                    from_asset = amount_and_asset[asset_start..].trim().to_string();
                }
            }
        }

        if let Some(for_pos) = description.find(" for ") {
            let after_for = &description[for_pos + 5..];
            if let Some(space_pos) = after_for.find(' ') {
                to_asset = after_for[..space_pos].trim().to_string();
            } else {
                to_asset = after_for.trim().to_string();
            }
        }

        if description.contains("slippage") {
            if let Some(slippage_pos) = description.find("slippage") {
                let after_slippage = &description[slippage_pos + 8..];
                if let Some(percent_pos) = after_slippage.find('%') {
                    slippage = after_slippage[..percent_pos].trim().to_string();
                }
            }
        }

        // Check additional lines for parameters
        for line in lines {
            if line.contains("amount:") {
                amount = line.split(':').nth(1).unwrap_or("10").trim().to_string();
            } else if line.contains("slippage:") {
                slippage = line.split(':').nth(1).unwrap_or("1").trim().to_string();
            }
        }

        Ok((from_asset, to_asset, amount, slippage))
    }

    /// Extract liquidity parameters
    fn extract_liquidity_params(
        description: &str,
        lines: &[String],
    ) -> Result<(String, String, String), ScriptParseError> {
        let mut pool_id = "1".to_string();
        let mut asset_a_amount = "100".to_string();
        let mut asset_b_amount = "100".to_string();

        // Extract from additional lines
        for line in lines {
            if line.contains("pool_id:") {
                pool_id = line.split(':').nth(1).unwrap_or("1").trim().to_string();
            } else if line.contains("asset_a_amount:") {
                asset_a_amount = line.split(':').nth(1).unwrap_or("100").trim().to_string();
            } else if line.contains("asset_b_amount:") {
                asset_b_amount = line.split(':').nth(1).unwrap_or("100").trim().to_string();
            }
        }

        Ok((pool_id, asset_a_amount, asset_b_amount))
    }

    /// Extract withdrawal parameters
    fn extract_withdrawal_params(
        description: &str,
        lines: &[String],
    ) -> Result<(String, String), ScriptParseError> {
        let mut pool_id = "1".to_string();
        let mut lp_amount = "50".to_string();

        // Extract from additional lines
        for line in lines {
            if line.contains("pool_id:") {
                pool_id = line.split(':').nth(1).unwrap_or("1").trim().to_string();
            } else if line.contains("lp_amount:") {
                lp_amount = line.split(':').nth(1).unwrap_or("50").trim().to_string();
            }
        }

        Ok((pool_id, lp_amount))
    }

    /// Extract pool creation parameters
    fn extract_pool_creation_params(
        description: &str,
        lines: &[String],
    ) -> Result<(String, String, String), ScriptParseError> {
        let mut asset_a = "ATOM".to_string();
        let mut asset_b = "USDC".to_string();
        let mut initial_price = "1.0".to_string();

        // Extract from additional lines
        for line in lines {
            if line.contains("asset_a:") {
                asset_a = line.split(':').nth(1).unwrap_or("ATOM").trim().to_string();
            } else if line.contains("asset_b:") {
                asset_b = line.split(':').nth(1).unwrap_or("USDC").trim().to_string();
            } else if line.contains("initial_price:") {
                initial_price = line.split(':').nth(1).unwrap_or("1.0").trim().to_string();
            }
        }

        Ok((asset_a, asset_b, initial_price))
    }

    /// Extract monitor parameters
    fn extract_monitor_params(
        description: &str,
        lines: &[String],
    ) -> Result<(String, u64), ScriptParseError> {
        let mut tx_hash = "".to_string();
        let mut timeout = 30u64;

        // Extract from additional lines
        for line in lines {
            if line.contains("tx_hash:") {
                tx_hash = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.contains("timeout:") {
                timeout = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("30")
                    .trim()
                    .parse()
                    .unwrap_or(30);
            }
        }

        Ok((tx_hash, timeout))
    }

    /// Validate parsed script
    fn validate_script(script: &TestScript) -> Result<(), ScriptParseError> {
        if script.name.is_empty() {
            return Err(ScriptParseError::MissingSection("name".to_string()));
        }

        if script.steps.is_empty() {
            return Err(ScriptParseError::MissingSection("steps".to_string()));
        }

        // Validate network
        if script.setup.network.is_empty() {
            return Err(ScriptParseError::InvalidFormat(
                "Network must be specified".to_string(),
            ));
        }

        Ok(())
    }
}

/// Internal enum to track parsing state
#[derive(Debug, Clone)]
enum ScriptSection {
    None,
    Title,
    Setup,
    Steps,
    ExpectedResults,
    Metadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_script() {
        let content = r#"
# Test Script: Basic Swap Test

## Setup
- Network: mantra-dukong
- Wallet: test wallet with sufficient balance

## Steps
1. **Check wallet balance** for ATOM and USDC
2. **Execute swap** of 10 ATOM for USDC with 1% slippage

## Expected Results
- Swap should complete successfully
- Balance should reflect the trade
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.name, "Basic Swap Test");
        assert_eq!(script.setup.network, "mantra-dukong");
        assert_eq!(script.steps.len(), 2);
        assert_eq!(script.expected_results.len(), 2);
    }
}
