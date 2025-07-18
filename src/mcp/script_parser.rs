use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::error;
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
        pool_id: String,
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
        // Check file size before reading to prevent DoS attacks
        const MAX_SCRIPT_FILE_SIZE: u64 = 1024 * 1024; // 1MB limit
        
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_SCRIPT_FILE_SIZE {
            return Err(ScriptParseError::InvalidFormat(format!(
                "Script file is too large: {} bytes (max: {} bytes)",
                metadata.len(),
                MAX_SCRIPT_FILE_SIZE
            )));
        }
        
        let content = fs::read_to_string(path)?;
        Self::parse_content(&content)
    }

    /// Parse script content from a string
    pub fn parse_content(content: &str) -> Result<TestScript, ScriptParseError> {
        let lines: Vec<&str> = content.lines().collect();
        
        // Compile regex for step number detection (matches one or more digits followed by a dot)
        let step_regex = Regex::new(r"^\d+\.")
            .map_err(|e| ScriptParseError::InvalidFormat(format!("Regex compilation failed: {}", e)))?;
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
                    if step_regex.is_match(trimmed) {
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
            let (from_asset, to_asset, amount, slippage, pool_id) =
                Self::extract_swap_params(description, lines)?;
            return Ok(StepAction::ExecuteSwap {
                from_asset,
                to_asset,
                amount,
                slippage,
                pool_id,
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
            // Note: This should ideally be specified in the script
            "DEFAULT_POOL_ID".to_string() // Placeholder - should be specified in script
        } else {
            "DEFAULT_POOL_ID".to_string()
        }
    }

    /// Extract swap parameters
    fn extract_swap_params(
        description: &str,
        lines: &[String],
    ) -> Result<(String, String, String, String, String), ScriptParseError> {
        let mut from_asset = "ATOM".to_string();
        let mut to_asset = "USDC".to_string();
        let mut amount = "10".to_string();
        let mut slippage = "1".to_string();
        let mut pool_id = None;

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
            } else if line.contains("pool_id:") {
                pool_id = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
            }
        }

        // pool_id is required - return error if not provided
        let pool_id = pool_id.ok_or_else(|| {
            ScriptParseError::InvalidParameter("pool_id is required for swap operations".to_string())
        })?;

        Ok((from_asset, to_asset, amount, slippage, pool_id))
    }

    /// Extract liquidity parameters
    fn extract_liquidity_params(
        _description: &str,
        lines: &[String],
    ) -> Result<(String, String, String), ScriptParseError> {
        let mut pool_id = None;
        let mut asset_a_amount = "100".to_string();
        let mut asset_b_amount = "100".to_string();

        // Extract from additional lines
        for line in lines {
            if line.contains("pool_id:") {
                pool_id = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
            } else if line.contains("asset_a_amount:") {
                asset_a_amount = line.split(':').nth(1).unwrap_or("100").trim().to_string();
            } else if line.contains("asset_b_amount:") {
                asset_b_amount = line.split(':').nth(1).unwrap_or("100").trim().to_string();
            }
        }

        // pool_id is required - return error if not provided
        let pool_id = pool_id.ok_or_else(|| {
            ScriptParseError::InvalidParameter("pool_id is required for liquidity operations".to_string())
        })?;

        Ok((pool_id, asset_a_amount, asset_b_amount))
    }

    /// Extract withdrawal parameters
    fn extract_withdrawal_params(
        _description: &str,
        lines: &[String],
    ) -> Result<(String, String), ScriptParseError> {
        let mut pool_id = None;
        let mut lp_amount = "50".to_string();

        // Extract from additional lines
        for line in lines {
            if line.contains("pool_id:") {
                pool_id = Some(line.split(':').nth(1).unwrap_or("").trim().to_string());
            } else if line.contains("lp_amount:") {
                lp_amount = line.split(':').nth(1).unwrap_or("50").trim().to_string();
            }
        }

        // pool_id is required - return error if not provided
        let pool_id = pool_id.ok_or_else(|| {
            ScriptParseError::InvalidParameter("pool_id is required for withdrawal operations".to_string())
        })?;

        Ok((pool_id, lp_amount))
    }

    /// Extract pool creation parameters
    fn extract_pool_creation_params(
        _description: &str,
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
        _description: &str,
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

    /// Basic validation for parsed script (called during parsing)
    fn validate_script(script: &TestScript) -> Result<(), ScriptParseError> {
        // Basic structural validation only
        if script.name.is_empty() {
            return Err(ScriptParseError::MissingSection("name".to_string()));
        }

        if script.steps.is_empty() {
            return Err(ScriptParseError::MissingSection("steps".to_string()));
        }

        // Script size validation to prevent DoS attacks (basic check only)
        const MAX_SCRIPT_SIZE: usize = 100; // Maximum number of steps
        const MAX_NAME_LENGTH: usize = 200;
        const MAX_DESCRIPTION_LENGTH: usize = 1000;
        
        if script.steps.len() > MAX_SCRIPT_SIZE {
            return Err(ScriptParseError::InvalidFormat(format!(
                "Script has too many steps: {} (max: {})",
                script.steps.len(),
                MAX_SCRIPT_SIZE
            )));
        }
        
        if script.name.len() > MAX_NAME_LENGTH {
            return Err(ScriptParseError::InvalidFormat(format!(
                "Script name is too long: {} characters (max: {})",
                script.name.len(),
                MAX_NAME_LENGTH
            )));
        }
        
        if let Some(desc) = &script.description {
            if desc.len() > MAX_DESCRIPTION_LENGTH {
                return Err(ScriptParseError::InvalidFormat(format!(
                    "Script description is too long: {} characters (max: {})",
                    desc.len(),
                    MAX_DESCRIPTION_LENGTH
                )));
            }
        }

        // Basic network validation - just check it's not empty
        if script.setup.network.is_empty() {
            return Err(ScriptParseError::InvalidFormat(
                "Network must be specified".to_string(),
            ));
        }

        Ok(())
    }

    /// Comprehensive validation for script execution (call before executing)
    pub fn validate_script_for_execution(script: &TestScript) -> Result<(), ScriptParseError> {
        // First run basic validation
        Self::validate_script(script)?;

        // Validate network name against allowed values
        let valid_networks = ["mantra-dukong", "mantra-hongbai", "mantra-mainnet", "testnet", "mainnet"];
        if !valid_networks.contains(&script.setup.network.as_str()) {
            return Err(ScriptParseError::InvalidFormat(format!(
                "Invalid network '{}'. Valid networks: {:?}",
                script.setup.network,
                valid_networks
            )));
        }

        // Validate setup parameters
        for (key, value) in &script.setup.parameters {
            if value.trim().is_empty() {
                return Err(ScriptParseError::InvalidFormat(format!(
                    "Setup parameter '{}' cannot be empty",
                    key
                )));
            }
            
            // Validate timeout parameters
            if key.contains("timeout") {
                if let Ok(timeout_val) = value.parse::<u64>() {
                    if timeout_val == 0 || timeout_val > 300 {
                        return Err(ScriptParseError::InvalidFormat(format!(
                            "Timeout parameter '{}' must be between 1 and 300 seconds, got: {}",
                            key,
                            timeout_val
                        )));
                    }
                } else {
                    return Err(ScriptParseError::InvalidFormat(format!(
                        "Timeout parameter '{}' must be a valid number",
                        key
                    )));
                }
            }
        }

        // Validate each step comprehensively
        for (index, step) in script.steps.iter().enumerate() {
            // Validate step number sequence
            if step.step_number != index + 1 {
                return Err(ScriptParseError::InvalidFormat(format!(
                    "Step number mismatch at position {}: expected {}, got {}",
                    index + 1,
                    index + 1,
                    step.step_number
                )));
            }
            
            // Validate step description
            if step.description.trim().is_empty() {
                return Err(ScriptParseError::InvalidFormat(format!(
                    "Step {} description cannot be empty",
                    step.step_number
                )));
            }
            
            // Validate step parameters
            for (param_key, param_value) in &step.parameters {
                if param_value.trim().is_empty() {
                    return Err(ScriptParseError::InvalidFormat(format!(
                        "Step {} parameter '{}' cannot be empty",
                        step.step_number,
                        param_key
                    )));
                }
                
                // Validate asset names (should match expected patterns)
                if param_key.contains("asset") && !param_value.starts_with("ibc/") && !param_value.starts_with("factory/") {
                    let valid_asset_pattern = regex::Regex::new(r"^[a-zA-Z][a-zA-Z0-9]*$").unwrap();
                    if !valid_asset_pattern.is_match(param_value) {
                        return Err(ScriptParseError::InvalidFormat(format!(
                            "Step {} asset parameter '{}' has invalid format: '{}'. Assets should start with a letter and contain only alphanumeric characters, or be IBC/factory tokens",
                            step.step_number,
                            param_key,
                            param_value
                        )));
                    }
                }
                
                // Validate amount parameters (should be numeric)
                if param_key.contains("amount") {
                    if param_value.parse::<f64>().is_err() {
                        return Err(ScriptParseError::InvalidFormat(format!(
                            "Step {} amount parameter '{}' must be a valid number: '{}'",
                            step.step_number,
                            param_key,
                            param_value
                        )));
                    }
                }
                
                // Validate slippage tolerance
                if param_key.contains("slippage") {
                    if let Ok(slippage_val) = param_value.parse::<f64>() {
                        if slippage_val < 0.0 || slippage_val > 100.0 {
                            return Err(ScriptParseError::InvalidFormat(format!(
                                "Step {} slippage parameter '{}' must be between 0 and 100: '{}'",
                                step.step_number,
                                param_key,
                                param_value
                            )));
                        }
                    } else {
                        return Err(ScriptParseError::InvalidFormat(format!(
                            "Step {} slippage parameter '{}' must be a valid number: '{}'",
                            step.step_number,
                            param_key,
                            param_value
                        )));
                    }
                }
                
                // Validate timeout parameters
                if param_key.contains("timeout") {
                    if let Ok(timeout_val) = param_value.parse::<u64>() {
                        if timeout_val == 0 || timeout_val > 300 {
                            return Err(ScriptParseError::InvalidFormat(format!(
                                "Step {} timeout parameter '{}' must be between 1 and 300 seconds: '{}'",
                                step.step_number,
                                param_key,
                                param_value
                            )));
                        }
                    } else {
                        return Err(ScriptParseError::InvalidFormat(format!(
                            "Step {} timeout parameter '{}' must be a valid number: '{}'",
                            step.step_number,
                            param_key,
                            param_value
                        )));
                    }
                }
                
                // Validate wallet addresses (should start with expected prefix)
                if param_key.contains("address") || param_key.contains("wallet") {
                    if !param_value.starts_with("mantra") && !param_value.is_empty() {
                        return Err(ScriptParseError::InvalidFormat(format!(
                            "Step {} address parameter '{}' should be a valid Mantra address (starts with 'mantra'): '{}'",
                            step.step_number,
                            param_key,
                            param_value
                        )));
                    }
                }
            }
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
   pool_id: ATOM/USDC

## Expected Results
- Swap should complete successfully
- Balance should reflect the trade
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.name, "Basic Swap Test");
        assert_eq!(script.setup.network, "mantra-dukong");
        assert_eq!(script.steps.len(), 2);
        assert_eq!(script.expected_results.len(), 2);
        
        // Check that the swap step has the required pool_id
        if let StepAction::ExecuteSwap { pool_id, .. } = &script.steps[1].action {
            assert_eq!(pool_id, "ATOM/USDC");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_missing_script_name_header() {
        let content = r#"
## Setup
- Network: mantra-dukong

## Steps
1. **Check wallet balance** for ATOM
"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::MissingSection(section)) = result {
            assert_eq!(section, "name");
        } else {
            panic!("Expected MissingSection error for name");
        }
    }

    #[test]
    fn test_missing_setup_section() {
        let content = r#"
# Test Script: Missing Setup Test

## Steps
1. **Check wallet balance** for ATOM
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        // Should use default setup values
        assert_eq!(script.setup.network, "mantra-dukong");
        assert_eq!(script.setup.wallet.wallet_type, "test");
    }

    #[test]
    fn test_missing_steps_section() {
        let content = r#"
# Test Script: Missing Steps Test

## Setup
- Network: mantra-dukong

## Expected Results
- Should have some steps
"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::MissingSection(section)) = result {
            assert_eq!(section, "steps");
        } else {
            panic!("Expected MissingSection error for steps");
        }
    }

    #[test]
    fn test_empty_script_name() {
        let content = r#"
# 

## Setup
- Network: mantra-dukong

## Steps
1. **Check wallet balance** for ATOM
"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::MissingSection(section)) = result {
            assert_eq!(section, "name");
        } else {
            panic!("Expected MissingSection error for empty name");
        }
    }

    #[test]
    fn test_malformed_markdown_wrong_heading_levels() {
        let content = r#"
# Test Script: Malformed Test

#### Setup
- Network: mantra-dukong

##### Steps
1. **Check wallet balance** for ATOM
"#;

        // Parser should still work but may not recognize sections correctly
        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.name, "Malformed Test");
        // The parser treats any heading starting with # as a potential section
        assert_eq!(script.steps.len(), 1); // Steps are recognized even with wrong heading level
    }

    #[test]
    fn test_malformed_markdown_missing_space_after_hash() {
        let content = r#"
#Test Script: No Space Test

##Setup
- Network: mantra-dukong

##Steps
1. **Check wallet balance** for ATOM
"#;

        // Parser may not recognize sections without proper spacing
        let script = ScriptParser::parse_content(content).unwrap();
        // Actually the parser does handle missing spaces, so steps will be recognized
        assert_eq!(script.steps.len(), 1); // Steps are recognized
    }

    #[test]
    fn test_malformed_markdown_inconsistent_formatting() {
        let content = r#"
# Test Script: Inconsistent Test

## Setup
- Network: mantra-dukong

# Steps  
1. **Check wallet balance** for ATOM
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.name, "Inconsistent Test");
        // Steps section uses # instead of ## but parser should handle it
        assert_eq!(script.steps.len(), 1);
    }

    #[test]
    fn test_invalid_step_format_no_dot() {
        let content = r#"
# Test Script: Invalid Step Format Test

## Setup
- Network: mantra-dukong

## Steps
1 **Check wallet balance** for ATOM
2. **Execute swap** of 10 ATOM for USDC
   pool_id: ATOM/USDC
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        // The parser recognizes both lines as steps because they both start with numbers
        assert_eq!(script.steps.len(), 2); // Both steps are parsed
    }

    #[test]
    fn test_empty_step_content() {
        let content = r#"
# Test Script: Empty Step Test

## Setup
- Network: mantra-dukong

## Steps
1.

2. **Check wallet balance** for ATOM
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        // Both steps are parsed - the first one is empty, the second has content
        assert_eq!(script.steps.len(), 2);
        assert_eq!(script.steps[0].description, "");
        assert_eq!(script.steps[1].description, "Check wallet balance"); // Bold formatting is removed
    }

    #[test]
    fn test_unknown_action_defaults_to_custom() {
        let content = r#"
# Test Script: Unknown Action Test

## Setup
- Network: mantra-dukong

## Steps
1. **Perform unknown action** that doesn't match any known pattern
   param1: value1
   param2: value2
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::Custom { tool_name, parameters } = &script.steps[0].action {
            assert_eq!(tool_name, "unknown");
            assert_eq!(parameters.get("param1"), Some(&"value1".to_string()));
            assert_eq!(parameters.get("param2"), Some(&"value2".to_string()));
        } else {
            panic!("Expected Custom action for unknown action");
        }
    }

    #[test]
    fn test_swap_missing_pool_id() {
        let content = r#"
# Test Script: Missing Pool ID Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 10 ATOM for USDC with 1% slippage
   amount: 10
   slippage: 1
"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::InvalidParameter(msg)) = result {
            assert!(msg.contains("pool_id is required"));
        } else {
            panic!("Expected InvalidParameter error for missing pool_id");
        }
    }

    #[test]
    fn test_provide_liquidity_missing_pool_id() {
        let content = r#"
# Test Script: Missing Pool ID Liquidity Test

## Setup
- Network: mantra-dukong

## Steps
1. **Provide liquidity** to the pool
   asset_a_amount: 100
   asset_b_amount: 200
"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::InvalidParameter(msg)) = result {
            assert!(msg.contains("pool_id is required"));
        } else {
            panic!("Expected InvalidParameter error for missing pool_id in liquidity");
        }
    }

    #[test]
    fn test_withdraw_liquidity_missing_pool_id() {
        let content = r#"
# Test Script: Missing Pool ID Withdrawal Test

## Setup
- Network: mantra-dukong

## Steps
1. **Withdraw liquidity** from the pool
   lp_amount: 50
"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::InvalidParameter(msg)) = result {
            assert!(msg.contains("pool_id is required"));
        } else {
            panic!("Expected InvalidParameter error for missing pool_id in withdrawal");
        }
    }

    #[test]
    fn test_empty_pool_id() {
        let content = r#"
# Test Script: Empty Pool ID Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 10 ATOM for USDC
   pool_id:
   amount: 10
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        // Empty pool_id is parsed as empty string, validation occurs later
        if let StepAction::ExecuteSwap { pool_id, .. } = &script.steps[0].action {
            assert_eq!(pool_id, "");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_zero_amount_in_swap() {
        let content = r#"
# Test Script: Zero Amount Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 0 ATOM for USDC
   pool_id: ATOM/USDC
   amount: 0
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::ExecuteSwap { amount, .. } = &script.steps[0].action {
            assert_eq!(amount, "0");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_negative_amount_in_liquidity() {
        let content = r#"
# Test Script: Negative Amount Test

## Setup
- Network: mantra-dukong

## Steps
1. **Provide liquidity** to the pool
   pool_id: ATOM/USDC
   asset_a_amount: -100
   asset_b_amount: 200
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::ProvideLiquidity { asset_a_amount, .. } = &script.steps[0].action {
            assert_eq!(asset_a_amount, "-100");
        } else {
            panic!("Expected ProvideLiquidity action");
        }
    }

    #[test]
    fn test_empty_string_amounts() {
        let content = r#"
# Test Script: Empty String Amounts Test

## Setup
- Network: mantra-dukong

## Steps
1. **Provide liquidity** to the pool
   pool_id: ATOM/USDC
   asset_a_amount: 
   asset_b_amount: 
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::ProvideLiquidity { asset_a_amount, asset_b_amount, .. } = &script.steps[0].action {
            assert_eq!(asset_a_amount, "");
            assert_eq!(asset_b_amount, "");
        } else {
            panic!("Expected ProvideLiquidity action");
        }
    }

    #[test]
    fn test_empty_network_in_setup() {
        let content = r#"
# Test Script: Empty Network Test

## Setup
- Network: 

## Steps
1. **Check wallet balance** for ATOM
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        // The parser maintains the default network value when empty is provided
        assert_eq!(script.setup.network, "mantra-dukong");
    }

    #[test]
    fn test_whitespace_only_pool_id() {
        let content = r#"
# Test Script: Whitespace Pool ID Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 10 ATOM for USDC
   pool_id:    
   amount: 10
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        // Whitespace-only pool_id is trimmed to empty string
        if let StepAction::ExecuteSwap { pool_id, .. } = &script.steps[0].action {
            assert_eq!(pool_id, "");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_zero_lp_amount_in_withdrawal() {
        let content = r#"
# Test Script: Zero LP Amount Test

## Setup
- Network: mantra-dukong

## Steps
1. **Withdraw liquidity** from the pool
   pool_id: ATOM/USDC
   lp_amount: 0
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::WithdrawLiquidity { lp_amount, .. } = &script.steps[0].action {
            assert_eq!(lp_amount, "0");
        } else {
            panic!("Expected WithdrawLiquidity action");
        }
    }

    #[test]
    fn test_negative_slippage() {
        let content = r#"
# Test Script: Negative Slippage Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 10 ATOM for USDC
   pool_id: ATOM/USDC
   amount: 10
   slippage: -5
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::ExecuteSwap { slippage, .. } = &script.steps[0].action {
            assert_eq!(slippage, "-5");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_very_high_slippage() {
        let content = r#"
# Test Script: Very High Slippage Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 10 ATOM for USDC
   pool_id: ATOM/USDC
   amount: 10
   slippage: 150
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::ExecuteSwap { slippage, .. } = &script.steps[0].action {
            assert_eq!(slippage, "150");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_non_numeric_slippage() {
        let content = r#"
# Test Script: Non-Numeric Slippage Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 10 ATOM for USDC
   pool_id: ATOM/USDC
   amount: 10
   slippage: abc
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::ExecuteSwap { slippage, .. } = &script.steps[0].action {
            assert_eq!(slippage, "abc");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_empty_slippage() {
        let content = r#"
# Test Script: Empty Slippage Test

## Setup
- Network: mantra-dukong

## Steps
1. **Execute swap** of 10 ATOM for USDC
   pool_id: ATOM/USDC
   amount: 10
   slippage: 
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.steps.len(), 1);
        
        if let StepAction::ExecuteSwap { slippage, .. } = &script.steps[0].action {
            assert_eq!(slippage, "");
        } else {
            panic!("Expected ExecuteSwap action");
        }
    }

    #[test]
    fn test_empty_expected_results_section() {
        let content = r#"
# Test Script: Empty Expected Results Test

## Setup
- Network: mantra-dukong

## Steps
1. **Check wallet balance** for ATOM

## Expected Results

## Metadata
author: test
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.expected_results.len(), 0);
        assert_eq!(script.metadata.get("author"), Some(&"test".to_string()));
    }

    #[test]
    fn test_malformed_expected_results() {
        let content = r#"
# Test Script: Malformed Expected Results Test

## Setup
- Network: mantra-dukong

## Steps
1. **Check wallet balance** for ATOM

## Expected Results
This is not a proper list item
- This is a proper list item
Another improper line
- Another proper list item
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        // Should parse both proper and improper lines
        assert_eq!(script.expected_results.len(), 4);
        assert_eq!(script.expected_results[0], "This is not a proper list item");
        assert_eq!(script.expected_results[1], "This is a proper list item");
        assert_eq!(script.expected_results[2], "Another improper line");
        assert_eq!(script.expected_results[3], "Another proper list item");
    }

    #[test]
    fn test_missing_metadata_values() {
        let content = r#"
# Test Script: Missing Metadata Values Test

## Setup
- Network: mantra-dukong

## Steps
1. **Check wallet balance** for ATOM

## Metadata
author:
version: 1.0
description:
tags: test, validation
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.metadata.get("author"), Some(&"".to_string()));
        assert_eq!(script.metadata.get("version"), Some(&"1.0".to_string()));
        assert_eq!(script.metadata.get("description"), Some(&"".to_string()));
        assert_eq!(script.metadata.get("tags"), Some(&"test, validation".to_string()));
    }

    #[test]
    fn test_malformed_metadata() {
        let content = r#"
# Test Script: Malformed Metadata Test

## Setup
- Network: mantra-dukong

## Steps
1. **Check wallet balance** for ATOM

## Metadata
author: test author
no_colon_line_should_be_ignored
version: 1.0
: empty_key
key_with_multiple: colons: should: work
"#;

        let script = ScriptParser::parse_content(content).unwrap();
        assert_eq!(script.metadata.get("author"), Some(&"test author".to_string()));
        assert_eq!(script.metadata.get("version"), Some(&"1.0".to_string()));
        assert_eq!(script.metadata.get("key_with_multiple"), Some(&"colons: should: work".to_string()));
        // Lines without colons should be ignored
        assert!(!script.metadata.contains_key("no_colon_line_should_be_ignored"));
    }

    #[test]
    fn test_completely_invalid_script_content() {
        let content = "This is not markdown at all, just plain text with no structure";

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::MissingSection(section)) = result {
            assert_eq!(section, "name");
        } else {
            panic!("Expected MissingSection error for invalid content");
        }
    }

    #[test]
    fn test_regex_compilation_failure() {
        // This test verifies error handling, but since we use a simple regex,
        // we'll test empty content which should trigger validation errors
        let content = "";

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::MissingSection(section)) = result {
            assert_eq!(section, "name");
        } else {
            panic!("Expected MissingSection error for empty content");
        }
    }

    #[test]
    fn test_invalid_step_empty_lines() {
        let content = r#"
# Test Script: Invalid Step Test

## Setup
- Network: mantra-dukong

## Steps

"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::MissingSection(section)) = result {
            assert_eq!(section, "steps");
        } else {
            panic!("Expected MissingSection error for no steps");
        }
    }

    #[test]
    fn test_script_with_only_invalid_sections() {
        let content = r#"
### This is not a valid main heading

#### This is also not valid

##### Still not valid

Some random content that doesn't follow any structure
"#;

        let result = ScriptParser::parse_content(content);
        assert!(result.is_err());
        if let Err(ScriptParseError::MissingSection(section)) = result {
            assert_eq!(section, "name");
        } else {
            panic!("Expected MissingSection error for invalid structure");
        }
    }
}
