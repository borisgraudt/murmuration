//! Minimal error type for the routing crate.
//!
//! The parent node has a large `MeshError`; the routing algorithms need only a
//! sliver of it, so this crate carries its own small error to stay decoupled.

use std::fmt;

/// Errors surfaced by the routing layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError {
    /// A malformed address or protocol-level value.
    Protocol(String),
}

impl fmt::Display for RoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoutingError::Protocol(m) => write!(f, "protocol error: {m}"),
        }
    }
}

impl std::error::Error for RoutingError {}

pub type Result<T> = std::result::Result<T, RoutingError>;
