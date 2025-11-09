/// AI routing and statistics collection
pub mod router;
pub mod routing_logger;
pub mod stats_collector;

pub use router::{ElysiumAddress, MeshMessage, Router};
pub use routing_logger::{RoutingLogger, RoutingLogEntry, PeerSelection, PeerMetricsSnapshot, MessageContext};
pub use stats_collector::StatsCollector;
