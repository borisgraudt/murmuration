/// Contact storage — persists contacts in sled DB
use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub node_id: String,
    pub display_name: String,
    pub alias: Option<String>,
    pub added_at: String, // RFC3339
}

pub struct ContactStore {
    db: sled::Db,
}

impl ContactStore {
    pub fn new(data_dir: &Path) -> Result<Self> {
        let db = sled::open(data_dir.join("contacts.db"))
            .map_err(|e| MeshError::Storage(format!("contacts DB: {}", e)))?;
        Ok(Self { db })
    }

    pub fn add_contact(&self, c: &Contact) -> Result<()> {
        let val = serde_json::to_vec(c).map_err(MeshError::Serialization)?;
        self.db
            .insert(c.node_id.as_bytes(), val)
            .map_err(|e| MeshError::Storage(format!("add_contact: {}", e)))?;
        Ok(())
    }

    pub fn get_contacts(&self) -> Result<Vec<Contact>> {
        let mut out = Vec::new();
        for entry in self.db.iter().flatten() {
            let (_, val) = entry;
            if let Ok(c) = serde_json::from_slice::<Contact>(&val) {
                out.push(c);
            }
        }
        Ok(out)
    }

    pub fn get_contact(&self, node_id: &str) -> Result<Option<Contact>> {
        match self.db.get(node_id.as_bytes()).map_err(|e| MeshError::Storage(format!("get_contact: {}", e)))? {
            Some(val) => {
                let c = serde_json::from_slice::<Contact>(&val).map_err(MeshError::Serialization)?;
                Ok(Some(c))
            }
            None => Ok(None),
        }
    }

    pub fn remove_contact(&self, node_id: &str) -> Result<bool> {
        let removed = self.db
            .remove(node_id.as_bytes())
            .map_err(|e| MeshError::Storage(format!("remove_contact: {}", e)))?;
        Ok(removed.is_some())
    }
}

impl Clone for ContactStore {
    fn clone(&self) -> Self {
        Self { db: self.db.clone() }
    }
}
