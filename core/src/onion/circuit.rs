//! Onion circuit construction and management.
//!
//! A circuit is a pre-negotiated path of N hops (default 3) through the mesh.
//! For each hop, an ephemeral X25519 key agreement is performed and the resulting
//! shared secret is passed through HKDF-SHA256 to derive the hop's AES-256-GCM key.
//!
//! # Sender anonymity
//! Each hop only learns:
//! - Who sent the cell to it (previous hop)
//! - Who to forward it to (next hop)
//!
//! No single node knows both the sender and the recipient.
//!
//! # Integration with UCB1 routing
//! Guard node selection reuses the existing [`crate::ai::router::Router`] UCB1
//! implementation. Pass `anonymity_mode = true` in the routing context to prefer
//! high-uptime peers and penalise known-slow hops.

use crate::error::{MeshError, Result};
use crate::onion::cell::build_onion;
#[allow(deprecated)]
use aes_gcm::{Aes256Gcm, Key};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use sha2::Sha256;
use std::time::{Duration, Instant};
use x25519_dalek::{EphemeralSecret, PublicKey};

/// Default number of hops in an onion circuit.
pub const DEFAULT_HOPS: usize = 3;
/// How long a circuit lives before it must be rebuilt.
pub const CIRCUIT_LIFETIME: Duration = Duration::from_secs(600);

// ---------------------------------------------------------------------------
// Key derivation
// ---------------------------------------------------------------------------

/// Derive a per-hop AES-256-GCM key (32 bytes) and nonce (12 bytes) from a
/// 32-byte X25519 shared secret using HKDF-SHA256.
///
/// The HKDF info string encodes the circuit ID and hop index to ensure that
/// the same DH secret cannot produce the same subkeys for different hops.
pub fn derive_hop_keys(
    shared_secret: &[u8; 32],
    circuit_id: u32,
    hop_index: u8,
) -> (Key<Aes256Gcm>, Vec<u8>) {
    let info = format!("murmuration-onion-v1-circuit{}-hop{}", circuit_id, hop_index);
    let hk = Hkdf::<Sha256>::new(None, shared_secret);

    let mut okm = [0u8; 44]; // 32 B key + 12 B nonce
    hk.expand(info.as_bytes(), &mut okm)
        .expect("HKDF expand: 44-byte output always fits");

    #[allow(deprecated)]
    let key = Key::<Aes256Gcm>::from(<[u8; 32]>::try_from(&okm[..32]).unwrap());
    let nonce = okm[32..44].to_vec();
    (key, nonce)
}

// ---------------------------------------------------------------------------
// Circuit hop
// ---------------------------------------------------------------------------

/// One hop in an onion circuit, holding the per-hop session key.
pub struct CircuitHop {
    /// Peer node ID of this relay.
    pub node_id: String,
    /// AES-256-GCM key derived from X25519 ECDH with this hop.
    pub session_key: Key<Aes256Gcm>,
    /// AES-GCM nonce for this hop (12 bytes).
    pub nonce: Vec<u8>,
}

impl std::fmt::Debug for CircuitHop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitHop")
            .field("node_id", &self.node_id)
            .field("nonce_len", &self.nonce.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Circuit
// ---------------------------------------------------------------------------

/// An active onion circuit from the local node to a destination.
#[derive(Debug)]
pub struct Circuit {
    /// Unique circuit identifier (random u32 per session).
    pub id: u32,
    /// Ordered hops: index 0 = guard (first hop), last = exit.
    pub hops: Vec<CircuitHop>,
    /// When this circuit was established (for expiry checking).
    pub created_at: Instant,
    /// Destination node ID (known only to the exit hop).
    pub destination: String,
}

impl Circuit {
    /// Create a circuit from pre-negotiated hops.
    pub fn new(id: u32, hops: Vec<CircuitHop>, destination: String) -> Self {
        Self {
            id,
            hops,
            created_at: Instant::now(),
            destination,
        }
    }

    /// Returns `true` if the circuit has exceeded [`CIRCUIT_LIFETIME`].
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > CIRCUIT_LIFETIME
    }

    /// Wrap `plaintext` in layered onion encryption for all hops.
    ///
    /// The outermost layer belongs to `hops[0]` (guard node).
    pub fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let layer_refs: Vec<(&Key<Aes256Gcm>, &[u8])> = self
            .hops
            .iter()
            .map(|h| (&h.session_key, h.nonce.as_slice()))
            .collect();
        build_onion(plaintext, &layer_refs)
    }

    /// Number of hops in this circuit.
    pub fn hop_count(&self) -> usize {
        self.hops.len()
    }
}

// ---------------------------------------------------------------------------
// Circuit builder
// ---------------------------------------------------------------------------

/// Constructs circuits from pre-negotiated shared secrets.
///
/// In production, shared secrets are obtained by performing X25519 ECDH with
/// each relay's ephemeral public key, exchanged during circuit creation.
/// This builder accepts them directly to keep the API testable without a running mesh.
pub struct CircuitBuilder;

impl CircuitBuilder {
    /// Build a circuit given a list of `(node_id, x25519_shared_secret)` pairs.
    ///
    /// Hops must be ordered guard → middle → exit.
    pub fn build_from_shared_secrets(
        circuit_id: u32,
        hops: Vec<(String, [u8; 32])>,
        destination: String,
    ) -> Result<Circuit> {
        if hops.is_empty() {
            return Err(MeshError::Peer("Circuit must have at least one hop".into()));
        }

        let circuit_hops: Vec<CircuitHop> = hops
            .into_iter()
            .enumerate()
            .map(|(i, (node_id, shared_secret))| {
                let (session_key, nonce) = derive_hop_keys(&shared_secret, circuit_id, i as u8);
                CircuitHop {
                    node_id,
                    session_key,
                    nonce,
                }
            })
            .collect();

        Ok(Circuit::new(circuit_id, circuit_hops, destination))
    }
}

// ---------------------------------------------------------------------------
// Ephemeral key generation (used during circuit creation handshake)
// ---------------------------------------------------------------------------

/// Generate a fresh X25519 ephemeral keypair for circuit creation.
///
/// Returns `(secret, public_key)`. The secret must be consumed by
/// [`complete_hop_dh`] after receiving the relay's public key.
pub fn generate_ephemeral_keypair() -> (EphemeralSecret, PublicKey) {
    let secret = EphemeralSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (secret, public)
}

/// Complete the X25519 DH for one hop and derive its session keys.
///
/// Call this after receiving the relay's ephemeral public key.
pub fn complete_hop_dh(
    our_secret: EphemeralSecret,
    relay_pubkey: PublicKey,
    circuit_id: u32,
    hop_index: u8,
) -> (Key<Aes256Gcm>, Vec<u8>) {
    let shared = our_secret.diffie_hellman(&relay_pubkey);
    derive_hop_keys(shared.as_bytes(), circuit_id, hop_index)
}

// ---------------------------------------------------------------------------
// Circuit manager (tracks live circuits)
// ---------------------------------------------------------------------------

/// Manages the set of active outbound circuits for a node.
pub struct CircuitManager {
    circuits: Vec<Circuit>,
    next_id: u32,
}

impl CircuitManager {
    pub fn new() -> Self {
        Self {
            circuits: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a newly-built circuit and return its ID.
    pub fn add(&mut self, circuit: Circuit) -> u32 {
        let id = circuit.id;
        self.circuits.push(circuit);
        id
    }

    /// Get a reference to the circuit with the given ID.
    pub fn get(&self, id: u32) -> Option<&Circuit> {
        self.circuits.iter().find(|c| c.id == id)
    }

    /// Remove expired circuits.
    pub fn prune_expired(&mut self) {
        self.circuits.retain(|c| !c.is_expired());
    }

    /// Generate a fresh circuit ID.
    pub fn next_circuit_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }
}

impl Default for CircuitManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onion::cell::peel_layer;

    #[test]
    fn test_circuit_wrap_and_peel_3hops() {
        let circuit_id = 42u32;
        let shared = [
            [0x11u8; 32], // guard
            [0x22u8; 32], // middle
            [0x33u8; 32], // exit
        ];

        let circuit = CircuitBuilder::build_from_shared_secrets(
            circuit_id,
            vec![
                ("guard".into(), shared[0]),
                ("middle".into(), shared[1]),
                ("exit".into(), shared[2]),
            ],
            "recipient".into(),
        )
        .unwrap();

        let plaintext = b"secret message for the recipient";
        let onion = circuit.wrap(plaintext).unwrap();

        // Simulate each relay peeling one layer.
        let (guard_key, guard_nonce) = derive_hop_keys(&shared[0], circuit_id, 0);
        let (middle_key, middle_nonce) = derive_hop_keys(&shared[1], circuit_id, 1);
        let (exit_key, exit_nonce) = derive_hop_keys(&shared[2], circuit_id, 2);

        let after_guard = peel_layer(&onion, &guard_key, &guard_nonce).unwrap();
        let after_middle = peel_layer(&after_guard, &middle_key, &middle_nonce).unwrap();
        let after_exit = peel_layer(&after_middle, &exit_key, &exit_nonce).unwrap();

        assert_eq!(after_exit.as_slice(), plaintext.as_slice());
    }

    #[test]
    fn test_circuit_is_not_immediately_expired() {
        let c = CircuitBuilder::build_from_shared_secrets(
            1,
            vec![("hop".into(), [0u8; 32])],
            "dest".into(),
        )
        .unwrap();
        assert!(!c.is_expired());
    }

    #[test]
    fn test_ephemeral_dh_both_sides_derive_same_key() {
        let (sec_a, pub_a) = generate_ephemeral_keypair();
        let (sec_b, pub_b) = generate_ephemeral_keypair();

        let (key_a, nonce_a) = complete_hop_dh(sec_a, pub_b, 7, 0);
        let (key_b, nonce_b) = complete_hop_dh(sec_b, pub_a, 7, 0);

        // Both sides must derive identical key material from the same DH shared secret.
        assert_eq!(key_a.as_slice(), key_b.as_slice());
        assert_eq!(nonce_a, nonce_b);
    }

    #[test]
    fn test_different_circuit_ids_produce_different_keys() {
        let secret = [0xAAu8; 32];
        let (key_1, nonce_1) = derive_hop_keys(&secret, 1, 0);
        let (key_2, nonce_2) = derive_hop_keys(&secret, 2, 0);

        assert_ne!(key_1.as_slice(), key_2.as_slice());
        assert_ne!(nonce_1, nonce_2);
    }

    #[test]
    fn test_circuit_manager_lifecycle() {
        let mut mgr = CircuitManager::new();
        let id = mgr.next_circuit_id();
        let c = CircuitBuilder::build_from_shared_secrets(
            id,
            vec![("hop".into(), [0u8; 32])],
            "dest".into(),
        )
        .unwrap();
        mgr.add(c);
        assert!(mgr.get(id).is_some());
        mgr.prune_expired();
        assert!(mgr.get(id).is_some()); // not yet expired
    }

    #[test]
    fn test_guard_cannot_see_destination() {
        // The guard node receives the outermost layer — after peeling it gets
        // an opaque blob that looks like random bytes. It has no way to determine
        // the final destination. This test verifies that the guard's plaintext
        // (after peeling) is NOT the original plaintext.
        let circuit_id = 99u32;
        let shared = [[0xAAu8; 32], [0xBBu8; 32], [0xCCu8; 32]];

        let circuit = CircuitBuilder::build_from_shared_secrets(
            circuit_id,
            vec![
                ("guard".into(), shared[0]),
                ("middle".into(), shared[1]),
                ("exit".into(), shared[2]),
            ],
            "recipient".into(),
        )
        .unwrap();

        let plaintext = b"destination:recipient\x00payload:hello";
        let onion = circuit.wrap(plaintext).unwrap();

        let (guard_key, guard_nonce) = derive_hop_keys(&shared[0], circuit_id, 0);
        let guard_sees = peel_layer(&onion, &guard_key, &guard_nonce).unwrap();

        // Guard's view is encrypted blob — NOT the plaintext.
        assert_ne!(guard_sees.as_slice(), plaintext.as_slice());
        // And it does not contain the plaintext as a substring.
        assert!(
            !guard_sees
                .windows(plaintext.len())
                .any(|w| w == plaintext.as_slice()),
            "Guard node must not see the plaintext destination"
        );
    }
}
