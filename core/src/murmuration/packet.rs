use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};

/// Signed (eventually) application-level packet.
///
/// For now `signature` is optional and is not validated yet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MurmurationPacket {
    pub src: String,
    /// Destination address (recommended format: `mur://<node_id>`). `None` means broadcast.
    pub dst: Option<String>,
    /// Raw payload bytes (UTF-8 text or binary).
    pub payload: Vec<u8>,
    pub timestamp: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<Vec<u8>>,
}

impl MurmurationPacket {
    pub fn new(src: String, dst_node_id: Option<String>, payload: Vec<u8>) -> Self {
        let dst = dst_node_id.map(|id| format!("mur://{}", id));
        Self {
            src,
            dst,
            payload,
            timestamp: chrono::Utc::now().timestamp(),
            signature: None,
        }
    }

    pub fn dst_node_id(&self) -> Option<&str> {
        self.dst.as_deref().and_then(|s| s.strip_prefix("mur://"))
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(MeshError::Serialization)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(MeshError::Serialization)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_roundtrip() {
        let p = MurmurationPacket::new("a".to_string(), Some("b".to_string()), b"hello".to_vec());
        let bytes = p.to_bytes().unwrap();
        let decoded = MurmurationPacket::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.src, "a");
        assert_eq!(decoded.dst_node_id(), Some("b"));
        assert_eq!(decoded.payload, b"hello".to_vec());
    }
}
