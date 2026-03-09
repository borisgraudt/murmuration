//! Traffic obfuscation transport layer.
//!
//! Wraps raw TCP connections with camouflage to defeat Deep Packet Inspection (DPI).
//! Supports TLS 1.3 camouflage (makes traffic indistinguishable from HTTPS WebSocket)
//! and obfs4 (random-looking byte stream with no statistical signature).
//!
//! # Usage
//! ```no_run
//! use meshlink_core::transport::obfs::{ObfsMode, TlsCamouflage};
//! ```
pub mod obfs;

pub use obfs::{ObfsMode, ObfsStream, TlsCamouflage};
