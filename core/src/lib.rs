pub mod ai;
pub mod api;
pub mod config;
/// MeshNet - Decentralized P2P Network Protocol
///
/// A production-grade peer-to-peer networking library with protocol versioning,
/// connection management, AI routing, and graceful shutdown.
#[cfg(not(doctest))]
pub mod error;
#[cfg(doctest)]
mod error;
pub mod node;
pub mod p2p;
pub mod utils;

pub use config::Config;
pub use error::{MeshError, Result};
pub use node::Node;
