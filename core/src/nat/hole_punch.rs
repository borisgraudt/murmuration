//! UDP hole punching for NAT traversal.
//!
//! Both peers simultaneously send UDP packets to each other's external address
//! (as discovered via STUN). Most NAT types (full cone, restricted cone,
//! port-restricted cone) will open the mapping when the first packet arrives,
//! allowing the response to pass through.
//!
//! Symmetric NAT is not supported by hole punching — the TURN relay fallback
//! in `nat::relay` is used in that case.

use crate::error::{MeshError, Result};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// Magic bytes that identify an Elysium hole-punch probe.
const PROBE_MAGIC: &[u8] = b"ELY_PUNCH\x00";
/// Magic bytes for the acknowledgement.
const PROBE_ACK: &[u8] = b"ELY_PUNCH_ACK\x00";

/// Configuration for a hole-punch attempt.
pub struct HolePunchConfig {
    /// Local address to bind the UDP socket to.
    pub local_addr: SocketAddr,
    /// Peer's external address (from STUN discovery).
    pub remote_external_addr: SocketAddr,
    /// How long to wait for the peer's probe before giving up.
    pub timeout: Duration,
    /// Number of probe packets to send (redundancy against packet loss).
    pub probe_count: u32,
}

impl Default for HolePunchConfig {
    fn default() -> Self {
        Self {
            local_addr: "0.0.0.0:0".parse().unwrap(),
            remote_external_addr: "0.0.0.0:0".parse().unwrap(),
            timeout: Duration::from_secs(5),
            probe_count: 5,
        }
    }
}

/// Outcome of a hole-punch attempt.
pub struct HolePunchResult {
    /// Whether the hole punch succeeded (both sides received each other's probe).
    pub success: bool,
    /// The bound UDP socket, ready for use, if successful.
    pub socket: Option<UdpSocket>,
}

/// Attempt UDP hole punching to reach `config.remote_external_addr`.
///
/// The caller must ensure that both peers call this function at approximately
/// the same time — coordination is done by the mesh rendezvous peer that
/// exchanged external addresses via STUN.
pub async fn punch_hole(config: HolePunchConfig) -> Result<HolePunchResult> {
    let socket = UdpSocket::bind(config.local_addr)
        .await
        .map_err(|e| MeshError::Peer(format!("Hole-punch bind failed: {}", e)))?;

    // Send probes to open the NAT mapping on our side and trigger the peer's mapping.
    for _ in 0..config.probe_count {
        let _ = socket
            .send_to(PROBE_MAGIC, config.remote_external_addr)
            .await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Wait for the peer's probe to arrive.
    let mut buf = [0u8; 64];
    let recv_result = timeout(config.timeout, socket.recv_from(&mut buf)).await;

    match recv_result {
        Ok(Ok((n, from))) => {
            if from == config.remote_external_addr && buf[..n].starts_with(PROBE_MAGIC) {
                // Send acknowledgement.
                let _ = socket.send_to(PROBE_ACK, from).await;
                Ok(HolePunchResult {
                    success: true,
                    socket: Some(socket),
                })
            } else {
                Ok(HolePunchResult {
                    success: false,
                    socket: None,
                })
            }
        }
        _ => Ok(HolePunchResult {
            success: false,
            socket: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two sockets on loopback hole-punching each other (simulates both-sides-NAT scenario).
    #[tokio::test]
    async fn test_hole_punch_loopback() {
        let addr_a: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let addr_b: SocketAddr = "127.0.0.1:0".parse().unwrap();

        // Pre-bind sockets to get concrete ports.
        let sock_a = UdpSocket::bind(addr_a).await.unwrap();
        let sock_b = UdpSocket::bind(addr_b).await.unwrap();
        let local_a = sock_a.local_addr().unwrap();
        let local_b = sock_b.local_addr().unwrap();
        drop(sock_a);
        drop(sock_b);

        let cfg_a = HolePunchConfig {
            local_addr: local_a,
            remote_external_addr: local_b,
            timeout: Duration::from_secs(2),
            probe_count: 3,
        };
        let cfg_b = HolePunchConfig {
            local_addr: local_b,
            remote_external_addr: local_a,
            timeout: Duration::from_secs(2),
            probe_count: 3,
        };

        let (res_a, res_b) = tokio::join!(punch_hole(cfg_a), punch_hole(cfg_b));

        assert!(
            res_a.unwrap().success,
            "Hole punch A should succeed on loopback"
        );
        assert!(
            res_b.unwrap().success,
            "Hole punch B should succeed on loopback"
        );
    }
}
