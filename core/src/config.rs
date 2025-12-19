/// Configuration management
use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_DISCOVERY_PORT: u16 = 9998;

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

    /// API server address for local clients (defaults to 127.0.0.1:(9000 + listen_port))
    pub api_addr: Option<SocketAddr>,

    /// UDP discovery port (should be same across nodes on the same LAN)
    pub discovery_port: u16,

    /// Enable UDP discovery
    pub enable_discovery: bool,
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
            api_addr: None,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            enable_discovery: true,
        }
    }
}

impl Config {
    /// Create config from command line arguments
    pub fn from_args(args: &[String]) -> Result<Self> {
        if args.len() < 2 {
            return Err(MeshError::Config(format!(
                "Usage: {} <port> [peer1] [peer2] ... [--ai-debug] [--data-dir <path>] [--api-port <port>] [--discovery-port <port>] [--no-discovery]",
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
        let mut api_port: Option<u16> = None;
        let mut discovery_port: Option<u16> = None;
        let mut enable_discovery = true;
        
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
                "--api-port" => {
                    let p = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--api-port requires a port argument".to_string())
                    })?;
                    api_port = Some(p.parse::<u16>().map_err(|_| {
                        MeshError::Config("--api-port must be a valid number (0-65535)".to_string())
                    })?);
                    i += 2;
                }
                "--discovery-port" => {
                    let p = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--discovery-port requires a port argument".to_string())
                    })?;
                    discovery_port = Some(p.parse::<u16>().map_err(|_| {
                        MeshError::Config(
                            "--discovery-port must be a valid number (0-65535)".to_string(),
                        )
                    })?);
                    i += 2;
                }
                "--no-discovery" => {
                    enable_discovery = false;
                    i += 1;
                }
                other => {
                    known_peers.push(other.to_string());
                    i += 1;
                }
            }
        }

        // Env overrides (nice for scripts)
        if let Some(p) = std::env::var("MESHLINK_API_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
        {
            api_port = Some(p);
        }
        if let Some(p) = std::env::var("MESHLINK_DISCOVERY_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
        {
            discovery_port = Some(p);
        }
        if std::env::var("MESHLINK_NO_DISCOVERY").is_ok() {
            enable_discovery = false;
        }

        let api_addr = api_port.map(|p| format!("127.0.0.1:{}", p).parse()).transpose().map_err(
            |_| MeshError::Config("Invalid api address".to_string()),
        )?;

        Ok(Self {
            listen_addr,
            known_peers,
            ai_debug,
            data_dir,
            api_addr,
            discovery_port: discovery_port.unwrap_or(DEFAULT_DISCOVERY_PORT),
            enable_discovery,
            ..Default::default()
        })
    }
}
