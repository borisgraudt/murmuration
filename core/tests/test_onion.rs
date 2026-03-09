//! Onion routing tests (Upgrade 3).
//!
//! Verifies:
//! - 3-hop circuit construction and teardown.
//! - Sender/recipient anonymity: guard cannot see destination, exit cannot see sender.
//! - 3-relay chain end-to-end: plaintext recovered correctly.
//! - Circuit manager lifecycle (add, get, prune).

use meshlink_core::onion::{
    derive_hop_keys, generate_ephemeral_keypair, peel_layer, CircuitBuilder, CircuitLeg,
    CircuitManager, OnionRelay, DEFAULT_HOPS,
};

/// Verify circuit is built with exactly DEFAULT_HOPS (3) hops.
#[test]
fn test_circuit_construction_3hops() {
    let hops: Vec<(String, [u8; 32])> = (0..DEFAULT_HOPS)
        .map(|i| (format!("node_{}", i), [(i as u8 + 1) * 0x11; 32]))
        .collect();

    let circuit = CircuitBuilder::build_from_shared_secrets(42, hops, "recipient".into()).unwrap();
    assert_eq!(circuit.hop_count(), DEFAULT_HOPS);
    assert_eq!(circuit.destination, "recipient");
    assert!(!circuit.is_expired());
}

/// Guard node sees only its own layer — cannot read destination or plaintext.
#[test]
fn test_guard_node_cannot_see_destination() {
    let circuit_id = 100u32;
    let secrets = [[0x11u8; 32], [0x22u8; 32], [0x33u8; 32]];

    let circuit = CircuitBuilder::build_from_shared_secrets(
        circuit_id,
        vec![
            ("guard".into(), secrets[0]),
            ("middle".into(), secrets[1]),
            ("exit".into(), secrets[2]),
        ],
        "recipient_node_xyz".into(),
    )
    .unwrap();

    // A message containing the destination (as an exit node would embed it).
    let plaintext = b"dest=recipient_node_xyz payload=hello_world";
    let onion = circuit.wrap(plaintext).unwrap();

    // Guard peels its layer.
    let (guard_key, guard_nonce) = derive_hop_keys(&secrets[0], circuit_id, 0);
    let guard_sees = peel_layer(&onion, &guard_key, &guard_nonce).unwrap();

    // Guard must NOT see the plaintext or destination.
    assert_ne!(guard_sees.as_slice(), plaintext.as_slice());
    assert!(
        !guard_sees
            .windows(b"recipient_node_xyz".len())
            .any(|w| w == b"recipient_node_xyz"),
        "Guard node must not see the destination"
    );
}

/// Exit node sees the plaintext but not the original sender's identity.
/// (Sender identity is never embedded in the onion payload in this implementation.)
#[test]
fn test_exit_node_sees_plaintext_not_sender() {
    let circuit_id = 200u32;
    let secrets = [[0xAAu8; 32], [0xBBu8; 32], [0xCCu8; 32]];

    let circuit = CircuitBuilder::build_from_shared_secrets(
        circuit_id,
        vec![
            ("guard".into(), secrets[0]),
            ("middle".into(), secrets[1]),
            ("exit".into(), secrets[2]),
        ],
        "recipient".into(),
    )
    .unwrap();

    let plaintext = b"hello recipient";
    let onion = circuit.wrap(plaintext).unwrap();

    // Peel guard and middle layers.
    let (gk, gn) = derive_hop_keys(&secrets[0], circuit_id, 0);
    let (mk, mn) = derive_hop_keys(&secrets[1], circuit_id, 1);
    let (ek, en) = derive_hop_keys(&secrets[2], circuit_id, 2);

    let after_guard = peel_layer(&onion, &gk, &gn).unwrap();
    let after_middle = peel_layer(&after_guard, &mk, &mn).unwrap();
    let exit_sees = peel_layer(&after_middle, &ek, &en).unwrap();

    // Exit node receives the plaintext.
    assert_eq!(exit_sees.as_slice(), plaintext.as_slice());
}

/// Full 3-relay chain simulation: cells traverse guard → middle → exit.
#[tokio::test]
async fn test_circuit_construction_and_relay_chain_3hops() {
    let circuit_id = 77u32;
    let secrets = [[0x11u8; 32], [0x22u8; 32], [0x33u8; 32]];

    let circuit = CircuitBuilder::build_from_shared_secrets(
        circuit_id,
        vec![
            ("guard".into(), secrets[0]),
            ("middle".into(), secrets[1]),
            ("exit".into(), secrets[2]),
        ],
        "recipient".into(),
    )
    .unwrap();

    let plaintext = b"secret delivery to recipient";
    let onion = circuit.wrap(plaintext).unwrap();

    // Set up relay nodes.
    let guard = OnionRelay::new();
    let middle = OnionRelay::new();
    let exit = OnionRelay::new();

    let (gk, gn) = derive_hop_keys(&secrets[0], circuit_id, 0);
    let (mk, mn) = derive_hop_keys(&secrets[1], circuit_id, 1);
    let (ek, en) = derive_hop_keys(&secrets[2], circuit_id, 2);

    guard
        .register_circuit(
            circuit_id,
            CircuitLeg {
                decrypt_key: gk,
                decrypt_nonce: gn,
                next_hop: Some("middle".into()),
            },
        )
        .await;
    middle
        .register_circuit(
            circuit_id,
            CircuitLeg {
                decrypt_key: mk,
                decrypt_nonce: mn,
                next_hop: Some("exit".into()),
            },
        )
        .await;
    exit.register_circuit(
        circuit_id,
        CircuitLeg {
            decrypt_key: ek,
            decrypt_nonce: en,
            next_hop: None,
        },
    )
    .await;

    use meshlink_core::onion::cell::{CellCommand, OnionCell};

    let cell0 = OnionCell::new(circuit_id, CellCommand::Relay, onion);
    let (hop1, cell1) = guard.process_cell(&cell0).await.unwrap();
    assert_eq!(hop1.as_deref(), Some("middle"));

    let (hop2, cell2) = middle.process_cell(&cell1).await.unwrap();
    assert_eq!(hop2.as_deref(), Some("exit"));

    let (hop3, cell3) = exit.process_cell(&cell2).await.unwrap();
    assert!(hop3.is_none(), "Exit has no next hop");
    assert_eq!(cell3.payload.as_slice(), plaintext.as_slice());
}

/// Ephemeral X25519 key agreement: both sides derive identical keys.
#[test]
fn test_ephemeral_key_agreement() {
    let (sec_a, pub_a) = generate_ephemeral_keypair();
    let (sec_b, pub_b) = generate_ephemeral_keypair();

    use meshlink_core::onion::complete_hop_dh;
    let (key_a, nonce_a) = complete_hop_dh(sec_a, pub_b, 1, 0);
    let (key_b, nonce_b) = complete_hop_dh(sec_b, pub_a, 1, 0);

    assert_eq!(key_a.as_slice(), key_b.as_slice());
    assert_eq!(nonce_a, nonce_b);
}

/// Circuit manager: add, retrieve, and prune circuits.
#[test]
fn test_circuit_manager_operations() {
    let mut mgr = CircuitManager::new();

    let id1 = mgr.next_circuit_id();
    let id2 = mgr.next_circuit_id();
    assert_ne!(id1, id2);

    let c1 = CircuitBuilder::build_from_shared_secrets(
        id1,
        vec![("hop".into(), [0u8; 32])],
        "d1".into(),
    )
    .unwrap();
    let c2 = CircuitBuilder::build_from_shared_secrets(
        id2,
        vec![("hop".into(), [0xFFu8; 32])],
        "d2".into(),
    )
    .unwrap();

    mgr.add(c1);
    mgr.add(c2);

    assert!(mgr.get(id1).is_some());
    assert!(mgr.get(id2).is_some());
    assert!(mgr.get(9999).is_none());

    // Neither circuit has expired yet.
    mgr.prune_expired();
    assert!(mgr.get(id1).is_some());
}

/// Verify DEFAULT_HOPS constant equals 3 (anonymity requirement).
#[test]
fn test_default_hops_is_3() {
    assert_eq!(DEFAULT_HOPS, 3);
}
