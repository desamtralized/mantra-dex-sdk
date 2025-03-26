use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("SDK Error: {0}")]
    Sdk(#[from] mantra_dex_sdk::Error),

    #[error("IO Error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Toml Error: {0}")]
    Toml(#[from] toml::ser::Error),

    #[error("Toml Deserialization Error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("Wallet Error: {0}")]
    Wallet(String),

    #[error("Command Error: {0}")]
    Command(String),

    #[error("Parse Error: {0}")]
    Parse(String),

    #[error("Password decryption failed")]
    DecryptionFailed,
}
