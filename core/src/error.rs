/// Error types for the P2P network
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MeshError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Peer error: {0}")]
    Peer(String),

    #[error("Timeout error: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, MeshError>;

