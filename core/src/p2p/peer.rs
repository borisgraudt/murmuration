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
                // Update address if changed, but preserve added_at
                p.address = address;
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
            }
        }
    }
    
    /// Update peer last seen
    pub async fn update_peer_last_seen(&self, node_id: &str) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.update_last_seen();
        }
    }
    
    /// Get all connected peers
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values()
            .filter(|p| p.is_connected())
            .cloned()
            .collect()
    }
    
    /// Get all known peers
    pub async fn get_all_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().cloned().collect()
    }
    
    /// Remove stale peers (only disconnected ones that haven't been seen)
    pub async fn remove_stale_peers(&self, timeout: Duration) -> usize {
        let mut peers = self.peers.write().await;
        let initial_len = peers.len();
        peers.retain(|_, peer| {
            // Don't remove connected peers
            if peer.is_connected() {
                return true;
            }
            // Don't remove peers that are trying to connect
            if peer.state == ConnectionState::Connecting || peer.state == ConnectionState::Handshaking {
                return true;
            }
            // Only remove disconnected peers that are stale
            !peer.is_stale(timeout)
        });
        initial_len - peers.len()
    }
    
    /// Increment connection attempts for a peer
    pub async fn increment_connection_attempts(&self, node_id: &str) {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(node_id) {
            peer.connection_attempts += 1;
        }
    }
    
    /// Perform handshake with a peer
    pub async fn perform_handshake(
        &self,
        stream: &mut TcpStream,
        is_incoming: bool,
    ) -> Result<(String, u8)> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use crate::p2p::protocol::Frame;
        
        if is_incoming {
            // Wait for handshake from peer
            let mut len_buf = [0u8; 4];
            timeout(Duration::from_secs(5), stream.read_exact(&mut len_buf))
                .await
                .map_err(|_| MeshError::Timeout("Handshake timeout".to_string()))?
                .map_err(|e| MeshError::Io(e))?;
            
            let length = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; length];
            timeout(Duration::from_secs(5), stream.read_exact(&mut payload))
                .await
                .map_err(|_| MeshError::Timeout("Handshake read timeout".to_string()))?
                .map_err(|e| MeshError::Io(e))?;
            
            let handshake = Message::from_bytes(&payload)
                .map_err(|e| MeshError::Protocol(format!("Invalid handshake: {}", e)))?;
            
            let (node_id, protocol_version) = match handshake {
                Message::Handshake { node_id, protocol_version, .. } => {
                    if protocol_version != PROTOCOL_VERSION {
                        return Err(MeshError::Protocol(format!(
                            "Protocol version mismatch: expected {}, got {}",
                            PROTOCOL_VERSION, protocol_version
                        )));
                    }
                    (node_id, protocol_version)
                }
                _ => return Err(MeshError::Protocol("Expected handshake message".to_string())),
            };
            
            // Send handshake ack
            let ack = Message::HandshakeAck {
                node_id: self.our_node_id.clone(),
                protocol_version: PROTOCOL_VERSION,
            };
            let frame = Frame::from_message(&ack)
                .map_err(|e| MeshError::Protocol(format!("Failed to serialize ack: {}", e)))?;
            stream.write_all(&frame.to_bytes())
                .await
                .map_err(|e| MeshError::Io(e))?;
            
            Ok((node_id, protocol_version))
        } else {
            // Send handshake
            let handshake = Message::Handshake {
                node_id: self.our_node_id.clone(),
                protocol_version: PROTOCOL_VERSION,
                listen_port: self.our_listen_port,
            };
            let frame = Frame::from_message(&handshake)
                .map_err(|e| MeshError::Protocol(format!("Failed to serialize handshake: {}", e)))?;
            stream.write_all(&frame.to_bytes())
                .await
                .map_err(|e| MeshError::Io(e))?;
            
            // Wait for ack
            let mut len_buf = [0u8; 4];
            timeout(Duration::from_secs(5), stream.read_exact(&mut len_buf))
                .await
                .map_err(|_| MeshError::Timeout("Handshake ack timeout".to_string()))?
                .map_err(|e| MeshError::Io(e))?;
            
            let length = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; length];
            timeout(Duration::from_secs(5), stream.read_exact(&mut payload))
                .await
                .map_err(|_| MeshError::Timeout("Handshake ack read timeout".to_string()))?
                .map_err(|e| MeshError::Io(e))?;
            
            let ack = Message::from_bytes(&payload)
                .map_err(|e| MeshError::Protocol(format!("Invalid handshake ack: {}", e)))?;
            
            let (node_id, protocol_version) = match ack {
                Message::HandshakeAck { node_id, protocol_version } => {
                    if protocol_version != PROTOCOL_VERSION {
                        return Err(MeshError::Protocol(format!(
                            "Protocol version mismatch: expected {}, got {}",
                            PROTOCOL_VERSION, protocol_version
                        )));
                    }
                    (node_id, protocol_version)
                }
                _ => return Err(MeshError::Protocol("Expected handshake ack".to_string())),
            };
            
            Ok((node_id, protocol_version))
        }
    }
}

