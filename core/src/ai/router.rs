/// Routing logic for mesh messages
/// Implements flooding-based routing for MVP
use crate::error::{MeshError, Result};
use crate::p2p::protocol::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::debug;

/// Elysium address format: ely://<node_id>
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElysiumAddress {
    pub node_id: String,
}

impl ElysiumAddress {
    /// Parse from string format: ely://<node_id>
    pub fn from_string(addr: &str) -> Result<Self> {
        if let Some(stripped) = addr.strip_prefix("ely://") {
            Ok(Self {
                node_id: stripped.to_string(),
            })
        } else {
            Err(MeshError::Protocol(format!("Invalid Elysium address format: {}", addr)))
        }
    }

    /// Convert to string format
    pub fn to_string(&self) -> String {
        format!("ely://{}", self.node_id)
    }
}

/// Mesh message for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    pub from: String,
    pub to: Option<String>, // None = broadcast
    pub data: Vec<u8>,
    pub message_id: String,
    pub ttl: u8,
    pub path: Vec<String>, // Route path for loop detection
}

impl MeshMessage {
    /// Create a new mesh message
    pub fn new(from: String, to: Option<String>, data: Vec<u8>) -> Self {
        Self {
            from,
            to,
            data,
            message_id: uuid::Uuid::new_v4().to_string(),
            ttl: 10, // Default TTL
            path: Vec::new(),
        }
    }

    /// Convert to protocol message
    pub fn to_protocol_message(&self) -> Message {
        Message::MeshMessage {
            from: self.from.clone(),
            to: self.to.clone(),
            data: self.data.clone(),
            message_id: self.message_id.clone(),
            ttl: self.ttl,
            path: self.path.clone(),
        }
    }

    /// Create from protocol message
    pub fn from_protocol_message(msg: &Message) -> Option<Self> {
        if let Message::MeshMessage {
            from,
            to,
            data,
            message_id,
            ttl,
            path,
        } = msg {
            Some(Self {
                from: from.clone(),
                to: to.clone(),
                data: data.clone(),
                message_id: message_id.clone(),
                ttl: *ttl,
                path: path.clone(),
            })
        } else {
            None
        }
    }
}

/// Router for mesh message routing
pub struct Router {
    our_node_id: String,
    seen_messages: Arc<RwLock<HashMap<String, Instant>>>, // message_id -> timestamp
    message_cache: Arc<RwLock<HashMap<String, MeshMessage>>>, // Cache for deduplication
}

impl Router {
    /// Create a new router
    pub fn new(our_node_id: String) -> Self {
        Self {
            our_node_id,
            seen_messages: Arc::new(RwLock::new(HashMap::new())),
            message_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if message should be processed (deduplication and TTL check)
    pub async fn should_process(&self, message: &MeshMessage) -> bool {
        // Check TTL
        if message.ttl == 0 {
            debug!("Message {} dropped: TTL expired", message.message_id);
            return false;
        }

        // Check if we've seen this message recently (within 60 seconds)
        let seen = self.seen_messages.read().await;
        if let Some(timestamp) = seen.get(&message.message_id) {
            if timestamp.elapsed() < Duration::from_secs(60) {
                debug!("Message {} dropped: already seen", message.message_id);
                return false;
            }
        }
        drop(seen);

        // Check if we're in the path (loop detection)
        if message.path.contains(&self.our_node_id) {
            debug!("Message {} dropped: loop detected", message.message_id);
            return false;
        }

        true
    }

    /// Mark message as seen
    pub async fn mark_seen(&self, message_id: &str) {
        let mut seen = self.seen_messages.write().await;
        seen.insert(message_id.to_string(), Instant::now());
        
        // Cleanup old entries (older than 5 minutes)
        seen.retain(|_, timestamp| timestamp.elapsed() < Duration::from_secs(300));
    }

    /// Check if message is for us
    pub fn is_for_us(&self, message: &MeshMessage) -> bool {
        match &message.to {
            None => true, // Broadcast
            Some(to) => to == &self.our_node_id,
        }
    }

    /// Prepare message for forwarding (decrement TTL, add to path)
    pub fn prepare_for_forwarding(&self, message: &MeshMessage) -> MeshMessage {
        let mut forward_msg = message.clone();
        forward_msg.ttl = forward_msg.ttl.saturating_sub(1);
        forward_msg.path.push(self.our_node_id.clone());
        forward_msg
    }

    /// Get list of peers to forward to (flooding: all except sender)
    pub fn get_forward_peers(&self, message: &MeshMessage, all_peers: &[String]) -> Vec<String> {
        all_peers
            .iter()
            .filter(|peer_id| {
                // Don't forward to sender
                **peer_id != message.from &&
                // Don't forward to nodes already in path (loop prevention)
                !message.path.contains(peer_id)
            })
            .cloned()
            .collect()
    }

    /// Cleanup old cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.message_cache.write().await;
        cache.retain(|_, _msg| {
            // Keep messages that are less than 5 minutes old
            true // For now, keep all cached messages
        });
    }
}

impl Clone for Router {
    fn clone(&self) -> Self {
        Self {
            our_node_id: self.our_node_id.clone(),
            seen_messages: self.seen_messages.clone(),
            message_cache: self.message_cache.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_elysium_address_parse() {
        let addr = ElysiumAddress::from_string("ely://node123").unwrap();
        assert_eq!(addr.node_id, "node123");
        
        let invalid = ElysiumAddress::from_string("invalid");
        assert!(invalid.is_err());
    }
    
    #[test]
    fn test_elysium_address_to_string() {
        let addr = ElysiumAddress { node_id: "node123".to_string() };
        assert_eq!(addr.to_string(), "ely://node123");
    }
    
    #[tokio::test]
    async fn test_router_should_process() {
        let router = Router::new("our-node".to_string());
        let message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        
        // New message should be processed
        assert!(router.should_process(&message).await);
        
        // Mark as seen
        router.mark_seen(&message.message_id).await;
        
        // Should not process again immediately
        assert!(!router.should_process(&message).await);
    }
    
    #[tokio::test]
    async fn test_router_is_for_us() {
        let router = Router::new("our-node".to_string());
        
        // Broadcast message
        let broadcast = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        assert!(router.is_for_us(&broadcast));
        
        // Directed to us
        let directed = MeshMessage::new("peer1".to_string(), Some("our-node".to_string()), b"test".to_vec());
        assert!(router.is_for_us(&directed));
        
        // Directed to someone else
        let other = MeshMessage::new("peer1".to_string(), Some("other-node".to_string()), b"test".to_vec());
        assert!(!router.is_for_us(&other));
    }
    
    #[tokio::test]
    async fn test_router_prepare_for_forwarding() {
        let router = Router::new("our-node".to_string());
        let message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        let original_ttl = message.ttl;
        
        let forwarded = router.prepare_for_forwarding(&message);
        
        assert_eq!(forwarded.ttl, original_ttl - 1);
        assert!(forwarded.path.contains(&"our-node".to_string()));
    }
    
    #[tokio::test]
    async fn test_router_get_forward_peers() {
        let router = Router::new("our-node".to_string());
        let message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        let all_peers = vec!["peer1".to_string(), "peer2".to_string(), "peer3".to_string()];
        
        let forward_peers = router.get_forward_peers(&message, &all_peers);
        
        // Should not include sender (peer1) or our node
        assert!(!forward_peers.contains(&"peer1".to_string()));
        assert!(forward_peers.contains(&"peer2".to_string()));
        assert!(forward_peers.contains(&"peer3".to_string()));
    }
    
    #[tokio::test]
    async fn test_router_loop_detection() {
        let router = Router::new("our-node".to_string());
        let mut message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        message.path.push("our-node".to_string());
        
        // Should not process if we're in the path
        assert!(!router.should_process(&message).await);
    }
}
