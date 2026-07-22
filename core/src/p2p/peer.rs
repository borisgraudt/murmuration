/// Peer management and connection state
use crate::error::{MeshError, Result};
use crate::p2p::protocol::{Message, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::RwLock;

/// Connection state of a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Handshake in progress
    Handshaking,
    /// Fully connected and ready
    Connected,
    /// Connection is closing
    Closing,
}

/// Peer metrics for AI routing
#[derive(Debug, Clone)]
pub struct PeerMetrics {
    /// Average latency (measured via ping/pong)
    pub latency: Option<Duration>,
    /// Uptime (how long peer has been connected)
    pub uptime: Duration,
    /// Number of successful pings
    pub ping_count: u32,
    /// Number of failed pings
    pub ping_failures: u32,
    /// Last ping timestamp
    pub last_ping: Option<Instant>,
}

impl Default for PeerMetrics {
    fn default() -> Self {
        Self {
            latency: None,
            uptime: Duration::ZERO,
            ping_count: 0,
            ping_failures: 0,
            last_ping: None,
        }
    }
}

impl PeerMetrics {
    /// Update latency with new measurement (exponential moving average)
    pub fn update_latency(&mut self, new_latency: Duration) {
        const ALPHA: f64 = 0.3; // Smoothing factor
        self.latency = Some(
            self.latency
                .map(|old| {
                    let old_ms = old.as_secs_f64() * 1000.0;
                    let new_ms = new_latency.as_secs_f64() * 1000.0;
                    let smoothed = ALPHA * new_ms + (1.0 - ALPHA) * old_ms;
                    Duration::from_millis(smoothed as u64)
                })
                .unwrap_or(new_latency),
        );
        self.ping_count += 1;
        self.last_ping = Some(Instant::now());
    }

    /// Record a ping failure
    pub fn record_ping_failure(&mut self) {
        self.ping_failures += 1;
    }

    /// Calculate reliability score (0.0 to 1.0)
    pub fn reliability_score(&self) -> f32 {
        let total_pings = self.ping_count + self.ping_failures;
        if total_pings == 0 {
            return 0.5; // Default score
        }
        (self.ping_count as f32 / total_pings as f32).clamp(0.0, 1.0)
    }
}

/// Information about a peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: String,
    pub address: SocketAddr,
    pub state: ConnectionState,
    pub protocol_version: Option<u8>,
    pub last_seen: Option<Instant>,
    pub connected_at: Option<Instant>,
    pub connection_attempts: u32,
    pub added_at: Instant,    // When this peer was first added
    pub metrics: PeerMetrics, // Metrics for AI routing
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(node_id: String, address: SocketAddr) -> Self {
        Self {
            node_id,
            address,
            state: ConnectionState::Disconnected,
            protocol_version: None,
            last_seen: None,
            connected_at: None,
            connection_attempts: 0,
            added_at: Instant::now(),
            metrics: PeerMetrics::default(),
        }
    }

    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = Some(Instant::now());
    }

    /// Update uptime based on connected_at (call periodically to keep metrics fresh)
    pub fn update_uptime(&mut self) {
        if let Some(connected_at) = self.connected_at {
            self.metrics.uptime = connected_at.elapsed();
        } else {
            self.metrics.uptime = Duration::ZERO;
        }
    }

    /// Get current uptime
    pub fn get_uptime(&self) -> Duration {
        if let Some(connected_at) = self.connected_at {
            connected_at.elapsed()
        } else {
            Duration::ZERO
        }
    }

    /// Check if peer should be considered stale
    pub fn is_stale(&self, timeout: Duration) -> bool {
        // Don't consider peers stale if they were just added (within 30 seconds)
        if self.added_at.elapsed() < Duration::from_secs(30) {
            return false;
        }

        if let Some(last_seen) = self.last_seen {
            last_seen.elapsed() > timeout
        } else {
            // If never seen, consider stale only if added more than timeout ago
            self.added_at.elapsed() > timeout
        }
    }
}

/// Peer manager for tracking and managing peer connections
#[derive(Clone)]
pub struct PeerManager {
    peers: Arc<RwLock<std::collections::HashMap<String, PeerInfo>>>,
    our_node_id: String,
    #[allow(dead_code)]
    our_listen_port: u16,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(our_node_id: String, our_listen_port: u16) -> Self {
        Self {
            peers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            our_node_id,
            our_listen_port,
        }
    }

    /// Add or update a peer
    pub async fn add_peer(&self, node_id: String, address: SocketAddr) {
        let mut peers = self.peers.write().await;
        peers
            .entry(node_id.clone())
            .and_modify(|p| {
                p.address = address;
                p.update_last_seen();
            })
            .or_insert_with(|| PeerInfo::new(node_id, address));
    }

    /// Get peer info
    pub async fn get_peer(&self, node_id: &str) -> Option<PeerInfo> {
        let peers = self.peers.read().await;
        peers.get(node_id).cloned()
    }

    /// Update peer state
    pub async fn update_peer_state(&self, node_id: &str, state: ConnectionState) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.state = state;
            if state == ConnectionState::Connected {
                peer.connected_at = Some(Instant::now());
                peer.update_last_seen();
                peer.update_uptime();
            } else if state == ConnectionState::Disconnected {
                // Reset uptime when disconnected
                peer.metrics.uptime = Duration::ZERO;
            }
        }
    }

    /// Update peer protocol version
    pub async fn update_peer_protocol(&self, node_id: &str, version: u8) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.protocol_version = Some(version);
        }
    }

    /// Get all peers
    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }

    /// Get connected peers only
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.is_connected())
            .cloned()
            .collect()
    }

    /// Remove stale peers
    pub async fn remove_stale_peers(&self, timeout: Duration) -> usize {
        let mut peers = self.peers.write().await;
        let before = peers.len();
        peers.retain(|_, peer| {
            // Don't remove if connecting, handshaking, or connected
            if matches!(
                peer.state,
                ConnectionState::Connecting
                    | ConnectionState::Handshaking
                    | ConnectionState::Connected
            ) {
                return true;
            }
            !peer.is_stale(timeout)
        });
        before - peers.len()
    }

    /// Increment connection attempts
    pub async fn increment_connection_attempts(&self, node_id: &str) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.connection_attempts += 1;
        }
    }

    /// Update peer last seen timestamp
    pub async fn update_peer_last_seen(&self, node_id: &str) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.update_last_seen();
            peer.update_uptime();
        }
    }

    /// Update peer latency (called after ping/pong)
    pub async fn update_peer_latency(&self, node_id: &str, latency: Duration) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.metrics.update_latency(latency);
            peer.update_last_seen();
            peer.update_uptime();
        }
    }

    /// Record ping failure for a peer
    pub async fn record_ping_failure(&self, node_id: &str) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.metrics.record_ping_failure();
        }
    }

    /// Perform handshake with a peer.
    ///
    /// Protocol v2: includes X25519 ephemeral public keys in both directions.
    /// Session key = HKDF-SHA256(DH(our_ephemeral_priv, peer_ephemeral_pub), info="murmuration-v2").
    /// Falls back to RSA key-encapsulation (v1) if peer omits ephemeral_pubkey.
    pub async fn perform_handshake(
        &self,
        stream: &mut TcpStream,
        is_outgoing: bool,
        encryption_manager: Option<&crate::p2p::encryption::EncryptionManager>,
        session_keys: Option<&crate::p2p::encryption::SessionKeyManager>,
    ) -> Result<(String, u8, Option<rsa::RsaPublicKey>)> {
        use crate::p2p::encryption::EncryptionManager;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        if is_outgoing {
            // For outgoing connection: send handshake first, then read ack
            let our_public_key = if let Some(enc_mgr) = encryption_manager {
                enc_mgr.get_public_key_string()?
            } else {
                String::new()
            };

            // Generate ephemeral X25519 keypair for forward secrecy
            let our_ephemeral_secret =
                x25519_dalek::EphemeralSecret::random_from_rng(rand::rngs::OsRng);
            let our_ephemeral_public = x25519_dalek::PublicKey::from(&our_ephemeral_secret);
            let ephemeral_pubkey_b64 = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                our_ephemeral_public.as_bytes(),
            );

            let handshake_message = Message::Handshake {
                node_id: self.our_node_id.clone(),
                protocol_version: PROTOCOL_VERSION,
                listen_port: self.our_listen_port,
                public_key: Some(our_public_key),
                ephemeral_pubkey: Some(ephemeral_pubkey_b64),
            };

            let handshake_bytes = handshake_message
                .to_bytes()
                .map_err(MeshError::Serialization)?;
            let len = handshake_bytes.len() as u32;

            stream
                .write_all(&len.to_be_bytes())
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to write handshake length: {}", e)))?;
            stream
                .write_all(&handshake_bytes)
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to write handshake: {}", e)))?;
            stream
                .flush()
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to flush handshake: {}", e)))?;

            // Read handshake ack
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await.map_err(|e| {
                MeshError::Peer(format!("Failed to read handshake ack length: {}", e))
            })?;
            let len = u32::from_be_bytes(len_buf) as usize;

            // Validate length - RSA 2048 encrypted session key + JSON overhead should be < 4KB
            // Reasonable upper bound: 10KB (allows for future protocol extensions)
            const MAX_HANDSHAKE_SIZE: usize = 10 * 1024; // 10KB
            const MIN_HANDSHAKE_SIZE: usize = 10; // Minimum reasonable size

            // Sanity check: if length looks like ASCII or is suspiciously large, something's wrong
            if !(MIN_HANDSHAKE_SIZE..=MAX_HANDSHAKE_SIZE).contains(&len) {
                // Check if we received HTTP response instead of handshake
                let first_bytes = &len_buf[..4.min(len_buf.len())];
                let is_http = first_bytes == b"HTTP"
                    || (first_bytes.len() >= 3 && &first_bytes[0..3] == b"GET")
                    || (first_bytes.len() >= 4 && &first_bytes[0..4] == b"POST");

                if is_http {
                    tracing::error!(
                        "Received HTTP response instead of handshake (raw bytes: {:?}, hex: {:02x}{:02x}{:02x}{:02x})",
                        first_bytes, len_buf[0], len_buf[1], len_buf[2], len_buf[3]
                    );
                    return Err(MeshError::Peer(
                        "Connected to HTTP port (Web Gateway/API) instead of P2P port. \
                         Make sure you're connecting to the P2P port (e.g., 8080), not the API port (e.g., 17080) or Gateway port (e.g., 17081). \
                         Usage: mur start <p2p_port> [peer_ip:p2p_port]"
                            .to_string(),
                    ));
                }

                tracing::error!(
                    "Invalid handshake ack length: {} bytes (raw bytes: {:?}, hex: {:02x}{:02x}{:02x}{:02x})",
                    len, first_bytes, len_buf[0], len_buf[1], len_buf[2], len_buf[3]
                );
                return Err(MeshError::Peer(format!(
                    "Invalid handshake ack length: {} bytes (expected {}..{} bytes). \
                     This may indicate protocol desynchronization or corrupted data.",
                    len, MIN_HANDSHAKE_SIZE, MAX_HANDSHAKE_SIZE
                )));
            }

            tracing::debug!("Reading handshake ack, size: {} bytes", len);

            let mut buf = vec![0u8; len];
            stream
                .read_exact(&mut buf)
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to read handshake ack: {}", e)))?;

            // Parse handshake ack
            let message: Message = Message::from_bytes(&buf).map_err(|e| {
                MeshError::Peer(format!("Failed to parse handshake ack message: {}", e))
            })?;

            let (peer_id, protocol_version, peer_public_key) = match message {
                Message::HandshakeAck {
                    node_id,
                    protocol_version,
                    public_key,
                    encrypted_session_key,
                    nonce,
                    ephemeral_pubkey,
                } => {
                    // Parse peer's RSA public key if provided
                    let peer_pub_key = if let Some(pub_key_str) = &public_key {
                        Some(EncryptionManager::parse_public_key(pub_key_str)?)
                    } else {
                        None
                    };

                    if let Some(sess_keys) = session_keys {
                        if let Some(peer_eph_b64) = ephemeral_pubkey {
                            // ── v2: X25519 DH forward-secret path ──────────────────
                            match base64::Engine::decode(
                                &base64::engine::general_purpose::STANDARD,
                                &peer_eph_b64,
                            ) {
                                Ok(peer_eph_bytes) if peer_eph_bytes.len() == 32 => {
                                    let mut arr = [0u8; 32];
                                    arr.copy_from_slice(&peer_eph_bytes);
                                    let peer_eph_pub = x25519_dalek::PublicKey::from(arr);
                                    let shared = our_ephemeral_secret.diffie_hellman(&peer_eph_pub);
                                    let aes_key = EncryptionManager::derive_session_key_hkdf(
                                        shared.as_bytes(),
                                    );
                                    sess_keys
                                        .set_session_key(node_id.clone(), aes_key, vec![])
                                        .await;
                                    tracing::info!(
                                        "Forward-secret session established with {} (X25519+HKDF)",
                                        node_id
                                    );
                                }
                                _ => tracing::warn!(
                                    "Invalid ephemeral pubkey from {}, skipping encryption",
                                    node_id
                                ),
                            }
                        } else if let (Some(enc_mgr), Some(enc_key), Some(nonce_bytes)) =
                            (encryption_manager, encrypted_session_key, nonce)
                        {
                            // ── v1 fallback: RSA key encapsulation ─────────────────
                            match enc_mgr.decrypt_with_private_key(&enc_key).await {
                                Ok(aes_key_bytes) => {
                                    #[allow(deprecated)]
                                    let aes_key = aes_gcm::Key::<aes_gcm::Aes256Gcm>::from_slice(
                                        &aes_key_bytes,
                                    );
                                    sess_keys
                                        .set_session_key(node_id.clone(), *aes_key, nonce_bytes)
                                        .await;
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to decrypt session key: {}", e);
                                }
                            }
                        }
                    }

                    (node_id, protocol_version, peer_pub_key)
                }
                _ => {
                    return Err(MeshError::Peer(
                        "Expected handshake ack message".to_string(),
                    ))
                }
            };

            Ok((peer_id, protocol_version, peer_public_key))
        } else {
            // For incoming connection: read handshake first, then send ack
            // Read handshake
            let mut len_buf = [0u8; 4];
            stream
                .read_exact(&mut len_buf)
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to read handshake length: {}", e)))?;
            let len = u32::from_be_bytes(len_buf) as usize;

            // Validate length - RSA public keys + JSON should be < 4KB
            const MAX_HANDSHAKE_SIZE: usize = 10 * 1024; // 10KB
            const MIN_HANDSHAKE_SIZE: usize = 10; // Minimum reasonable size

            if !(MIN_HANDSHAKE_SIZE..=MAX_HANDSHAKE_SIZE).contains(&len) {
                tracing::error!(
                    "Invalid handshake length: {} bytes (raw bytes: {:?}, hex: {:02x}{:02x}{:02x}{:02x})",
                    len,
                    len_buf,
                    len_buf[0], len_buf[1], len_buf[2], len_buf[3]
                );
                return Err(MeshError::Peer(format!(
                    "Invalid handshake length: {} bytes (expected {}..{} bytes). \
                     This may indicate protocol desynchronization or corrupted data.",
                    len, MIN_HANDSHAKE_SIZE, MAX_HANDSHAKE_SIZE
                )));
            }

            tracing::debug!("Reading handshake, size: {} bytes", len);

            let mut buf = vec![0u8; len];
            stream
                .read_exact(&mut buf)
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to read handshake: {}", e)))?;

            // Parse message from payload
            let message: Message = Message::from_bytes(&buf).map_err(|e| {
                MeshError::Peer(format!("Failed to parse handshake message: {}", e))
            })?;

            // Generate our ephemeral X25519 keypair before parsing (needed for v2 DH)
            let our_eph_secret = x25519_dalek::EphemeralSecret::random_from_rng(rand::rngs::OsRng);
            let our_eph_public = x25519_dalek::PublicKey::from(&our_eph_secret);
            let our_eph_b64 = base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                our_eph_public.as_bytes(),
            );

            let (peer_id, protocol_version, peer_public_key, peer_eph_pubkey) = match message {
                Message::Handshake {
                    node_id,
                    protocol_version,
                    listen_port: _,
                    public_key,
                    ephemeral_pubkey,
                } => {
                    let peer_pub_key = if let Some(pub_key_str) = &public_key {
                        Some(EncryptionManager::parse_public_key(pub_key_str)?)
                    } else {
                        None
                    };
                    (node_id, protocol_version, peer_pub_key, ephemeral_pubkey)
                }
                _ => return Err(MeshError::Peer("Expected handshake message".to_string())),
            };

            // Build HandshakeAck — prefer X25519 DH (v2), fall back to RSA (v1)
            let our_public_key = if let Some(enc_mgr) = encryption_manager {
                enc_mgr.get_public_key_string()?
            } else {
                String::new()
            };

            let (encrypted_session_key, nonce, ack_ephemeral) = if let Some(peer_eph_b64) =
                &peer_eph_pubkey
            {
                // ── v2: X25519 DH ────────────────────────────────────────────────
                match base64::Engine::decode(
                    &base64::engine::general_purpose::STANDARD,
                    peer_eph_b64,
                ) {
                    Ok(bytes) if bytes.len() == 32 => {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        let peer_eph_pub = x25519_dalek::PublicKey::from(arr);
                        let shared = our_eph_secret.diffie_hellman(&peer_eph_pub);
                        let aes_key = EncryptionManager::derive_session_key_hkdf(shared.as_bytes());
                        if let Some(sess_keys) = session_keys {
                            sess_keys
                                .set_session_key(peer_id.clone(), aes_key, vec![])
                                .await;
                        }
                        tracing::info!(
                            "Forward-secret session established with {} (X25519+HKDF)",
                            peer_id
                        );
                        (None, None, Some(our_eph_b64))
                    }
                    _ => {
                        tracing::warn!("Bad ephemeral key from {}, falling back to RSA", peer_id);
                        (None, None, None)
                    }
                }
            } else if let (Some(enc_mgr), Some(sess_keys)) = (encryption_manager, session_keys) {
                // ── v1 fallback: RSA key encapsulation ───────────────────────────
                if let Some(peer_pub_key) = &peer_public_key {
                    let (aes_key, nonce_bytes) = EncryptionManager::generate_session_key();
                    #[allow(deprecated)]
                    let encrypted_key =
                        enc_mgr.encrypt_with_public_key(aes_key.as_slice(), peer_pub_key)?;
                    sess_keys
                        .set_session_key(peer_id.clone(), aes_key, nonce_bytes.clone())
                        .await;
                    (Some(encrypted_key), Some(nonce_bytes), None)
                } else {
                    (None, None, None)
                }
            } else {
                (None, None, None)
            };

            let ack_message = Message::HandshakeAck {
                node_id: self.our_node_id.clone(),
                protocol_version: PROTOCOL_VERSION,
                public_key: Some(our_public_key),
                encrypted_session_key,
                nonce,
                ephemeral_pubkey: ack_ephemeral,
            };

            let ack_bytes = ack_message.to_bytes().map_err(MeshError::Serialization)?;
            let len = ack_bytes.len() as u32;

            stream.write_all(&len.to_be_bytes()).await.map_err(|e| {
                MeshError::Peer(format!("Failed to write handshake ack length: {}", e))
            })?;
            stream
                .write_all(&ack_bytes)
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to write handshake ack: {}", e)))?;
            stream
                .flush()
                .await
                .map_err(|e| MeshError::Peer(format!("Failed to flush handshake ack: {}", e)))?;

            use crate::p2p::encryption_pqc::is_pqc_available;
            if is_pqc_available() {
                tracing::info!("Forward-secret PQC session established with {}", peer_id);
            } else if peer_eph_pubkey.is_some() {
                tracing::info!(
                    "Forward-secret session established with {} (X25519)",
                    peer_id
                );
            } else {
                tracing::info!("RSA session established with {} (v1 fallback)", peer_id);
            }

            Ok((peer_id, protocol_version, peer_public_key))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_peer_info_new() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let peer = PeerInfo::new("test-node".to_string(), addr);

        assert_eq!(peer.node_id, "test-node");
        assert_eq!(peer.address, addr);
        assert_eq!(peer.state, ConnectionState::Disconnected);
        assert_eq!(peer.protocol_version, None);
        assert_eq!(peer.connection_attempts, 0);
    }

    #[tokio::test]
    async fn test_peer_info_is_connected() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut peer = PeerInfo::new("test-node".to_string(), addr);

        assert!(!peer.is_connected());
        peer.state = ConnectionState::Connected;
        assert!(peer.is_connected());
    }

    #[tokio::test]
    async fn test_peer_info_is_stale() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let peer = PeerInfo::new("test-node".to_string(), addr);

        // New peer should not be stale
        assert!(!peer.is_stale(Duration::from_secs(60)));

        // Wait a bit and check again (should still not be stale due to 30s grace period)
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!peer.is_stale(Duration::from_secs(60)));
    }

    #[tokio::test]
    async fn test_peer_manager_add_peer() {
        let manager = PeerManager::new("our-node".to_string(), 8080);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);

        manager.add_peer("peer1".to_string(), addr).await;

        let peer = manager.get_peer("peer1").await;
        assert!(peer.is_some());
        let peer = peer.unwrap();
        assert_eq!(peer.node_id, "peer1");
        assert_eq!(peer.address, addr);
    }

    #[tokio::test]
    async fn test_peer_manager_update_state() {
        let manager = PeerManager::new("our-node".to_string(), 8080);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);

        manager.add_peer("peer1".to_string(), addr).await;
        manager
            .update_peer_state("peer1", ConnectionState::Connected)
            .await;

        let peer = manager.get_peer("peer1").await.unwrap();
        assert_eq!(peer.state, ConnectionState::Connected);
        assert!(peer.connected_at.is_some());
    }

    #[tokio::test]
    async fn test_peer_manager_get_connected_peers() {
        let manager = PeerManager::new("our-node".to_string(), 8080);
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082);

        manager.add_peer("peer1".to_string(), addr1).await;
        manager.add_peer("peer2".to_string(), addr2).await;

        manager
            .update_peer_state("peer1", ConnectionState::Connected)
            .await;
        manager
            .update_peer_state("peer2", ConnectionState::Disconnected)
            .await;

        let connected = manager.get_connected_peers().await;
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].node_id, "peer1");
    }
}
