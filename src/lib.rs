pub mod client;
pub mod config;
pub mod error;
pub mod wallet;
// Re-export mantra-dex-std for user convenience
pub use mantra_dex_std;

pub use client::MantraDexClient;
pub use config::{MantraNetworkConfig, NetworkConstants};
pub use error::Error;
pub use wallet::MantraWallet;

// Re-export common types from mantra-dex-std
pub use cosmwasm_std::{Coin, Decimal, Uint128};
pub use mantra_dex_std::{
    fee::PoolFee,
    pool_manager::{PoolInfo, PoolType, SwapOperation},
};
