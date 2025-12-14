pub mod ai;
pub mod api;
pub mod config;
pub mod cli_app;
/// MeshNet - Decentralized P2P Network Protocol
///
/// A production-grade peer-to-peer networking library with protocol versioning,
/// connection management, AI routing, and graceful shutdown.
pub mod error;
pub mod node;
pub mod p2p;
pub mod utils;

pub use config::Config;
pub use error::{MeshError, Result};
pub use node::Node;
