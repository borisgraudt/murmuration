/// Simple naming system: ely://name → node_id resolution
/// Frugal chic: local cache only, no gossip yet
use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameRecord {
    pub name: String,
    pub node_id: String,
    pub timestamp: i64,
}

#[derive(Clone)]
pub struct NameRegistry {
    cache: Arc<RwLock<HashMap<String, NameRecord>>>,
    db: Option<sled::Db>,
}

impl NameRegistry {
    /// Create new registry (in-memory only)
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            db: None,
        }
    }

    /// Create registry with persistent storage
    pub fn with_storage(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("names.db");
        let db = sled::open(&db_path)
            .map_err(|e| MeshError::Storage(format!("Failed to open names DB: {}", e)))?;

        let mut cache = HashMap::new();

        // Load existing names
        for entry in db.iter() {
            if let Ok((key, value)) = entry {
                if let Ok(record) = serde_json::from_slice::<NameRecord>(&value) {
                    cache.insert(String::from_utf8_lossy(&key).to_string(), record);
                }
            }
        }

        Ok(Self {
            cache: Arc::new(RwLock::new(cache)),
            db: Some(db),
        })
    }

    /// Register a name (local only)
    pub async fn register(&self, name: String, node_id: String) -> Result<()> {
        let record = NameRecord {
            name: name.clone(),
            node_id,
            timestamp: chrono::Utc::now().timestamp(),
        };

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(name.clone(), record.clone());
        }

        // Persist if DB available
        if let Some(db) = &self.db {
            let value = serde_json::to_vec(&record)
                .map_err(MeshError::Serialization)?;
            db.insert(name.as_bytes(), value)
                .map_err(|e| MeshError::Storage(format!("Failed to store name: {}", e)))?;
            db.flush()
                .map_err(|e| MeshError::Storage(format!("Failed to flush DB: {}", e)))?;
        }

        Ok(())
    }

    /// Resolve name to node_id
    pub async fn resolve(&self, name: &str) -> Option<String> {
        let cache = self.cache.read().await;
        cache.get(name).map(|r| r.node_id.clone())
    }

    /// List all names
    pub async fn list(&self) -> Vec<NameRecord> {
        let cache = self.cache.read().await;
        cache.values().cloned().collect()
    }

    /// Delete a name
    pub async fn delete(&self, name: &str) -> Result<()> {
        {
            let mut cache = self.cache.write().await;
            cache.remove(name);
        }

        if let Some(db) = &self.db {
            db.remove(name.as_bytes())
                .map_err(|e| MeshError::Storage(format!("Failed to delete name: {}", e)))?;
        }

        Ok(())
    }
}

impl Default for NameRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_register_and_resolve() {
        let registry = NameRegistry::new();

        registry
            .register("alice".to_string(), "Qm7xRJ...".to_string())
            .await
            .unwrap();

        let resolved = registry.resolve("alice").await;
        assert_eq!(resolved, Some("Qm7xRJ...".to_string()));
    }

    #[tokio::test]
    async fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let registry = NameRegistry::with_storage(temp_dir.path()).unwrap();

        registry
            .register("bob".to_string(), "Qm8xSK...".to_string())
            .await
            .unwrap();

        // Drop and reload
        drop(registry);

        let registry2 = NameRegistry::with_storage(temp_dir.path()).unwrap();
        let resolved = registry2.resolve("bob").await;
        assert_eq!(resolved, Some("Qm8xSK...".to_string()));
    }
}

