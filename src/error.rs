use cosmrs::rpc::endpoint::broadcast::tx_sync::Response as SyncTxResponse;
use thiserror::Error;

/// SDK Error type
#[derive(Error, Debug)]
pub enum Error {
    /// Error when interacting with CosmRS
    #[error("CosmRS error: {0}")]
    CosmRs(#[from] cosmrs::Error),

    /// RPC client error
    #[error("RPC error: {0}")]
    Rpc(String),

    /// Transaction broadcast error with response
    #[error("Transaction broadcast error: {0:?}")]
    TxBroadcast(SyncTxResponse),

    /// Transaction simulation error
    #[error("Transaction simulation error: {0}")]
    TxSimulation(String),

    /// Wallet error
    #[error("Wallet error: {0}")]
    Wallet(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Contract interaction error
    #[error("Contract error: {0}")]
    Contract(String),

    /// Serialization/Deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Other errors
    #[error("{0}")]
    Other(String),

    /// Transaction error
    #[error("Transaction error: {0}")]
    Tx(String),
}
