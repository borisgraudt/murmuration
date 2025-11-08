/// MeshLink - Decentralized P2P Network Protocol
/// 
/// A production-grade peer-to-peer networking library with protocol versioning,
/// connection management, and graceful shutdown.

pub mod error;
pub mod config;
pub mod node;
pub mod p2p;
pub mod utils;

pub use error::{MeshError, Result};
pub use config::Config;
pub use node::Node;

