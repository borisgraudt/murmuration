use crate::error::{MeshError, Result};
use crate::p2p::encryption::EncryptionManager;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Generate node_id from public key (base58 encoded hash)
fn derive_node_id(encryption: &EncryptionManager) -> Result<String> {
    // Get public key DER bytes
    let public_key_der = encryption.get_public_key_der()?;

    // Hash it
    let mut hasher = Sha256::new();
    hasher.update(&public_key_der);
    let hash = hasher.finalize();

    // Base58 encode (like Bitcoin addresses)
    let node_id = bs58::encode(&hash[..]).into_string();

    Ok(node_id)
}

#[derive(Clone)]
pub struct NodeIdentity {
    pub node_id: String,
    pub encryption: EncryptionManager,
    pub data_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct IdentityFileV1 {
    version: u8,
    node_id: String,
    rsa_private_key_pkcs8_b64: String,
}

fn keys_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("keys")
}

fn identity_path(data_dir: &Path) -> PathBuf {
    keys_dir(data_dir).join("identity.json")
}

pub fn load_or_create(data_dir: &Path) -> Result<NodeIdentity> {
    let data_dir = data_dir.to_path_buf();
    let keys_dir = keys_dir(&data_dir);
    fs::create_dir_all(&keys_dir).map_err(MeshError::Io)?;

    let path = identity_path(&data_dir);
    if path.exists() {
        let raw = fs::read_to_string(&path).map_err(MeshError::Io)?;
        let parsed: IdentityFileV1 =
            serde_json::from_str(&raw).map_err(MeshError::Serialization)?;
        if parsed.version != 1 {
            return Err(MeshError::Config(format!(
                "Unsupported identity file version: {}",
                parsed.version
            )));
        }

        let pkcs8 = general_purpose::STANDARD
            .decode(parsed.rsa_private_key_pkcs8_b64)
            .map_err(|e| MeshError::Config(format!("Invalid base64 in identity: {}", e)))?;

        let private_key = EncryptionManager::decode_private_key_pkcs8(&pkcs8)?;
        let encryption = EncryptionManager::from_private_key(private_key)?;

        return Ok(NodeIdentity {
            node_id: parsed.node_id,
            encryption,
            data_dir,
        });
    }

    // Create a new identity
    let encryption = EncryptionManager::new()?;
    let node_id = derive_node_id(&encryption)?;

    let private_key_pkcs8 = encryption.encode_private_key_pkcs8()?;
    let file = IdentityFileV1 {
        version: 1,
        node_id: node_id.clone(),
        rsa_private_key_pkcs8_b64: general_purpose::STANDARD.encode(private_key_pkcs8),
    };
    let json = serde_json::to_string_pretty(&file).map_err(MeshError::Serialization)?;
    fs::write(&path, json).map_err(MeshError::Io)?;

    // Best-effort file permissions (0600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }

    Ok(NodeIdentity {
        node_id,
        encryption,
        data_dir,
    })
}
