//! Onion cell format — the fundamental unit of onion routing.
//!
//! A cell carries a command byte, circuit ID, and a layered-encrypted payload.
//! Each relay node peels exactly one encryption layer before forwarding.
//!
//! Encryption: AES-256-GCM with per-hop keys derived via HKDF-SHA256 from the
//! X25519 shared secret negotiated during circuit construction.

use crate::error::{MeshError, Result};
#[allow(deprecated)]
use aes_gcm::{
    aead::{generic_array::GenericArray, Aead, KeyInit},
    Aes256Gcm, Key,
};
use serde::{Deserialize, Serialize};

/// Command byte identifying the purpose of an onion cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CellCommand {
    /// Initiate a new circuit leg (sender → first hop).
    Create = 0x01,
    /// Confirm circuit leg creation (first hop → sender).
    Created = 0x02,
    /// Forward an encrypted payload (any relay → next hop).
    Relay = 0x03,
    /// Relay payload backwards (exit → sender path).
    RelayBack = 0x04,
    /// Tear down a circuit.
    Destroy = 0x05,
}

impl CellCommand {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Create),
            0x02 => Some(Self::Created),
            0x03 => Some(Self::Relay),
            0x04 => Some(Self::RelayBack),
            0x05 => Some(Self::Destroy),
            _ => None,
        }
    }
}

/// An onion cell — the unit of communication in an onion circuit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnionCell {
    /// Circuit identifier (unique per originating sender, per session).
    pub circuit_id: u32,
    /// Command byte (see [`CellCommand`]).
    pub command: u8,
    /// Encrypted or plaintext payload, depending on hop position.
    pub payload: Vec<u8>,
}

impl OnionCell {
    pub fn new(circuit_id: u32, command: CellCommand, payload: Vec<u8>) -> Self {
        Self {
            circuit_id,
            command: command as u8,
            payload,
        }
    }

    /// Serialize to bytes (JSON for now; could be replaced with a binary codec).
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data).map_err(MeshError::Serialization)
    }
}

// ---------------------------------------------------------------------------
// Layer encryption / decryption
// ---------------------------------------------------------------------------

/// Encrypt `payload` with one onion layer using AES-256-GCM.
#[allow(deprecated)]
pub fn encrypt_layer(payload: &[u8], key: &Key<Aes256Gcm>, nonce: &[u8]) -> Result<Vec<u8>> {
    if nonce.len() != 12 {
        return Err(MeshError::Peer("Onion nonce must be 12 bytes".into()));
    }
    let cipher = Aes256Gcm::new(key);
    let nonce_arr = GenericArray::from_slice(nonce);
    cipher
        .encrypt(nonce_arr, payload)
        .map_err(|e| MeshError::Peer(format!("Onion encrypt failed: {}", e)))
}

/// Decrypt one onion layer using AES-256-GCM (peels one hop).
#[allow(deprecated)]
pub fn decrypt_layer(ciphertext: &[u8], key: &Key<Aes256Gcm>, nonce: &[u8]) -> Result<Vec<u8>> {
    if nonce.len() != 12 {
        return Err(MeshError::Peer("Onion nonce must be 12 bytes".into()));
    }
    let cipher = Aes256Gcm::new(key);
    let nonce_arr = GenericArray::from_slice(nonce);
    cipher
        .decrypt(nonce_arr, ciphertext)
        .map_err(|e| MeshError::Peer(format!("Onion decrypt failed: {}", e)))
}

// Alias for symmetric interface.
pub use decrypt_layer as peel_layer;

/// Build a fully layered onion from `plaintext`.
///
/// `layers` must be ordered **outermost → innermost** (guard → middle → exit).
/// The innermost encryption is applied first, so guard peels last.
///
/// # Example
/// ```ignore
/// let onion = build_onion(plaintext, &[(&guard_key, &guard_nonce),
///                                      (&middle_key, &middle_nonce),
///                                      (&exit_key, &exit_nonce)])?;
/// // Peel: guard first, then middle, then exit.
/// ```
pub fn build_onion(plaintext: &[u8], layers: &[(&Key<Aes256Gcm>, &[u8])]) -> Result<Vec<u8>> {
    let mut data = plaintext.to_vec();
    // Apply innermost layer first (exit), outermost last (guard).
    for (key, nonce) in layers.iter().rev() {
        data = encrypt_layer(&data, key, nonce)?;
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(deprecated)]
    use aes_gcm::aead::{AeadCore, OsRng};

    fn fresh_key_nonce() -> (Key<Aes256Gcm>, Vec<u8>) {
        #[allow(deprecated)]
        let key = Aes256Gcm::generate_key(&mut OsRng);
        #[allow(deprecated)]
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng).as_slice().to_vec();
        (key, nonce)
    }

    #[test]
    fn test_single_layer_roundtrip() {
        let (key, nonce) = fresh_key_nonce();
        let plaintext = b"single layer test";

        let enc = encrypt_layer(plaintext, &key, &nonce).unwrap();
        assert_ne!(enc.as_slice(), plaintext.as_slice());

        let dec = decrypt_layer(&enc, &key, &nonce).unwrap();
        assert_eq!(dec.as_slice(), plaintext.as_slice());
    }

    #[test]
    fn test_build_onion_3_layers_peel_in_order() {
        let (k0, n0) = fresh_key_nonce();
        let (k1, n1) = fresh_key_nonce();
        let (k2, n2) = fresh_key_nonce();

        let plaintext = b"onion routing test payload";
        let onion = build_onion(plaintext, &[(&k0, &n0), (&k1, &n1), (&k2, &n2)]).unwrap();

        // Guard peels first (outermost).
        let after_guard = peel_layer(&onion, &k0, &n0).unwrap();
        // Middle peels second.
        let after_middle = peel_layer(&after_guard, &k1, &n1).unwrap();
        // Exit peels last (innermost).
        let after_exit = peel_layer(&after_middle, &k2, &n2).unwrap();

        assert_eq!(after_exit.as_slice(), plaintext.as_slice());
    }

    #[test]
    fn test_wrong_key_fails() {
        let (k, n) = fresh_key_nonce();
        let (k_wrong, _) = fresh_key_nonce();

        let enc = encrypt_layer(b"secret", &k, &n).unwrap();
        assert!(decrypt_layer(&enc, &k_wrong, &n).is_err());
    }

    #[test]
    fn test_onion_cell_serialization() {
        let cell = OnionCell::new(42, CellCommand::Relay, b"payload".to_vec());
        let bytes = cell.to_bytes();
        let decoded = OnionCell::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.circuit_id, 42);
        assert_eq!(decoded.command, CellCommand::Relay as u8);
        assert_eq!(decoded.payload, b"payload");
    }
}
