//! NAT traversal — STUN discovery, UDP hole punching, and relay fallback.
//!
//! # Usage flow
//!
//! 1. On startup, call [`stun::discover_external_addr`] to learn your public IP:port.
//! 2. Announce the external address to the mesh (via the peer discovery protocol).
//! 3. When connecting to a NATted peer, attempt [`hole_punch::punch_hole`].
//! 4. If hole punching fails (symmetric NAT), fall back to [`relay::RelayNode`].

pub mod hole_punch;
pub mod relay;
pub mod stun;

pub use hole_punch::{punch_hole, HolePunchConfig, HolePunchResult};
pub use relay::{relay_join, relay_send, RelayNode};
pub use stun::{discover_external_addr, ExternalAddr, DEFAULT_STUN_SERVERS};
