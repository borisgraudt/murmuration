/// AI routing and statistics collection
pub mod router;
pub mod routing_logger;
pub mod stats_collector;

pub use router::{MurmurationAddress, MeshMessage, Router};
pub use routing_logger::{
    MessageContext, PeerMetricsSnapshot, PeerSelection, RoutingLogEntry, RoutingLogger,
};
pub use stats_collector::StatsCollector;
