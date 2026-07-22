/// Configuration management
use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_DISCOVERY_PORT: u16 = 9998;
const DEFAULT_CONNECT_COOLDOWN_MS: u64 = 8_000;
const DEFAULT_MAX_CONNECTIONS: usize = 24;
const DEFAULT_MAX_CONNECT_IN_FLIGHT: usize = 16;
const DEFAULT_CONNECT_BACKOFF_MAX_MS: u64 = 120_000;

// ---------------------------------------------------------------------------
// Censorship-resistance sub-configs (Upgrades 1–3)
// ---------------------------------------------------------------------------

/// Traffic obfuscation configuration (Upgrade 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Obfuscation mode: `"none"` | `"tls"` | `"obfs4"`.
    /// Default: `"tls"` — wrap all connections in TLS 1.3 + WebSocket upgrade.
    pub obfs_mode: String,
    /// SNI hostname presented in the TLS ClientHello.
    /// Should resemble a common CDN host to blend in with ambient HTTPS traffic.
    pub tls_sni: String,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            obfs_mode: "tls".to_string(),
            tls_sni: "cdn.cloudflare.com".to_string(),
        }
    }
}

/// NAT traversal configuration (Upgrade 2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatConfig {
    /// STUN servers used for external address discovery.
    pub stun_servers: Vec<String>,
    /// Whether this node is willing to act as a TURN relay for symmetric-NAT peers.
    pub enable_relay: bool,
    /// Known relay node addresses (auto-discovered from the DHT when empty).
    pub relay_candidates: Vec<String>,
}

impl Default for NatConfig {
    fn default() -> Self {
        Self {
            stun_servers: vec![
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
            ],
            enable_relay: true,
            relay_candidates: Vec::new(),
        }
    }
}

/// Onion routing configuration (Upgrade 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnionConfig {
    /// Enable onion routing for outbound messages. Default: `false` (opt-in).
    pub enabled: bool,
    /// Number of onion hops (minimum 3 for meaningful anonymity).
    pub hops: usize,
    /// How long (seconds) a circuit lives before being rebuilt.
    pub circuit_lifetime_s: u64,
    /// Automatically rebuild circuits that fail mid-transfer.
    pub rebuild_on_failure: bool,
}

impl Default for OnionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            hops: 3,
            circuit_lifetime_s: 600,
            rebuild_on_failure: true,
        }
    }
}

// ---------------------------------------------------------------------------

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

    /// Optional data directory for persistent identity/keys (defaults to `.mur/node-<port>`)
    pub data_dir: Option<PathBuf>,

    /// API server address for local clients (defaults to 127.0.0.1:(9000 + listen_port))
    pub api_addr: Option<SocketAddr>,

    /// Web Gateway port (defaults to API port + 1)
    pub gateway_port: Option<u16>,

    /// UDP discovery port (should be same across nodes on the same LAN)
    pub discovery_port: u16,

    /// Enable UDP discovery
    pub enable_discovery: bool,

    /// Maximum number of total connected peers (hard cap)
    pub max_connections: usize,

    /// Minimum delay between connection attempts to the same address
    pub connect_cooldown: Duration,

    /// Limit concurrent outbound connect attempts (prevents connection storms)
    pub max_connect_in_flight: usize,

    /// Max backoff for repeated connect attempts (cap)
    pub connect_backoff_max: Duration,

    /// Traffic obfuscation (TLS camouflage / obfs4).
    #[serde(default)]
    pub transport: TransportConfig,

    /// NAT traversal (STUN + hole punching + relay fallback).
    #[serde(default)]
    pub nat: NatConfig,

    /// Onion routing (opt-in anonymity layer).
    #[serde(default)]
    pub onion: OnionConfig,
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
            gateway_port: None,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            enable_discovery: true,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            connect_cooldown: Duration::from_millis(DEFAULT_CONNECT_COOLDOWN_MS),
            max_connect_in_flight: DEFAULT_MAX_CONNECT_IN_FLIGHT,
            connect_backoff_max: Duration::from_millis(DEFAULT_CONNECT_BACKOFF_MAX_MS),
            transport: TransportConfig::default(),
            nat: NatConfig::default(),
            onion: OnionConfig::default(),
        }
    }
}

impl Config {
    /// Create config from command line arguments
    pub fn from_args(args: &[String]) -> Result<Self> {
        if args.len() < 2 {
            return Err(MeshError::Config(format!(
                "Usage: {} <port> [peer1] [peer2] ... [--ai-debug] [--data-dir <path>] [--api-port <port>] [--gateway <port>] [--discovery-port <port>] [--no-discovery] [--max-connections <n>] [--connect-cooldown-ms <ms>] [--max-connect-in-flight <n>] [--connect-backoff-max-ms <ms>]",
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
        let mut gateway_port: Option<u16> = None;
        let mut discovery_port: Option<u16> = None;
        let mut enable_discovery = true;
        let mut max_connections: Option<usize> = None;
        let mut connect_cooldown_ms: Option<u64> = None;
        let mut max_connect_in_flight: Option<usize> = None;
        let mut connect_backoff_max_ms: Option<u64> = None;

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
                "--gateway" => {
                    let p = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--gateway requires a port argument".to_string())
                    })?;
                    gateway_port = Some(p.parse::<u16>().map_err(|_| {
                        MeshError::Config("--gateway must be a valid number (0-65535)".to_string())
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
                "--max-connections" => {
                    let n = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--max-connections requires a number".to_string())
                    })?;
                    max_connections = Some(n.parse::<usize>().map_err(|_| {
                        MeshError::Config("--max-connections must be a valid number".to_string())
                    })?);
                    i += 2;
                }
                "--connect-cooldown-ms" => {
                    let n = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--connect-cooldown-ms requires a number".to_string())
                    })?;
                    connect_cooldown_ms = Some(n.parse::<u64>().map_err(|_| {
                        MeshError::Config(
                            "--connect-cooldown-ms must be a valid number".to_string(),
                        )
                    })?);
                    i += 2;
                }
                "--max-connect-in-flight" => {
                    let n = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--max-connect-in-flight requires a number".to_string())
                    })?;
                    max_connect_in_flight = Some(n.parse::<usize>().map_err(|_| {
                        MeshError::Config(
                            "--max-connect-in-flight must be a valid number".to_string(),
                        )
                    })?);
                    i += 2;
                }
                "--connect-backoff-max-ms" => {
                    let n = args.get(i + 1).ok_or_else(|| {
                        MeshError::Config("--connect-backoff-max-ms requires a number".to_string())
                    })?;
                    connect_backoff_max_ms = Some(n.parse::<u64>().map_err(|_| {
                        MeshError::Config(
                            "--connect-backoff-max-ms must be a valid number".to_string(),
                        )
                    })?);
                    i += 2;
                }
                other => {
                    known_peers.push(other.to_string());
                    i += 1;
                }
            }
        }

        // Env overrides (nice for scripts)
        if let Some(p) = std::env::var("MURMURATION_API_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
        {
            api_port = Some(p);
        }
        if let Some(p) = std::env::var("MURMURATION_DISCOVERY_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
        {
            discovery_port = Some(p);
        }
        if std::env::var("MURMURATION_NO_DISCOVERY").is_ok() {
            enable_discovery = false;
        }
        if let Some(n) = std::env::var("MURMURATION_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        {
            max_connections = Some(n);
        }
        if let Some(n) = std::env::var("MURMURATION_CONNECT_COOLDOWN_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            connect_cooldown_ms = Some(n);
        }
        if let Some(n) = std::env::var("MURMURATION_MAX_CONNECT_IN_FLIGHT")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
        {
            max_connect_in_flight = Some(n);
        }
        if let Some(n) = std::env::var("MURMURATION_CONNECT_BACKOFF_MAX_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            connect_backoff_max_ms = Some(n);
        }

        let api_addr = api_port
            .map(|p| format!("127.0.0.1:{}", p).parse())
            .transpose()
            .map_err(|_| MeshError::Config("Invalid api address".to_string()))?;

        Ok(Self {
            listen_addr,
            known_peers,
            ai_debug,
            data_dir,
            api_addr,
            gateway_port,
            discovery_port: discovery_port.unwrap_or(DEFAULT_DISCOVERY_PORT),
            enable_discovery,
            max_connections: max_connections.unwrap_or(DEFAULT_MAX_CONNECTIONS),
            connect_cooldown: Duration::from_millis(
                connect_cooldown_ms.unwrap_or(DEFAULT_CONNECT_COOLDOWN_MS),
            ),
            max_connect_in_flight: max_connect_in_flight.unwrap_or(DEFAULT_MAX_CONNECT_IN_FLIGHT),
            connect_backoff_max: Duration::from_millis(
                connect_backoff_max_ms.unwrap_or(DEFAULT_CONNECT_BACKOFF_MAX_MS),
            ),
            transport: TransportConfig::default(),
            nat: NatConfig::default(),
            onion: OnionConfig::default(),
            ..Default::default()
        })
    }
}
