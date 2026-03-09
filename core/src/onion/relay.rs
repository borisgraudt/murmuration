//! Onion relay — intermediate node logic for peeling and forwarding cells.
//!
//! Each relay node maintains a table of active circuit legs. When a cell arrives,
//! the relay peels one encryption layer (using the per-hop session key negotiated
//! during circuit construction) and forwards the inner cell to the next hop.
//!
//! Exit nodes (last hop) deliver the fully-decrypted payload to the recipient
//! instead of forwarding further.

use crate::error::{MeshError, Result};
use crate::onion::cell::{peel_layer, CellCommand, OnionCell};
#[allow(deprecated)]
use aes_gcm::{Aes256Gcm, Key};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// State for one leg of an active circuit at a relay node.
pub struct CircuitLeg {
    /// AES-256-GCM key for decrypting the incoming layer (from circuit construction).
    pub decrypt_key: Key<Aes256Gcm>,
    /// AES-GCM nonce for this leg (12 bytes).
    pub decrypt_nonce: Vec<u8>,
    /// Node ID of the next hop to forward the peeled cell to.
    /// `None` if this node is the exit node (deliver to recipient instead).
    pub next_hop: Option<String>,
}

impl std::fmt::Debug for CircuitLeg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitLeg")
            .field("next_hop", &self.next_hop)
            .finish()
    }
}

/// Relay manager — tracks active circuit legs and processes incoming cells.
///
/// Thread-safe: wraps state in `Arc<RwLock<_>>` for use across async tasks.
#[derive(Clone)]
pub struct OnionRelay {
    /// circuit_id → leg configuration
    circuits: Arc<RwLock<HashMap<u32, CircuitLeg>>>,
}

impl OnionRelay {
    pub fn new() -> Self {
        Self {
            circuits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new circuit leg at this relay node.
    ///
    /// Called during circuit construction when the sender sends a CREATE cell.
    pub async fn register_circuit(&self, circuit_id: u32, leg: CircuitLeg) {
        self.circuits.write().await.insert(circuit_id, leg);
    }

    /// Process an incoming onion cell.
    ///
    /// Returns `(next_hop, peeled_cell)` where `next_hop` is the node ID to
    /// forward the peeled cell to, or `None` if this is the exit node.
    ///
    /// Returns `Err` if the circuit is unknown or decryption fails.
    pub async fn process_cell(&self, cell: &OnionCell) -> Result<(Option<String>, OnionCell)> {
        let circuits = self.circuits.read().await;
        let leg = circuits.get(&cell.circuit_id).ok_or_else(|| {
            MeshError::Peer(format!("Onion relay: unknown circuit {}", cell.circuit_id))
        })?;

        // Peel one encryption layer.
        let inner = peel_layer(&cell.payload, &leg.decrypt_key, &leg.decrypt_nonce)?;

        let forwarded = OnionCell::new(cell.circuit_id, CellCommand::Relay, inner);
        Ok((leg.next_hop.clone(), forwarded))
    }

    /// Destroy a circuit (called when a DESTROY cell arrives or on timeout).
    pub async fn destroy_circuit(&self, circuit_id: u32) {
        self.circuits.write().await.remove(&circuit_id);
    }

    /// Number of active circuit legs at this relay.
    pub async fn active_circuit_count(&self) -> usize {
        self.circuits.read().await.len()
    }
}

impl Default for OnionRelay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onion::cell::encrypt_layer;
    use crate::onion::circuit::derive_hop_keys;
    #[allow(deprecated)]
    use aes_gcm::{aead::OsRng, AeadCore, Aes256Gcm, KeyInit};

    fn make_leg() -> (CircuitLeg, Key<Aes256Gcm>, Vec<u8>) {
        #[allow(deprecated)]
        let key = Aes256Gcm::generate_key(&mut OsRng);
        #[allow(deprecated)]
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng).as_slice().to_vec();
        let leg = CircuitLeg {
            decrypt_key: key.clone(),
            decrypt_nonce: nonce.clone(),
            next_hop: Some("next_node".to_string()),
        };
        (leg, key, nonce)
    }

    #[tokio::test]
    async fn test_relay_peels_one_layer() {
        let (leg, key, nonce) = make_leg();
        let inner_payload = b"inner payload after peeling";

        // Simulate the encrypted cell that arrives at this relay.
        let encrypted = encrypt_layer(inner_payload, &key, &nonce).unwrap();
        let cell = OnionCell::new(1, CellCommand::Relay, encrypted);

        let relay = OnionRelay::new();
        relay.register_circuit(1, leg).await;

        let (next_hop, peeled) = relay.process_cell(&cell).await.unwrap();
        assert_eq!(next_hop.as_deref(), Some("next_node"));
        assert_eq!(peeled.payload.as_slice(), inner_payload.as_slice());
    }

    #[tokio::test]
    async fn test_exit_relay_has_no_next_hop() {
        let (key, nonce) = {
            #[allow(deprecated)]
            let k = Aes256Gcm::generate_key(&mut OsRng);
            #[allow(deprecated)]
            let n = Aes256Gcm::generate_nonce(&mut OsRng).as_slice().to_vec();
            (k, n)
        };
        let leg = CircuitLeg {
            decrypt_key: key.clone(),
            decrypt_nonce: nonce.clone(),
            next_hop: None, // exit node
        };

        let payload = b"final plaintext";
        let encrypted = encrypt_layer(payload, &key, &nonce).unwrap();
        let cell = OnionCell::new(2, CellCommand::Relay, encrypted);

        let relay = OnionRelay::new();
        relay.register_circuit(2, leg).await;

        let (next_hop, peeled) = relay.process_cell(&cell).await.unwrap();
        assert!(next_hop.is_none(), "Exit node has no next hop");
        assert_eq!(peeled.payload.as_slice(), payload.as_slice());
    }

    #[tokio::test]
    async fn test_unknown_circuit_returns_error() {
        let relay = OnionRelay::new();
        let cell = OnionCell::new(999, CellCommand::Relay, vec![]);
        assert!(relay.process_cell(&cell).await.is_err());
    }

    #[tokio::test]
    async fn test_destroy_circuit() {
        let (leg, _key, _nonce) = make_leg();
        let relay = OnionRelay::new();
        relay.register_circuit(5, leg).await;
        assert_eq!(relay.active_circuit_count().await, 1);

        relay.destroy_circuit(5).await;
        assert_eq!(relay.active_circuit_count().await, 0);
    }

    /// End-to-end test: 3-relay chain where each relay peels one layer,
    /// and the final plaintext matches the original message.
    #[tokio::test]
    async fn test_3hop_relay_chain() {
        let circuit_id = 77u32;
        let secrets: [[u8; 32]; 3] = [[0x11; 32], [0x22; 32], [0x33; 32]];

        // Build circuit (as the sender would).
        let circuit = crate::onion::circuit::CircuitBuilder::build_from_shared_secrets(
            circuit_id,
            vec![
                ("guard".into(), secrets[0]),
                ("middle".into(), secrets[1]),
                ("exit".into(), secrets[2]),
            ],
            "recipient".into(),
        )
        .unwrap();

        let plaintext = b"secret message for recipient";
        let onion = circuit.wrap(plaintext).unwrap();

        // Set up relays.
        let guard_relay = OnionRelay::new();
        let middle_relay = OnionRelay::new();
        let exit_relay = OnionRelay::new();

        let (gk, gn) = derive_hop_keys(&secrets[0], circuit_id, 0);
        let (mk, mn) = derive_hop_keys(&secrets[1], circuit_id, 1);
        let (ek, en) = derive_hop_keys(&secrets[2], circuit_id, 2);

        guard_relay
            .register_circuit(
                circuit_id,
                CircuitLeg {
                    decrypt_key: gk,
                    decrypt_nonce: gn,
                    next_hop: Some("middle".into()),
                },
            )
            .await;
        middle_relay
            .register_circuit(
                circuit_id,
                CircuitLeg {
                    decrypt_key: mk,
                    decrypt_nonce: mn,
                    next_hop: Some("exit".into()),
                },
            )
            .await;
        exit_relay
            .register_circuit(
                circuit_id,
                CircuitLeg {
                    decrypt_key: ek,
                    decrypt_nonce: en,
                    next_hop: None, // exit
                },
            )
            .await;

        // Simulate cell traversal through the chain.
        let incoming = OnionCell::new(circuit_id, CellCommand::Relay, onion);
        let (next1, cell1) = guard_relay.process_cell(&incoming).await.unwrap();
        assert_eq!(next1.as_deref(), Some("middle"));

        let (next2, cell2) = middle_relay.process_cell(&cell1).await.unwrap();
        assert_eq!(next2.as_deref(), Some("exit"));

        let (next3, cell3) = exit_relay.process_cell(&cell2).await.unwrap();
        assert!(next3.is_none());
        assert_eq!(cell3.payload.as_slice(), plaintext.as_slice());
    }
}
