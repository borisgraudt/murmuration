/// Protocol definitions for P2P communication
use serde::{Deserialize, Serialize};
use std::fmt;

/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Message types in the protocol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Message {
    /// Handshake message sent when establishing connection
    #[serde(rename = "handshake")]
    Handshake {
        node_id: String,
        protocol_version: u8,
        listen_port: u16,
        public_key: Option<String>, // RSA public key (base64 encoded)
    },
    
    /// Acknowledgment of handshake
    #[serde(rename = "handshake_ack")]
    HandshakeAck {
        node_id: String,
        protocol_version: u8,
        public_key: Option<String>, // RSA public key (base64 encoded)
        encrypted_session_key: Option<Vec<u8>>, // AES session key encrypted with our RSA public key
        nonce: Option<Vec<u8>>, // AES nonce
    },
    
    /// Regular data message
    #[serde(rename = "data")]
    Data {
        payload: Vec<u8>,
        message_id: String,
    },
    
    /// Ping message for keepalive
    #[serde(rename = "ping")]
    Ping {
        timestamp: i64,
    },
    
    /// Pong response to ping
    #[serde(rename = "pong")]
    Pong {
        timestamp: i64,
    },
    
    /// Peer discovery request
    #[serde(rename = "peer_request")]
    PeerRequest,
    
    /// Peer discovery response
    #[serde(rename = "peer_response")]
    PeerResponse {
        peers: Vec<String>,
    },
    
    /// Connection close notification
    #[serde(rename = "close")]
    Close {
        reason: String,
    },
    
    /// Elysium mesh message for routing
    #[serde(rename = "mesh_message")]
    MeshMessage {
        from: String,
        to: Option<String>, // None = broadcast
        data: Vec<u8>,
        message_id: String,
        ttl: u8, // Time to live (hop count)
        path: Vec<String>, // Route path for loop detection
    },
}

impl Message {
    /// Serialize message to JSON bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
    
    /// Deserialize message from JSON bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }
    
    /// Get message type as string
    pub fn message_type(&self) -> &'static str {
        match self {
            Message::Handshake { .. } => "handshake",
            Message::HandshakeAck { .. } => "handshake_ack",
            Message::Data { .. } => "data",
            Message::Ping { .. } => "ping",
            Message::Pong { .. } => "pong",
            Message::PeerRequest => "peer_request",
            Message::PeerResponse { .. } => "peer_response",
            Message::Close { .. } => "close",
            Message::MeshMessage { .. } => "mesh_message",
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Message({})", self.message_type())
    }
}

/// Protocol frame with length prefix
/// Can contain either plain or encrypted payload
#[derive(Debug)]
pub struct Frame {
    pub length: u32,
    pub payload: Vec<u8>,
    pub is_encrypted: bool, // If true, payload is encrypted (nonce + encrypted_data)
}

impl Frame {
    /// Create a new frame from a message (plain)
    pub fn from_message(message: &Message) -> Result<Self, serde_json::Error> {
        let payload = message.to_bytes()?;
        Ok(Self {
            length: payload.len() as u32,
            payload,
            is_encrypted: false,
        })
    }
    
    /// Create an encrypted frame (nonce + encrypted_data)
    pub fn from_encrypted(nonce: &[u8], encrypted_data: &[u8]) -> Self {
        let mut payload = Vec::with_capacity(12 + encrypted_data.len());
        payload.extend_from_slice(nonce); // 12 bytes nonce
        payload.extend_from_slice(encrypted_data);
        Self {
            length: payload.len() as u32,
            payload,
            is_encrypted: true,
        }
    }
    
    /// Extract nonce and encrypted data from encrypted frame
    pub fn extract_encrypted(&self) -> Option<(&[u8], &[u8])> {
        if !self.is_encrypted || self.payload.len() < 12 {
            return None;
        }
        Some((&self.payload[0..12], &self.payload[12..]))
    }
    
    /// Serialize frame to bytes (length prefix + payload)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.payload.len());
        buf.extend_from_slice(&self.length.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }
    
    /// Parse frame from bytes (assumes plain by default, encryption handled at higher level)
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        
        let length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        
        if data.len() < 4 + length {
            return None;
        }
        
        Some(Self {
            length: length as u32,
            payload: data[4..4 + length].to_vec(),
            is_encrypted: false, // Will be determined by context
        })
    }
    
    /// Parse encrypted frame from bytes
    pub fn from_encrypted_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        
        let length = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        
        if data.len() < 4 + length || length < 12 {
            return None;
        }
        
        Some(Self {
            length: length as u32,
            payload: data[4..4 + length].to_vec(),
            is_encrypted: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_serialization() {
        let msg = Message::Ping { timestamp: 12345 };
        let bytes = msg.to_bytes().unwrap();
        let deserialized = Message::from_bytes(&bytes).unwrap();
        assert_eq!(msg, deserialized);
    }
    
    #[test]
    fn test_frame_serialization() {
        let msg = Message::Ping { timestamp: 12345 };
        let frame = Frame::from_message(&msg).unwrap();
        let bytes = frame.to_bytes();
        let parsed = Frame::from_bytes(&bytes).unwrap();
        assert_eq!(frame.length, parsed.length);
        assert_eq!(frame.payload, parsed.payload);
    }
}

