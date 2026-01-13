/// Bundle protocol for store-and-forward (USB/SD card transfer)
/// Frugal: simple batch export/import, no complex routing yet
use crate::error::{MeshError, Result};
use crate::node::InboxMessage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageBundle {
    pub version: u8,
    pub created_at: i64,
    pub expires_at: i64,
    pub messages: Vec<InboxMessage>,
}

impl MessageBundle {
    /// Create new bundle from messages
    pub fn new(messages: Vec<InboxMessage>, ttl_days: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            version: 1,
            created_at: now,
            expires_at: now + (ttl_days * 86400),
            messages,
        }
    }

    /// Export bundle to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(MeshError::Serialization)?;
        fs::write(path, json)
            .map_err(MeshError::Io)?;
        Ok(())
    }

    /// Load bundle from file
    pub fn load(path: &Path) -> Result<Self> {
        let data = fs::read(path)
            .map_err(MeshError::Io)?;
        let bundle: MessageBundle = serde_json::from_slice(&data)
            .map_err(MeshError::Serialization)?;
        
        // Check version
        if bundle.version != 1 {
            return Err(MeshError::Protocol(format!(
                "Unsupported bundle version: {}",
                bundle.version
            )));
        }
        
        // Check expiry
        let now = chrono::Utc::now().timestamp();
        if bundle.expires_at < now {
            return Err(MeshError::Protocol("Bundle expired".to_string()));
        }
        
        Ok(bundle)
    }

    /// Get bundle info (for CLI display)
    pub fn info(&self) -> BundleInfo {
        BundleInfo {
            version: self.version,
            created_at: chrono::DateTime::from_timestamp(self.created_at, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            expires_at: chrono::DateTime::from_timestamp(self.expires_at, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            message_count: self.messages.len(),
            total_bytes: self.messages.iter().map(|m| m.bytes).sum(),
        }
    }
}

#[derive(Debug)]
pub struct BundleInfo {
    pub version: u8,
    pub created_at: String,
    pub expires_at: String,
    pub message_count: usize,
    pub total_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bundle_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let bundle_path = temp_dir.path().join("test.bundle");

        let messages = vec![InboxMessage {
            seq: 1,
            timestamp: "2024-01-07T12:00:00Z".to_string(),
            direction: "in".to_string(),
            kind: "mesh".to_string(),
            peer: "peer1".to_string(),
            from: "alice".to_string(),
            to: Some("bob".to_string()),
            message_id: Some("msg1".to_string()),
            bytes: 100,
            preview: "test message".to_string(),
        }];

        let bundle = MessageBundle::new(messages.clone(), 7);
        bundle.save(&bundle_path).unwrap();

        let loaded = MessageBundle::load(&bundle_path).unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.messages.len(), 1);
    }
}





