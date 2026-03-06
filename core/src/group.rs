/// Group messaging with shared symmetric key.
///
/// # Key derivation
/// Group key = SHA-256(group_id ‖ sorted_member_ids).
/// This is deterministic: any member who knows the group_id and member list can
/// derive the same key without a central key-distribution step.
///
/// # Limitations (known, future work)
/// - Adding/removing members requires a new group_id and key rotation.
/// - No forward secrecy within the group (use a ratchet in future iterations).
/// - No membership proof: any node that knows the group_id and member list can join.
use crate::error::{MeshError, Result};
use crate::p2p::encryption::EncryptionManager;
use aes_gcm::{
    aead::{AeadCore, OsRng},
    Aes256Gcm, Key,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Prefix byte marking a group-encrypted MeshMessage payload.
pub const GROUP_MARKER: u8 = 0x6B; // 'k' for "key"

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInfo {
    /// Unique group identifier (human-readable or UUID).
    pub group_id: String,
    /// Sorted list of member node_ids.
    pub members: Vec<String>,
}

impl GroupInfo {
    pub fn new(group_id: impl Into<String>, mut members: Vec<String>) -> Self {
        members.sort();
        members.dedup();
        Self {
            group_id: group_id.into(),
            members,
        }
    }

    /// Derive the group AES-256-GCM key deterministically from group_id + members.
    pub fn derive_key(&self) -> Key<Aes256Gcm> {
        let mut hasher = Sha256::new();
        hasher.update(b"elysium-group-v1:");
        hasher.update(self.group_id.as_bytes());
        for member in &self.members {
            hasher.update(b"|");
            hasher.update(member.as_bytes());
        }
        let hash = hasher.finalize();
        Key::<Aes256Gcm>::from(hash)
    }
}

/// Wire format for a group-encrypted message stored in `MeshMessage.data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPayload {
    pub group_id: String,
    /// AES-GCM nonce (12 bytes, base64)
    pub nonce: Vec<u8>,
    /// AES-GCM ciphertext
    pub ciphertext: Vec<u8>,
}

/// Encrypt `plaintext` for a group identified by `group`.
/// Returns bytes: `[GROUP_MARKER | JSON(GroupPayload)]`.
pub fn group_encrypt(plaintext: &[u8], group: &GroupInfo) -> Result<Vec<u8>> {
    let key = group.derive_key();
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    #[allow(deprecated)]
    let nonce_bytes = nonce.as_slice().to_vec();

    let ciphertext = EncryptionManager::encrypt_aes(plaintext, &key, &nonce_bytes)?;

    let payload = GroupPayload {
        group_id: group.group_id.clone(),
        nonce: nonce_bytes,
        ciphertext,
    };

    let json = serde_json::to_vec(&payload).map_err(MeshError::Serialization)?;
    let mut out = Vec::with_capacity(1 + json.len());
    out.push(GROUP_MARKER);
    out.extend_from_slice(&json);
    Ok(out)
}

/// Decrypt a group-encrypted payload.
/// Returns `None` if the marker is absent (not a group message).
pub fn group_decrypt(data: &[u8], group: &GroupInfo) -> Result<Option<Vec<u8>>> {
    if data.is_empty() || data[0] != GROUP_MARKER {
        return Ok(None);
    }

    let payload: GroupPayload =
        serde_json::from_slice(&data[1..]).map_err(MeshError::Serialization)?;

    if payload.group_id != group.group_id {
        return Ok(None); // Different group
    }

    let key = group.derive_key();
    let plaintext = EncryptionManager::decrypt_aes(&payload.ciphertext, &key, &payload.nonce)?;
    Ok(Some(plaintext))
}

/// Returns true if `data` is a group-encrypted payload.
pub fn is_group_message(data: &[u8]) -> bool {
    data.first() == Some(&GROUP_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_key_deterministic() {
        let g1 = GroupInfo::new("mygroup", vec!["alice".into(), "bob".into()]);
        let g2 = GroupInfo::new("mygroup", vec!["bob".into(), "alice".into()]); // different order
        assert_eq!(g1.derive_key(), g2.derive_key()); // must be identical
    }

    #[test]
    fn test_group_encrypt_decrypt_roundtrip() {
        let group = GroupInfo::new("test-group", vec!["alice".into(), "bob".into()]);
        let plaintext = b"Hello, group!";

        let encrypted = group_encrypt(plaintext, &group).unwrap();
        assert_eq!(encrypted[0], GROUP_MARKER);
        assert!(is_group_message(&encrypted));

        let decrypted = group_decrypt(&encrypted, &group).unwrap();
        assert_eq!(decrypted.as_deref(), Some(plaintext.as_slice()));
    }

    #[test]
    fn test_group_wrong_group_id_returns_none() {
        let group_a = GroupInfo::new("group-a", vec!["alice".into()]);
        let group_b = GroupInfo::new("group-b", vec!["alice".into()]);

        let encrypted = group_encrypt(b"Secret", &group_a).unwrap();
        // group_b has a different group_id — should return None, not decrypt garbage
        let result = group_decrypt(&encrypted, &group_b).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_group_wrong_key_fails() {
        // Same group_id but different members → different key → decryption error
        let group_a = GroupInfo::new("shared-id", vec!["alice".into()]);
        let group_b = GroupInfo::new("shared-id", vec!["alice".into(), "eve".into()]);

        let encrypted = group_encrypt(b"Secret", &group_a).unwrap();
        // Different key → AES-GCM authentication failure
        let result = group_decrypt(&encrypted, &group_b);
        assert!(result.is_err());
    }

    #[test]
    fn test_non_group_message_passthrough() {
        let group = GroupInfo::new("g", vec!["alice".into()]);
        let plaintext = b"plain text message";
        let result = group_decrypt(plaintext, &group).unwrap();
        assert_eq!(result, None);
    }
}
