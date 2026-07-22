pub mod ai;
pub mod api;
pub mod bundle;
pub mod cli_app;
pub mod config;
pub mod contact_store;
pub mod content_store;
pub mod e2e;
pub mod murmuration;
/// MeshNet - Decentralized P2P Network Protocol
///
/// A production-grade peer-to-peer networking library with protocol versioning,
/// connection management, AI routing, and graceful shutdown.
pub mod error;
pub mod group;
pub mod identity;
pub mod message_store;
pub mod messenger_api;
pub mod messenger_types;
pub mod naming;
pub mod nat;
pub mod node;
pub mod onion;
pub mod p2p;
mod peer_store;
pub use murmuration_routing::trace;
pub mod transport;
pub mod url_handler;
pub mod utils;
pub mod web_gateway;

pub use config::Config;
pub use error::{MeshError, Result};
pub use node::Node;
