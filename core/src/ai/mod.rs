//! AI routing and statistics collection.
//!
//! The routing algorithms (UCB1 bandit + Q-routing), peer types, contact traces,
//! and stats now live in the standalone [`murmuration_routing`] crate. This module
//! re-exports them so existing `crate::ai::router::…` paths keep working, and adds
//! the node-only `adapter` bridging `MeshMessage` and the wire `Message`.

pub mod adapter;

pub use murmuration_routing::router;
pub use murmuration_routing::routing_logger;
pub use murmuration_routing::stats as stats_collector;

pub use murmuration_routing::router::{MeshMessage, MurmurationAddress, Router, RouterStore};
pub use murmuration_routing::routing_logger::{
    MessageContext, PeerMetricsSnapshot, PeerSelection, RoutingLogEntry, RoutingLogger,
};
pub use murmuration_routing::stats::StatsCollector;
