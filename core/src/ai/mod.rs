/// AI routing and statistics collection
pub mod router;
pub mod stats_collector;

pub use router::{ElysiumAddress, MeshMessage, Router};
pub use stats_collector::StatsCollector;
