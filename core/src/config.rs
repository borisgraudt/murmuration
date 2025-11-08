/// Configuration management
use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
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
        }
    }
}

impl Config {
    /// Create config from command line arguments
    pub fn from_args(args: &[String]) -> Result<Self> {
        if args.len() < 2 {
            return Err(MeshError::Config(format!(
                "Usage: {} <port> [peer1] [peer2] ...",
                args.first().unwrap_or(&"meshlink".to_string())
            )));
        }

        let port = args[1]
            .parse::<u16>()
            .map_err(|_| MeshError::Config("Port must be a valid number (0-65535)".to_string()))?;

        let listen_addr = format!("0.0.0.0:{}", port)
            .parse()
            .map_err(|_| MeshError::Config("Invalid listen address".to_string()))?;

        let known_peers = args[2..].to_vec();

        Ok(Self {
            listen_addr,
            known_peers,
            ..Default::default()
        })
    }
}
