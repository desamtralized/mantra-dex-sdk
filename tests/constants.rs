/// Standard test amounts for consistency across tests
pub const SMALL_AMOUNT: u128 = 1_000_000; // 1 token (6 decimals)
pub const MEDIUM_AMOUNT: u128 = 10_000_000; // 10 tokens (6 decimals)
pub const LARGE_AMOUNT: u128 = 100_000_000; // 100 tokens (6 decimals)

/// Common fee percentages used in tests
pub const PROTOCOL_FEE_PERCENT: u64 = 1; // 1% protocol fee
pub const SWAP_FEE_PERCENT: u64 = 2; // 2% swap fee
pub const BURN_FEE_PERCENT: u64 = 0; // 0% burn fee

/// Test timeout durations
pub const TEST_TIMEOUT_SECONDS: u64 = 30;
pub const LONG_TEST_TIMEOUT_SECONDS: u64 = 60;

/// Standard slippage values for tests
pub const DEFAULT_SLIPPAGE_PERCENT: u64 = 5; // 5% slippage
pub const LOW_SLIPPAGE_PERCENT: u64 = 1; // 1% slippage
pub const HIGH_SLIPPAGE_PERCENT: u64 = 10; // 10% slippage

/// Pool identifier constants
pub const TEST_POOL_SUFFIX: &str = ".pool";
pub const POOL_PREFIX: &str = "o.";
