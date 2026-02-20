pub mod ai;
pub mod api;
pub mod bundle;
pub mod cli_app;
pub mod config;
pub mod content_store;
pub mod elysium;
/// MeshNet - Decentralized P2P Network Protocol
///
/// A production-grade peer-to-peer networking library with protocol versioning,
/// connection management, AI routing, and graceful shutdown.
pub mod error;
pub mod identity;
pub mod message_store;
pub mod naming;
pub mod node;
pub mod p2p;
mod peer_store;
pub mod url_handler;
pub mod utils;
pub mod messenger_api;
pub mod messenger_types;
pub mod contact_store;
pub mod web_gateway;

pub use config::Config;
pub use error::{MeshError, Result};
pub use node::Node;
