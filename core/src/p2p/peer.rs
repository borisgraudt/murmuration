/// Peer management and connection state
use crate::error::{MeshError, Result};
use crate::p2p::protocol::{Message, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::timeout;
// Tracing imports removed - using in node.rs instead

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
    pub added_at: Instant, // When this peer was first added
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
        peers.entry(node_id.clone())
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
        peers.values()
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
            if matches!(peer.state, ConnectionState::Connecting | ConnectionState::Handshaking | ConnectionState::Connected) {
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
        }
    }
    
    /// Perform handshake with a peer
    pub async fn perform_handshake(
        &self,
        stream: &mut TcpStream,
        is_outgoing: bool,
        encryption_manager: Option<&crate::p2p::encryption::EncryptionManager>,
        session_keys: Option<&crate::p2p::encryption::SessionKeyManager>,
    ) -> Result<(String, u8, Option<rsa::RsaPublicKey>)> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use crate::p2p::protocol::Frame;
        use crate::p2p::encryption::EncryptionManager;
        
        // Read handshake
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await
            .map_err(|e| MeshError::Peer(format!("Failed to read handshake length: {}", e)))?;
        let len = u32::from_be_bytes(len_buf) as usize;
        
        if len > 65536 {
            return Err(MeshError::Peer("Handshake message too large".to_string()));
        }
        
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await
            .map_err(|e| MeshError::Peer(format!("Failed to read handshake: {}", e)))?;
        
        // Parse message from payload
        let message: Message = Message::from_bytes(&buf)
            .map_err(|e| MeshError::Peer(format!("Failed to parse handshake message: {}", e)))?;
        
        let (peer_id, protocol_version, peer_public_key) = match message {
            Message::Handshake { node_id, protocol_version, listen_port: _, public_key } => {
                // Parse peer's public key if provided
                let peer_pub_key = if let Some(pub_key_str) = &public_key {
                    Some(EncryptionManager::parse_public_key(pub_key_str)?)
                } else {
                    None
                };
                
                (node_id, protocol_version, peer_pub_key)
            }
            _ => return Err(MeshError::Peer("Expected handshake message".to_string())),
        };
        
        // Send handshake ack
        let our_public_key = if let Some(enc_mgr) = encryption_manager {
            enc_mgr.get_public_key_string()?
        } else {
            String::new()
        };
        
        // Generate and encrypt session key if we have encryption manager
        let (encrypted_session_key, nonce) = if let (Some(enc_mgr), Some(sess_keys)) = (encryption_manager, session_keys) {
            if let Some(peer_pub_key) = &peer_public_key {
                // Generate AES session key
                let (aes_key, nonce_bytes) = EncryptionManager::generate_session_key();
                
                // Encrypt session key with peer's public key
                let encrypted_key = enc_mgr.encrypt_with_public_key(aes_key.as_slice(), peer_pub_key)?;
                
                // Store session key
                sess_keys.set_session_key(peer_id.clone(), aes_key, nonce_bytes.clone()).await;
                
                (Some(encrypted_key), Some(nonce_bytes))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        
        let ack_message = Message::HandshakeAck {
            node_id: self.our_node_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            public_key: Some(our_public_key),
            encrypted_session_key,
            nonce,
        };
        
        let ack_bytes = ack_message.to_bytes()
            .map_err(|e| MeshError::Serialization(e))?;
        let len = ack_bytes.len() as u32;
        
        stream.write_all(&len.to_be_bytes()).await
            .map_err(|e| MeshError::Peer(format!("Failed to write handshake ack length: {}", e)))?;
        stream.write_all(&ack_bytes).await
            .map_err(|e| MeshError::Peer(format!("Failed to write handshake ack: {}", e)))?;
        stream.flush().await
            .map_err(|e| MeshError::Peer(format!("Failed to flush handshake ack: {}", e)))?;
        
        Ok((peer_id, protocol_version, peer_public_key))
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
        manager.update_peer_state("peer1", ConnectionState::Connected).await;
        
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
        
        manager.update_peer_state("peer1", ConnectionState::Connected).await;
        manager.update_peer_state("peer2", ConnectionState::Disconnected).await;
        
        let connected = manager.get_connected_peers().await;
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].node_id, "peer1");
    }
}
