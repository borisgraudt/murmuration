//! Forward secrecy tests (Upgrade 4).
//!
//! Verifies that X25519 ECDH produces a unique AES-256-GCM session key for every
//! connection, so compromising a long-term RSA identity key does not expose
//! past session traffic.

use murmuration::p2p::encryption::EncryptionManager;
use x25519_dalek::{EphemeralSecret, PublicKey};

/// Establish 10 independent X25519 DH sessions and verify all session keys are unique.
#[tokio::test]
async fn test_session_keys_differ_per_connection() {
    let mut keys: Vec<Vec<u8>> = Vec::new();

    for _ in 0..10 {
        // Simulate a fresh connection: both sides generate ephemeral keypairs.
        let client_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
        let server_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);

        let client_pub = PublicKey::from(&client_secret);
        let server_pub = PublicKey::from(&server_secret);

        // Both sides perform DH.
        let client_shared = client_secret.diffie_hellman(&server_pub);
        // (server_secret consumed separately — simulate symmetrically)

        // Derive AES key from shared secret via HKDF-SHA256.
        let key = EncryptionManager::derive_session_key_hkdf(client_shared.as_bytes());
        keys.push(key.as_slice().to_vec());

        // Verify client_pub is non-zero (not a degenerate key exchange).
        assert_ne!(client_pub.as_bytes(), &[0u8; 32]);
    }

    // All 10 session keys must be distinct.
    for i in 0..keys.len() {
        for j in (i + 1)..keys.len() {
            assert_ne!(
                keys[i], keys[j],
                "Sessions {} and {} produced the same AES key — forward secrecy violation!",
                i, j
            );
        }
    }
}

/// Verify that both sides of an X25519 exchange derive the identical session key.
#[tokio::test]
async fn test_both_sides_derive_same_key() {
    let alice_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
    let bob_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);

    let alice_pub = PublicKey::from(&alice_secret);
    let bob_pub = PublicKey::from(&bob_secret);

    let alice_shared = alice_secret.diffie_hellman(&bob_pub);
    let bob_shared = bob_secret.diffie_hellman(&alice_pub);

    let alice_key = EncryptionManager::derive_session_key_hkdf(alice_shared.as_bytes());
    let bob_key = EncryptionManager::derive_session_key_hkdf(bob_shared.as_bytes());

    assert_eq!(
        alice_key.as_slice(),
        bob_key.as_slice(),
        "Alice and Bob must derive the same session key from the X25519 shared secret"
    );
}

/// Verify that HKDF produces a key distinct from the raw DH output.
#[test]
fn test_hkdf_key_differs_from_raw_shared_secret() {
    // Use a known "shared secret" (in practice, this is the X25519 output).
    let shared = [0x5Au8; 32];
    let key = EncryptionManager::derive_session_key_hkdf(&shared);

    // The derived key must differ from the raw input.
    assert_ne!(key.as_slice(), &shared);
}

/// Regression: AES-256-GCM with a forward-secret key must encrypt/decrypt correctly.
#[tokio::test]
async fn test_forward_secret_key_encrypts_correctly() {
    let secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
    let peer_secret = EphemeralSecret::random_from_rng(rand::rngs::OsRng);
    let peer_pub = PublicKey::from(&peer_secret);

    let shared = secret.diffie_hellman(&peer_pub);
    let key = EncryptionManager::derive_session_key_hkdf(shared.as_bytes());

    // Generate a fresh nonce and encrypt a test message.
    let (_, nonce) = EncryptionManager::generate_session_key();
    let plaintext = b"forward secrecy test payload";

    let ciphertext = EncryptionManager::encrypt_aes(plaintext, &key, &nonce).unwrap();
    assert_ne!(ciphertext.as_slice(), plaintext.as_slice());

    let decrypted = EncryptionManager::decrypt_aes(&ciphertext, &key, &nonce).unwrap();
    assert_eq!(decrypted.as_slice(), plaintext.as_slice());
}
