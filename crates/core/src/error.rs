use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("serialization error: {0}")]
    Serial(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, Error>;