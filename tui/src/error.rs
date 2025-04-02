use thiserror::Error;

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("SDK error: {0}")]
    Sdk(String),

    #[error("Command error: {0}")]
    Command(String),

    #[error("Wallet error: {0}")]
    Wallet(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<anyhow::Error> for TuiError {
    fn from(err: anyhow::Error) -> Self {
        TuiError::Other(err.to_string())
    }
}

impl From<mantra_dex_sdk::Error> for TuiError {
    fn from(err: mantra_dex_sdk::Error) -> Self {
        TuiError::Sdk(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, TuiError>; 