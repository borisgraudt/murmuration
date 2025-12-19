/// Test PQC handshake functionality
extern crate meshlink_core;

use meshlink_core::p2p::encryption_pqc::{is_pqc_available, PqcEncryptionManager};

#[tokio::test]
async fn test_pqc_key_generation() {
    // Skip if PQC not available
    if !is_pqc_available() {
        eprintln!("⚠️  PQC not available, skipping test");
        return;
    }

    let manager = PqcEncryptionManager::new();
    assert!(manager.is_ok(), "Should be able to create PQC manager");

    let manager = manager.unwrap();
    assert!(
        !manager.public_key.is_empty(),
        "Public key should not be empty"
    );
}

#[tokio::test]
async fn test_pqc_encapsulation() {
    // Skip if PQC not available
    if !is_pqc_available() {
        eprintln!("⚠️  PQC not available, skipping test");
        return;
    }

    // Create two managers (simulating two peers)
    let bob = PqcEncryptionManager::new().unwrap();

    // Alice encapsulates using Bob's public key
    let (shared_secret_alice, ciphertext) =
        PqcEncryptionManager::encapsulate(&bob.public_key).expect("Encapsulation should succeed");

    // Bob decapsulates using his secret key
    let shared_secret_bob = bob
        .decapsulate(&ciphertext)
        .expect("Decapsulation should succeed");

    // Shared secrets should match
    assert_eq!(
        shared_secret_alice, shared_secret_bob,
        "Shared secrets should match after encapsulation/decapsulation"
    );
    assert!(
        !shared_secret_alice.is_empty(),
        "Shared secret should not be empty"
    );
}

#[tokio::test]
async fn test_pqc_public_key_serialization() {
    // Skip if PQC not available
    if !is_pqc_available() {
        eprintln!("⚠️  PQC not available, skipping test");
        return;
    }

    let manager = PqcEncryptionManager::new().unwrap();
    let public_key_str = manager.get_public_key_string();

    assert!(
        !public_key_str.is_empty(),
        "Public key string should not be empty"
    );

    // Parse it back
    let parsed_key = PqcEncryptionManager::parse_public_key(&public_key_str)
        .expect("Should be able to parse public key");

    assert_eq!(
        manager.public_key, parsed_key,
        "Parsed public key should match original"
    );
}

#[tokio::test]
async fn test_pqc_key_exchange_roundtrip() {
    // Skip if PQC not available
    if !is_pqc_available() {
        eprintln!("⚠️  PQC not available, skipping test");
        return;
    }

    // Simulate key exchange between two peers
    let peer1 = PqcEncryptionManager::new().unwrap();

    // Peer 1 sends its public key to Peer 2
    let peer1_pub_key_str = peer1.get_public_key_string();
    let peer1_pub_key = PqcEncryptionManager::parse_public_key(&peer1_pub_key_str).unwrap();

    // Peer 2 encapsulates using Peer 1's public key
    let (shared_secret_2, ciphertext) = PqcEncryptionManager::encapsulate(&peer1_pub_key).unwrap();

    // Peer 1 decapsulates using its secret key
    let shared_secret_1 = peer1.decapsulate(&ciphertext).unwrap();

    // Both should have the same shared secret
    assert_eq!(
        shared_secret_1, shared_secret_2,
        "Both peers should derive the same shared secret"
    );
}
