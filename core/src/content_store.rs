/// Content storage for Elysium mesh sites
/// Stores content using key-value pairs: ely://<node_id>/<path> -> bytes
use crate::error::{MeshError, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

/// Content store backed by sled embedded database
#[derive(Clone)]
pub struct ContentStore {
    db: Arc<sled::Db>,
}

impl ContentStore {
    /// Create a new content store in the given data directory
    pub fn new(data_dir: &Path) -> Result<Self> {
        let db_path = data_dir.join("content.db");
        debug!("Opening content store at {:?}", db_path);
        
        let db = sled::open(&db_path).map_err(|e| {
            MeshError::Storage(format!("Failed to open content store: {}", e))
        })?;
        
        info!("Content store initialized at {:?}", db_path);
        Ok(Self { db: Arc::new(db) })
    }

    /// Store content at a specific path
    /// Path format: ely://<node_id>/<path> or just <path> (node_id prepended automatically)
    pub fn put(&self, path: &str, content: Vec<u8>) -> Result<()> {
        debug!("Storing content at {}: {} bytes", path, content.len());
        
        self.db
            .insert(path.as_bytes(), content)
            .map_err(|e| MeshError::Storage(format!("Failed to store content: {}", e)))?;
        
        self.db
            .flush()
            .map_err(|e| MeshError::Storage(format!("Failed to flush content store: {}", e)))?;
        
        Ok(())
    }

    /// Retrieve content from a specific path
    pub fn get(&self, path: &str) -> Result<Option<Vec<u8>>> {
        debug!("Fetching content from {}", path);
        
        match self.db.get(path.as_bytes()) {
            Ok(Some(value)) => Ok(Some(value.to_vec())),
            Ok(None) => Ok(None),
            Err(e) => Err(MeshError::Storage(format!("Failed to fetch content: {}", e))),
        }
    }

    /// Delete content at a specific path
    pub fn delete(&self, path: &str) -> Result<()> {
        debug!("Deleting content at {}", path);
        
        self.db
            .remove(path.as_bytes())
            .map_err(|e| MeshError::Storage(format!("Failed to delete content: {}", e)))?;
        
        Ok(())
    }

    /// List all stored paths (optionally with a prefix filter)
    pub fn list(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        debug!("Listing content with prefix: {:?}", prefix);
        
        let iter = if let Some(p) = prefix {
            self.db.scan_prefix(p.as_bytes())
        } else {
            self.db.iter()
        };
        
        let mut paths = Vec::new();
        for entry in iter {
            match entry {
                Ok((key, _)) => {
                    if let Ok(path) = String::from_utf8(key.to_vec()) {
                        paths.push(path);
                    }
                }
                Err(e) => {
                    return Err(MeshError::Storage(format!("Failed to list content: {}", e)))
                }
            }
        }
        
        Ok(paths)
    }

    /// Get total number of stored items
    pub fn count(&self) -> Result<usize> {
        Ok(self.db.len())
    }

    /// Get total size of stored content (approximate)
    pub fn size_bytes(&self) -> Result<u64> {
        self.db
            .size_on_disk()
            .map_err(|e| MeshError::Storage(format!("Failed to get store size: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_content_store_basic() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::new(temp_dir.path()).unwrap();

        // Put and get
        let content = b"Hello, Elysium!".to_vec();
        store.put("test/page.html", content.clone()).unwrap();
        
        let retrieved = store.get("test/page.html").unwrap();
        assert_eq!(retrieved, Some(content));

        // Not found
        let missing = store.get("nonexistent").unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn test_content_store_list() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::new(temp_dir.path()).unwrap();

        store.put("site1/index.html", b"Site 1".to_vec()).unwrap();
        store.put("site1/about.html", b"About 1".to_vec()).unwrap();
        store.put("site2/index.html", b"Site 2".to_vec()).unwrap();

        let all = store.list(None).unwrap();
        assert_eq!(all.len(), 3);

        let site1 = store.list(Some("site1/")).unwrap();
        assert_eq!(site1.len(), 2);
    }

    #[test]
    fn test_content_store_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::new(temp_dir.path()).unwrap();

        store.put("temp.txt", b"Temp".to_vec()).unwrap();
        assert!(store.get("temp.txt").unwrap().is_some());

        store.delete("temp.txt").unwrap();
        assert!(store.get("temp.txt").unwrap().is_none());
    }
}





