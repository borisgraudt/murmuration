/// AI routing and statistics collection
pub mod router;
pub mod stats_collector;

pub use router::{Router, MeshMessage, ElysiumAddress};
pub use stats_collector::StatsCollector;

