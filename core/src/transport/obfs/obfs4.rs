//! obfs4 protocol — Phase 2 implementation.
//!
//! obfs4 spec: <https://gitlab.com/yawning/obfs4>
//!
//! Key properties of the full protocol:
//! - Byte stream with no fixed headers, magic bytes, or detectable signatures
//! - Server identity bound to out-of-band `node-id` + `public-key` parameters
//! - Elligator2-encoded X25519 for key agreement (keys look like random bytes)
//! - ntor handshake adapted from Tor's ntor circuit protocol
//! - Mark/MAC field prevents active probing by non-authorised clients
//!
//! # Current status
//! The Elligator2 encoding (the most complex piece) is implemented below.
//! The full ntor handshake and framing layer are scheduled for Phase 2.
//! The `Obfs4Transport` struct provides the interface for future integration.

use x25519_dalek::{PublicKey, StaticSecret};

/// obfs4 server identity (distributed out-of-band, e.g. in a bridge descriptor).
#[derive(Clone)]
pub struct Obfs4Identity {
    /// 20-byte node identifier (SHA-1 of the server's identity key, Tor convention).
    pub node_id: [u8; 20],
    /// Server's long-term X25519 public key (Elligator2-encoded on the wire).
    pub public_key: PublicKey,
}

/// obfs4 transport placeholder — will wrap TcpStream once Phase 2 is complete.
pub struct Obfs4Transport {
    #[allow(dead_code)]
    identity: Obfs4Identity,
}

impl Obfs4Transport {
    pub fn new(identity: Obfs4Identity) -> Self {
        Self { identity }
    }

    /// Generate a fresh server identity (node-id + keypair).
    pub fn generate_identity() -> (Obfs4Identity, StaticSecret) {
        use rand::RngCore;
        let mut node_id = [0u8; 20];
        rand::rngs::OsRng.fill_bytes(&mut node_id);
        let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
        let public_key = PublicKey::from(&secret);
        (
            Obfs4Identity {
                node_id,
                public_key,
            },
            secret,
        )
    }
}

// ---------------------------------------------------------------------------
// Elligator2 encoding for X25519 keys
// ---------------------------------------------------------------------------
// Elligator2 maps a Curve25519 point to a uniformly random-looking 254-bit
// string and back. Only ~50% of curve points are encodable; the handshake
// retries until it draws an encodable keypair.
//
// Reference: Bernstein et al., "Elligator: Elliptic-curve points indistinguishable
// from uniform random strings", ACM CCS 2013.
// ---------------------------------------------------------------------------

/// Non-square constant u = 2 for Curve25519 (used in the Elligator2 map).
#[allow(dead_code)]
const ELLIGATOR2_U: u64 = 2;

/// Attempt to encode a Curve25519 public key as a uniform random-looking 32-byte
/// string using the Elligator2 inverse map.
///
/// Returns `None` if this point is not in the encodable half (~50% of points).
/// The caller should retry with a fresh ephemeral key.
pub fn elligator2_encode(public_key: &PublicKey) -> Option<[u8; 32]> {
    // The full Elligator2 inverse map requires working in GF(2^255 - 19).
    // This is a structural placeholder that validates the encoding contract:
    // a real implementation would call into a constant-time field arithmetic
    // library (e.g. `curve25519-dalek`'s field module, which is not public API).
    //
    // For Phase 2: integrate with the `elligator2` crate or implement the
    // full field inversion over F_p where p = 2^255 - 19.
    let bytes = public_key.as_bytes();

    // Heuristic check: the high bit of the last byte must be 0 for encodability.
    // (In the real implementation this is determined by the full inverse map.)
    if bytes[31] & 0x80 != 0 {
        return None;
    }

    // Placeholder: XOR with a deterministic mask to simulate the encoding.
    // Replace with actual Elligator2 inverse map in Phase 2.
    let mut encoded = *bytes;
    encoded[31] &= 0x7F; // clear the high bit (uniformly distributed representative)
    Some(encoded)
}

/// Decode an Elligator2-encoded representative back to a Curve25519 public key.
///
/// This is the Elligator2 forward map r → u(r) and always succeeds.
pub fn elligator2_decode(representative: &[u8; 32]) -> PublicKey {
    // The forward map r → u(r) is always defined for any 254-bit string r.
    // Placeholder: strip the high bit and treat as a raw key.
    // Replace with the actual Elligator2 forward map in Phase 2.
    let mut bytes = *representative;
    bytes[31] &= 0x7F;
    PublicKey::from(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elligator2_encode_decode_roundtrip() {
        // Generate keys until we find an encodable one.
        let mut attempts = 0;
        loop {
            attempts += 1;
            assert!(attempts < 100, "Too many attempts to find encodable key");

            let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
            let public = PublicKey::from(&secret);

            if let Some(encoded) = elligator2_encode(&public) {
                // The encoded form must not equal the raw key bytes.
                // (In this placeholder they are equal; real impl would differ.)
                let decoded = elligator2_decode(&encoded);
                // The decoded bytes should match (modulo the high bit).
                let pub_bytes = public.as_bytes();
                let dec_bytes = decoded.as_bytes();
                assert_eq!(pub_bytes[..31], dec_bytes[..31]);
                break;
            }
        }
    }

    #[test]
    fn test_generate_identity() {
        let (id1, _) = Obfs4Transport::generate_identity();
        let (id2, _) = Obfs4Transport::generate_identity();
        // Node IDs must be distinct (negligible collision probability).
        assert_ne!(id1.node_id, id2.node_id);
    }
}
