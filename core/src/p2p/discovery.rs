/// Peer discovery via UDP broadcast/multicast
use crate::error::Result;
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tracing::{debug, info, warn};

const DISCOVERY_MULTICAST: Ipv4Addr = Ipv4Addr::new(239, 255, 0, 1);

/// Discovery message sent via UDP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub node_id: String,
    pub listen_port: u16,
    pub public_key: String, // Base64-encoded RSA public key
    pub timestamp: i64,
}

/// Peer discovery manager
pub struct DiscoveryManager {
    node_id: String,
    listen_port: u16,
    public_key: String,
    discovery_port: u16,
    discovered_peers: mpsc::UnboundedSender<(String, SocketAddr, String)>, // (node_id, addr, public_key)
}

impl DiscoveryManager {
    /// Create a new discovery manager
    pub fn new(
        node_id: String,
        listen_port: u16,
        public_key: String,
        discovery_port: u16,
        discovered_peers: mpsc::UnboundedSender<(String, SocketAddr, String)>,
    ) -> Self {
        Self {
            node_id,
            listen_port,
            public_key,
            discovery_port,
            discovered_peers,
        }
    }

    /// Start discovery: broadcast and listen
    pub async fn start(&self) -> Result<()> {
        // Spawn broadcaster
        let broadcaster = {
            let node_id = self.node_id.clone();
            let listen_port = self.listen_port;
            let public_key = self.public_key.clone();
            let discovery_port = self.discovery_port;
            tokio::spawn(async move {
                Self::broadcast_loop(node_id, listen_port, public_key, discovery_port).await;
            })
        };

        // Spawn listener
        let listener = {
            let node_id = self.node_id.clone();
            let discovered_peers = self.discovered_peers.clone();
            let discovery_port = self.discovery_port;
            tokio::spawn(async move {
                Self::listen_loop(node_id, discovered_peers, discovery_port).await;
            })
        };

        // Wait for both tasks
        tokio::select! {
            _ = broadcaster => {},
            _ = listener => {},
        }

        Ok(())
    }

    /// Broadcast discovery messages periodically
    async fn broadcast_loop(
        node_id: String,
        listen_port: u16,
        public_key: String,
        discovery_port: u16,
    ) {
        let mut interval = interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let message = DiscoveryMessage {
                node_id: node_id.clone(),
                listen_port,
                public_key: public_key.clone(),
                timestamp: chrono::Utc::now().timestamp(),
            };

            if let Ok(json) = serde_json::to_string(&message) {
                // Multicast to local network (works across different listen ports; supports local multi-node)
                if let Ok(socket) = TokioUdpSocket::bind("0.0.0.0:0").await {
                    if let Ok(socket) = socket.into_std() {
                        let _ = socket.set_multicast_ttl_v4(1);
                        let _ = socket.set_multicast_loop_v4(true);
                        let socket = TokioUdpSocket::from_std(socket).unwrap();
                        let addr = SocketAddr::new(DISCOVERY_MULTICAST.into(), discovery_port);
                        let _ = socket.send_to(json.as_bytes(), addr).await;
                        debug!("Multicasted discovery message");
                    }
                }
            }
        }
    }

    /// Listen for discovery messages
    async fn listen_loop(
        our_node_id: String,
        discovered_peers: mpsc::UnboundedSender<(String, SocketAddr, String)>,
        discovery_port: u16,
    ) {
        let bind_addr: SocketAddr = format!("0.0.0.0:{}", discovery_port)
            .parse()
            .expect("valid discovery bind addr");

        // Use SO_REUSEADDR and multicast so multiple local nodes can listen on the same port.
        let socket = match bind_udp_reuse(bind_addr) {
            Ok(s) => {
                info!("Discovery listener started on {}", bind_addr);
                s
            }
            Err(e) => {
                warn!("Failed to bind discovery socket on {}: {}", bind_addr, e);
                return;
            }
        };

        let mut buf = vec![0u8; 2048];
        let mut last_seen: std::collections::HashMap<String, Instant> =
            std::collections::HashMap::new();

        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, addr)) => {
                    if let Ok(json_str) = std::str::from_utf8(&buf[..n]) {
                        if let Ok(message) = serde_json::from_str::<DiscoveryMessage>(json_str) {
                            // Ignore our own messages
                            if message.node_id == our_node_id {
                                continue;
                            }

                            // Rate limit: only process same peer once per 5 seconds
                            let now = Instant::now();
                            if let Some(last) = last_seen.get(&message.node_id) {
                                if now.duration_since(*last) < Duration::from_secs(5) {
                                    continue;
                                }
                            }
                            last_seen.insert(message.node_id.clone(), now);

                            // Extract peer address
                            let peer_addr = SocketAddr::new(addr.ip(), message.listen_port);

                            info!(
                                "Discovered peer {} at {} (public key: {}...)",
                                message.node_id,
                                peer_addr,
                                &message.public_key[..message.public_key.len().min(20)]
                            );

                            // Send to channel
                            let _ = discovered_peers.send((
                                message.node_id,
                                peer_addr,
                                message.public_key,
                            ));
                        }
                    }
                }
                Err(e) => {
                    warn!("Error receiving discovery message: {}", e);
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }
}

fn bind_udp_reuse(addr: SocketAddr) -> std::io::Result<TokioUdpSocket> {
    let socket = Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;

    let std_sock: std::net::UdpSocket = socket.into();
    // Join multicast group for discovery (receive LAN announcements)
    let _ = std_sock.join_multicast_v4(&DISCOVERY_MULTICAST, &Ipv4Addr::UNSPECIFIED);
    std_sock.set_nonblocking(true)?;
    TokioUdpSocket::from_std(std_sock)
}
