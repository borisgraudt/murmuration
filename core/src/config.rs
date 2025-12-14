/// Configuration management
use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

/// Node configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Listening address
    pub listen_addr: SocketAddr,

    /// Known peer addresses (bootstrap peers)
    pub known_peers: Vec<String>,

    /// Connection timeout
    pub connection_timeout: Duration,

    /// Keepalive interval
    pub keepalive_interval: Duration,

    /// Heartbeat interval
    pub heartbeat_interval: Duration,

    /// Peer stale timeout
    pub peer_stale_timeout: Duration,

    /// Max connection attempts per peer
    pub max_connection_attempts: u32,

    /// Retry connection interval
    pub retry_interval: Duration,

    /// Enable AI-routing debug output
    pub ai_debug: bool,

    /// Optional data directory for persistent identity/keys (defaults to `.ely/node-<port>`)
    pub data_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8080".parse().unwrap(),
            known_peers: Vec::new(),
            connection_timeout: Duration::from_secs(10),
            keepalive_interval: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(5),
            peer_stale_timeout: Duration::from_secs(120),
            max_connection_attempts: 5,
            retry_interval: Duration::from_secs(5),
            ai_debug: false,
            data_dir: None,
        }
    }
}

impl Config {
    /// Create config from command line arguments
    pub fn from_args(args: &[String]) -> Result<Self> {
        if args.len() < 2 {
            return Err(MeshError::Config(format!(
                "Usage: {} <port> [peer1] [peer2] ... [--ai-debug] [--data-dir <path>]",
                args.first().unwrap_or(&"meshlink".to_string())
            )));
        }

        let port = args[1]
            .parse::<u16>()
            .map_err(|_| MeshError::Config("Port must be a valid number (0-65535)".to_string()))?;

        let listen_addr = format!("0.0.0.0:{}", port)
            .parse()
            .map_err(|_| MeshError::Config("Invalid listen address".to_string()))?;

        // Parse known peers and flags
        let mut known_peers = Vec::new();
        let mut ai_debug = false;
        let mut data_dir: Option<PathBuf> = None;
        
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--ai-debug" => {
                    ai_debug = true;
                    i += 1;
                }
                "--data-dir" => {
                    let path = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--data-dir requires a path argument".to_string())
                    })?;
                    data_dir = Some(PathBuf::from(path));
                    i += 2;
                }
                other => {
                    known_peers.push(other.to_string());
                    i += 1;
                }
            }
        }

        Ok(Self {
            listen_addr,
            known_peers,
            ai_debug,
            data_dir,
            ..Default::default()
        })
    }
}
