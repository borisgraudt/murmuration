/// Message persistence: chat history stored in sled DB
/// Frugal: simple key-value, no complex queries
use crate::error::{MeshError, Result};
use crate::node::InboxMessage;
use std::path::Path;

pub struct MessageStore {
    db: sled::Db,
}

impl MessageStore {
    /// Create message store
    pub fn new(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("messages.db");
        let db = sled::open(&db_path)
            .map_err(|e| MeshError::Storage(format!("Failed to open messages DB: {}", e)))?;

        Ok(Self { db })
    }

    /// Save message to history
    pub fn save(&self, msg: &InboxMessage) -> Result<()> {
        let key = format!("msg:{}:{}", msg.timestamp, msg.seq);
        let value = serde_json::to_vec(msg).map_err(MeshError::Serialization)?;

        self.db
            .insert(key.as_bytes(), value)
            .map_err(|e| MeshError::Storage(format!("Failed to save message: {}", e)))?;

        Ok(())
    }

    /// Get recent messages (last N)
    pub fn get_recent(&self, limit: usize) -> Result<Vec<InboxMessage>> {
        let mut messages = Vec::new();

        for entry in self.db.iter().rev().take(limit).flatten() {
            let (_, value) = entry;
            if let Ok(msg) = serde_json::from_slice::<InboxMessage>(&value) {
                messages.push(msg);
            }
        }

        messages.reverse();
        Ok(messages)
    }

    /// Get message count
    pub fn count(&self) -> usize {
        self.db.len()
    }
}

impl Clone for MessageStore {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
        }
    }
}
