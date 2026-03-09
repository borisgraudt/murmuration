//! Onion routing — sender anonymity via layered encryption across N hops.
//!
//! # Protocol overview
//!
//! ```text
//! Sender → [Guard Node] → [Middle Node] → [Exit Node] → Recipient
//!          encrypt×3      encrypt×2        encrypt×1
//! ```
//!
//! Each hop peels one AES-256-GCM layer. Guard only knows the sender.
//! Exit only knows the recipient. No single node knows both.
//!
//! # Key exchange
//! An ephemeral X25519 keypair is negotiated per hop during circuit construction.
//! The shared secret is passed through HKDF-SHA256 to derive the AES key + nonce.
//!
//! # UCB1 integration
//! Guard node selection uses the existing UCB1 bandit router. Set `anonymity_mode = true`
//! in routing context to prefer high-uptime peers and apply latency penalties.

pub mod cell;
pub mod circuit;
pub mod relay;

pub use cell::{build_onion, decrypt_layer, encrypt_layer, peel_layer, CellCommand, OnionCell};
pub use circuit::{
    complete_hop_dh, derive_hop_keys, generate_ephemeral_keypair, Circuit, CircuitBuilder,
    CircuitHop, CircuitManager, DEFAULT_HOPS,
};
pub use relay::{CircuitLeg, OnionRelay};
