/// MeshLink P2P Node - Main entry point
use core::{Config, Node};
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .init();
    
    // Parse configuration
    let args: Vec<String> = env::args().collect();
    let config = Config::from_args(&args)
        .map_err(|e| anyhow::anyhow!("Configuration error: {}", e))?;
    
    // Create and start node
    let node = Node::new(config);
    info!("🚀 Starting MeshLink P2P Node");
    info!("   Node ID: {}", node.id);
    info!("   Protocol Version: {}", core::p2p::PROTOCOL_VERSION);
    
    // Start the node (this will block until shutdown)
    node.start().await
        .map_err(|e| anyhow::anyhow!("Node error: {}", e))?;
    
    Ok(())
}
