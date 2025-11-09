/// AI Routing Logger - Logs routing decisions for AI training
use crate::p2p::peer::PeerInfo;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Single routing decision log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingLogEntry {
    /// Timestamp of the routing decision
    pub timestamp: String,

    /// Message ID
    pub message_id: String,

    /// Node ID that made the routing decision
    pub node_id: String,

    /// Peer that sent the message (if forwarding)
    pub from_peer: Option<String>,

    /// Selected peers for forwarding (with scores and metrics)
    pub selected_peers: Vec<PeerSelection>,

    /// All available peers at the time of decision (with metrics)
    pub available_peers: Vec<PeerMetricsSnapshot>,

    /// Message context
    pub message_context: MessageContext,
}

/// Peer selection with score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSelection {
    pub peer_id: String,
    pub score: f64,
    pub metrics: PeerMetricsSnapshot,
}

/// Snapshot of peer metrics at decision time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerMetricsSnapshot {
    pub peer_id: String,
    pub latency_ms: Option<f64>,
    pub uptime_secs: f64,
    pub ping_count: u32,
    pub ping_failures: u32,
    pub reliability_score: f64,
    pub is_connected: bool,
}

/// Message context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContext {
    pub ttl: u8,
    pub path_length: usize,
    pub is_broadcast: bool,
    pub target_peer: Option<String>,
}

impl From<&PeerInfo> for PeerMetricsSnapshot {
    fn from(peer: &PeerInfo) -> Self {
        Self {
            peer_id: peer.node_id.clone(),
            latency_ms: peer.metrics.latency.map(|d| d.as_secs_f64() * 1000.0),
            uptime_secs: peer.metrics.uptime.as_secs_f64(),
            ping_count: peer.metrics.ping_count,
            ping_failures: peer.metrics.ping_failures,
            reliability_score: peer.metrics.reliability_score() as f64,
            is_connected: peer.is_connected(),
        }
    }
}

/// Routing logger
pub struct RoutingLogger {
    log_file: Arc<RwLock<Option<PathBuf>>>,
}

impl RoutingLogger {
    pub fn new() -> Self {
        Self {
            log_file: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize logger with log file path
    pub async fn init(&self, log_dir: Option<PathBuf>) {
        let log_path = if let Some(dir) = log_dir {
            dir.join("ai_routing_logs.jsonl")
        } else {
            // Default: logs/ directory in current working directory
            PathBuf::from("logs").join("ai_routing_logs.jsonl")
        };

        // Create directory if it doesn't exist
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut file = self.log_file.write().await;
        *file = Some(log_path);
    }

    /// Log a routing decision
    pub async fn log_routing_decision(&self, entry: RoutingLogEntry) {
        let file_path = {
            let file = self.log_file.read().await;
            file.clone()
        };

        if let Some(path) = file_path {
            // Append to JSONL file (one JSON object per line)
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
                if let Ok(json) = serde_json::to_string(&entry) {
                    let _ = writeln!(file, "{}", json);
                    let _ = file.flush();
                }
            }
        }
    }
}

impl Default for RoutingLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RoutingLogger {
    fn clone(&self) -> Self {
        Self {
            log_file: self.log_file.clone(),
        }
    }
}
