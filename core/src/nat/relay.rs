//! TURN-like relay for symmetric NAT fallback.
//!
//! Any Murmuration node can opt in as a relay (`mur start --relay`).
//! Relay nodes are announced in the DHT with a `relay=true` flag so
//! peers that fail hole punching can discover and use them.
//!
//! Protocol:
//! 1. Both NATted peers send a `JOIN` packet containing a shared 16-byte token
//!    to the relay's UDP port.
//! 2. The relay associates that token with both source addresses.
//! 3. Any subsequent packet prefixed with that token is forwarded to the other peer.
//!
//! The token is derived from the circuit ID or a pre-shared random value
//! exchanged via the mesh before the relay is needed.

use crate::error::{MeshError, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// Magic prefix that identifies a relay JOIN packet.
const JOIN_MAGIC: &[u8; 4] = b"ELJN";
/// Magic prefix for relay DATA packets (followed by token then payload).
const DATA_MAGIC: &[u8; 4] = b"ELDT";

/// Maximum UDP payload size accepted by the relay.
const MAX_RELAY_PAYLOAD: usize = 65_507;

/// A relay allocation: maps a token to the two peer addresses it bridges.
#[derive(Debug, Clone)]
struct RelayEntry {
    peer_a: SocketAddr,
    peer_b: Option<SocketAddr>,
}

/// A relay node that forwards UDP packets between NATted peers.
///
/// Start with [`RelayNode::bind`] then call [`RelayNode::run`] to begin
/// serving allocations.
pub struct RelayNode {
    socket: Arc<UdpSocket>,
    /// token → (peer_a, peer_b)
    allocations: Arc<RwLock<HashMap<[u8; 16], RelayEntry>>>,
}

impl RelayNode {
    /// Bind the relay to `0.0.0.0:port`.
    pub async fn bind(port: u16) -> Result<Self> {
        let addr: SocketAddr = format!("0.0.0.0:{}", port)
            .parse()
            .map_err(|_| MeshError::Config(format!("Invalid relay port: {}", port)))?;

        let socket = UdpSocket::bind(addr)
            .await
            .map_err(|e| MeshError::Peer(format!("Relay bind failed: {}", e)))?;

        Ok(Self {
            socket: Arc::new(socket),
            allocations: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Return the local address this relay is listening on.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket
            .local_addr()
            .map_err(|e| MeshError::Peer(format!("relay local_addr: {}", e)))
    }

    /// Main relay loop — receives packets and forwards them.
    ///
    /// Runs until cancelled (e.g. via a shutdown signal).
    pub async fn run(&self) -> Result<()> {
        let mut buf = vec![0u8; MAX_RELAY_PAYLOAD];

        loop {
            let (n, from) = self
                .socket
                .recv_from(&mut buf)
                .await
                .map_err(|e| MeshError::Peer(format!("Relay recv: {}", e)))?;

            if n < 4 {
                continue;
            }

            let magic: &[u8; 4] = buf[..4].try_into().unwrap();

            match magic {
                JOIN_MAGIC => {
                    if n < 20 {
                        continue; // JOIN: 4 magic + 16 token
                    }
                    let token: [u8; 16] = buf[4..20].try_into().unwrap();
                    self.handle_join(token, from).await;
                }
                DATA_MAGIC => {
                    if n < 20 {
                        continue; // DATA: 4 magic + 16 token + payload
                    }
                    let token: [u8; 16] = buf[4..20].try_into().unwrap();
                    let payload = &buf[20..n];
                    self.forward(token, from, payload).await;
                }
                _ => {} // Ignore unknown packets.
            }
        }
    }

    /// Handle a JOIN packet: register the peer under the given token.
    async fn handle_join(&self, token: [u8; 16], peer: SocketAddr) {
        let mut allocs = self.allocations.write().await;
        let entry = allocs.entry(token).or_insert(RelayEntry {
            peer_a: peer,
            peer_b: None,
        });
        if entry.peer_a != peer {
            entry.peer_b = Some(peer);
        }
    }

    /// Forward a DATA packet to the other peer in the allocation.
    async fn forward(&self, token: [u8; 16], from: SocketAddr, payload: &[u8]) {
        let allocs = self.allocations.read().await;
        if let Some(entry) = allocs.get(&token) {
            let dest = if from == entry.peer_a {
                entry.peer_b
            } else {
                Some(entry.peer_a)
            };
            if let Some(dest) = dest {
                let _ = self.socket.send_to(payload, dest).await;
            }
        }
    }
}

/// Send a relay JOIN packet to claim a token at the relay.
///
/// Call this from both peers before sending data.
pub async fn relay_join(
    socket: &UdpSocket,
    relay_addr: SocketAddr,
    token: &[u8; 16],
) -> Result<()> {
    let mut packet = Vec::with_capacity(20);
    packet.extend_from_slice(JOIN_MAGIC);
    packet.extend_from_slice(token);
    socket
        .send_to(&packet, relay_addr)
        .await
        .map_err(|e| MeshError::Peer(format!("relay JOIN send failed: {}", e)))?;
    Ok(())
}

/// Send a relay DATA packet through the relay.
///
/// Prepends the 4-byte magic and 16-byte token so the relay can forward it.
pub async fn relay_send(
    socket: &UdpSocket,
    relay_addr: SocketAddr,
    token: &[u8; 16],
    payload: &[u8],
) -> Result<()> {
    let mut packet = Vec::with_capacity(20 + payload.len());
    packet.extend_from_slice(DATA_MAGIC);
    packet.extend_from_slice(token);
    packet.extend_from_slice(payload);
    socket
        .send_to(&packet, relay_addr)
        .await
        .map_err(|e| MeshError::Peer(format!("relay DATA send failed: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    /// Spin up a relay, have two peers JOIN then exchange a message through it.
    #[tokio::test]
    async fn test_relay_forwards_data_between_peers() {
        let relay = RelayNode::bind(0).await.unwrap();
        // Relay binds to 0.0.0.0:PORT; rewrite to 127.0.0.1:PORT for loopback tests.
        let relay_addr: SocketAddr = format!("127.0.0.1:{}", relay.local_addr().unwrap().port())
            .parse()
            .unwrap();

        // Run relay in background.
        let relay_arc = Arc::new(relay);
        let relay_clone = relay_arc.clone();
        tokio::spawn(async move {
            let _ = relay_clone.run().await;
        });

        // Token shared by both peers (in production: derived from circuit ID).
        let token: [u8; 16] = [0xBE; 16];

        // Peer A.
        let sock_a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        // Peer B.
        let sock_b = UdpSocket::bind("127.0.0.1:0").await.unwrap();

        // Both peers JOIN the relay with the same token.
        relay_join(&sock_a, relay_addr, &token).await.unwrap();
        relay_join(&sock_b, relay_addr, &token).await.unwrap();

        // Give relay time to process JOIN packets.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Peer A sends data to peer B via relay.
        relay_send(&sock_a, relay_addr, &token, b"hello from A")
            .await
            .unwrap();

        // Peer B receives.
        let mut buf = [0u8; 128];
        let result = timeout(Duration::from_secs(2), sock_b.recv_from(&mut buf)).await;
        let (n, _) = result.unwrap().unwrap();
        assert_eq!(&buf[..n], b"hello from A");
    }
}
