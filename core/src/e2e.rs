/// End-to-end encryption for direct messages.
///
/// While hop-by-hop encryption (AES-GCM per TCP session) protects data in transit,
/// intermediate routing nodes can still read plaintext content. This module adds
/// an additional layer: DM payloads are encrypted with the *recipient's RSA public key*
/// before being placed in the MeshMessage.data field. Only the recipient can decrypt.
///
/// # Wire format
/// A DM encrypted with this module has the first byte set to `E2E_MARKER` (0xE2).
/// If the marker is absent the payload is treated as plaintext UTF-8.
///
/// # Threat model note
/// This is opportunistic E2E: it requires knowing the recipient's RSA public key.
/// The `to` field in MeshMessage remains in the clear (needed for routing).
/// For full anonymity, onion routing is required (future work).
use crate::error::{MeshError, Result};
use crate::p2p::encryption::EncryptionManager;
use base64::{engine::general_purpose, Engine as _};
use rsa::RsaPublicKey;
use serde::{Deserialize, Serialize};

/// Magic byte identifying an E2E-encrypted DM payload.
pub const E2E_MARKER: u8 = 0xE2;

/// E2E-encrypted payload stored in `MeshMessage.data`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2ePayload {
    /// RSA-OAEP-encrypted AES-256 key (256 bytes for RSA-2048)
    pub encrypted_key: Vec<u8>,
    /// AES-GCM nonce (12 bytes)
    pub nonce: Vec<u8>,
    /// AES-GCM ciphertext
    pub ciphertext: Vec<u8>,
}

/// Encrypt `plaintext` for `recipient_pubkey`.
/// Returns raw bytes: `[E2E_MARKER | msgpack/JSON of E2ePayload]`.
pub fn e2e_encrypt(plaintext: &[u8], recipient_pubkey: &RsaPublicKey) -> Result<Vec<u8>> {
    // Generate AES-256 session key
    let (aes_key, nonce) = EncryptionManager::generate_session_key();

    // Encrypt plaintext with AES-GCM
    let ciphertext = EncryptionManager::encrypt_aes(plaintext, &aes_key, &nonce)?;

    // Encrypt AES key with recipient RSA public key
    let dummy_enc = EncryptionManager::new()?; // throwaway — only used for encrypt_with_public_key
    #[allow(deprecated)]
    let encrypted_key = dummy_enc.encrypt_with_public_key(aes_key.as_slice(), recipient_pubkey)?;

    let payload = E2ePayload {
        encrypted_key,
        nonce,
        ciphertext,
    };

    let json = serde_json::to_vec(&payload).map_err(MeshError::Serialization)?;
    let mut out = Vec::with_capacity(1 + json.len());
    out.push(E2E_MARKER);
    out.extend_from_slice(&json);
    Ok(out)
}

/// Decrypt an E2E-encrypted payload produced by [`e2e_encrypt`].
/// Returns `None` if the payload does not start with `E2E_MARKER`.
pub async fn e2e_decrypt(data: &[u8], enc_mgr: &EncryptionManager) -> Result<Option<Vec<u8>>> {
    if data.is_empty() || data[0] != E2E_MARKER {
        return Ok(None); // Not E2E encrypted
    }

    let payload: E2ePayload =
        serde_json::from_slice(&data[1..]).map_err(MeshError::Serialization)?;

    // Decrypt AES key with our RSA private key
    let aes_key_bytes = enc_mgr
        .decrypt_with_private_key(&payload.encrypted_key)
        .await?;

    #[allow(deprecated)]
    let aes_key = aes_gcm::Key::<aes_gcm::Aes256Gcm>::from_slice(&aes_key_bytes);

    // Decrypt ciphertext
    let plaintext = EncryptionManager::decrypt_aes(&payload.ciphertext, aes_key, &payload.nonce)?;
    Ok(Some(plaintext))
}

/// Returns true if `data` is an E2E-encrypted payload.
pub fn is_e2e_encrypted(data: &[u8]) -> bool {
    data.first() == Some(&E2E_MARKER)
}

/// Encode a known RSA public key to base64 for storage / transmission alongside the node_id.
pub fn encode_pubkey(pubkey: &RsaPublicKey) -> Result<String> {
    use rsa::pkcs8::EncodePublicKey;
    let der = pubkey
        .to_public_key_der()
        .map_err(|e| MeshError::Peer(format!("Failed to encode public key: {}", e)))?;
    Ok(general_purpose::STANDARD.encode(der.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_e2e_roundtrip() {
        let recipient = EncryptionManager::new().unwrap();
        let pub_key_str = recipient.get_public_key_string().unwrap();
        let pub_key = EncryptionManager::parse_public_key(&pub_key_str).unwrap();

        let plaintext = b"Secret message for E2E test";
        let encrypted = e2e_encrypt(plaintext, &pub_key).unwrap();

        assert_eq!(encrypted[0], E2E_MARKER);
        assert!(is_e2e_encrypted(&encrypted));

        let decrypted = e2e_decrypt(&encrypted, &recipient).await.unwrap();
        assert_eq!(decrypted.as_deref(), Some(plaintext.as_slice()));
    }

    #[tokio::test]
    async fn test_e2e_non_encrypted_passthrough() {
        let recipient = EncryptionManager::new().unwrap();
        let plaintext = b"Hello, no E2E here";

        let result = e2e_decrypt(plaintext, &recipient).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_e2e_wrong_key_fails() {
        let recipient = EncryptionManager::new().unwrap();
        let attacker = EncryptionManager::new().unwrap();
        let pub_key_str = recipient.get_public_key_string().unwrap();
        let pub_key = EncryptionManager::parse_public_key(&pub_key_str).unwrap();

        let encrypted = e2e_encrypt(b"Secret", &pub_key).unwrap();
        // Decrypting with the wrong key should fail
        let result = e2e_decrypt(&encrypted, &attacker).await;
        assert!(result.is_err());
    }
}
