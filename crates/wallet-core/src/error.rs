use thiserror::Error;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("Failed to generate mnemonic: {0}")]
    MnemonicGeneration(String),

    #[error("Invalid mnemonic phrase: {0}")]
    InvalidMnemonic(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("Encryption failed: {0}")]
    Encryption(String),

    #[error("Decryption failed: {0}")]
    Decryption(String),

    #[error("Internal vault error: {0}")]
    VaultError(String),

    #[error("Invalid public key format: {0}")]
    InvalidPubkey(String),

    #[error("Blockchain network error: {0}")]
    NetworkError(String),

    #[error("Transaction failed: {0}")]
    TransactionError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, WalletError>;
