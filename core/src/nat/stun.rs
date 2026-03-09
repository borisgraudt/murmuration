//! RFC 5389 STUN client for external address discovery.
//!
//! Sends a STUN Binding Request to one or more public STUN servers and parses
//! the XOR-MAPPED-ADDRESS attribute to discover the node's external IP:port
//! as seen by the internet — the first step of NAT traversal.
//!
//! Only IPv4 is supported in this implementation.

use crate::error::{MeshError, Result};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

// RFC 5389 constants.
const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;
const MSG_BINDING_REQUEST: u16 = 0x0001;
const MSG_BINDING_RESPONSE: u16 = 0x0101;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
const ATTR_MAPPED_ADDRESS: u16 = 0x0001;
const STUN_TIMEOUT_MS: u64 = 3_000;

/// Well-known public STUN servers (tried in order).
pub const DEFAULT_STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302",
    "stun.cloudflare.com:3478",
];

/// External address as reported by a STUN server.
#[derive(Debug, Clone)]
pub struct ExternalAddr {
    /// The external (public) socket address of this node.
    pub address: SocketAddr,
    /// Which STUN server provided the answer.
    pub stun_server: String,
}

/// Discover the node's external IP:port by querying public STUN servers.
///
/// Tries each server in [`DEFAULT_STUN_SERVERS`] in order; returns the first
/// successful result. Binds to `local_port` on `0.0.0.0` temporarily.
///
/// Returns `Err` only if **all** servers fail (network unreachable, timeout, etc.).
pub fn discover_external_addr(local_port: u16) -> Result<ExternalAddr> {
    for server in DEFAULT_STUN_SERVERS {
        match query_stun_server(server, local_port) {
            Ok(addr) => {
                return Ok(ExternalAddr {
                    address: addr,
                    stun_server: server.to_string(),
                })
            }
            Err(_) => continue,
        }
    }
    Err(MeshError::Peer("All STUN servers unreachable".to_string()))
}

/// Send a STUN Binding Request to `server` and return the mapped address.
fn query_stun_server(server: &str, local_port: u16) -> Result<SocketAddr> {
    let local: SocketAddr = format!("0.0.0.0:{}", local_port)
        .parse()
        .map_err(|_| MeshError::Config(format!("Invalid local port: {}", local_port)))?;

    let socket = UdpSocket::bind(local)
        .map_err(|e| MeshError::Peer(format!("STUN UDP bind failed: {}", e)))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(STUN_TIMEOUT_MS)))
        .map_err(|e| MeshError::Peer(format!("STUN set timeout: {}", e)))?;

    let server_addr = resolve_addr(server)?;

    // Build and send the binding request.
    let tx_id = rand_tx_id();
    let request = build_binding_request(&tx_id);

    socket
        .send_to(&request, server_addr)
        .map_err(|e| MeshError::Peer(format!("STUN send failed: {}", e)))?;

    // Receive the binding response.
    let mut buf = [0u8; 1024];
    let (n, _) = socket
        .recv_from(&mut buf)
        .map_err(|e| MeshError::Peer(format!("STUN recv failed: {}", e)))?;

    parse_binding_response(&buf[..n], &tx_id)
}

/// Resolve a `host:port` string to a `SocketAddr`, performing DNS lookup if needed.
fn resolve_addr(addr: &str) -> Result<SocketAddr> {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs()
        .map_err(|e| MeshError::Peer(format!("DNS resolve '{}' failed: {}", addr, e)))?
        .next()
        .ok_or_else(|| MeshError::Peer(format!("No DNS result for '{}'", addr)))
}

/// Generate a random 12-byte STUN transaction ID.
fn rand_tx_id() -> [u8; 12] {
    use rand::RngCore;
    let mut id = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut id);
    id
}

/// Build a 20-byte STUN Binding Request with no attributes.
fn build_binding_request(tx_id: &[u8; 12]) -> [u8; 20] {
    let mut msg = [0u8; 20];
    // Bytes 0-1: Message Type = Binding Request (0x0001)
    msg[0..2].copy_from_slice(&MSG_BINDING_REQUEST.to_be_bytes());
    // Bytes 2-3: Message Length = 0 (no attributes)
    msg[2..4].copy_from_slice(&0u16.to_be_bytes());
    // Bytes 4-7: Magic Cookie
    msg[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    // Bytes 8-19: Transaction ID
    msg[8..20].copy_from_slice(tx_id);
    msg
}

/// Parse a STUN Binding Response and extract the mapped address attribute.
fn parse_binding_response(data: &[u8], tx_id: &[u8; 12]) -> Result<SocketAddr> {
    if data.len() < 20 {
        return Err(MeshError::Peer("STUN response too short".into()));
    }

    let msg_type = u16::from_be_bytes([data[0], data[1]]);
    let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
    let magic = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

    if msg_type != MSG_BINDING_RESPONSE {
        return Err(MeshError::Peer(format!(
            "Unexpected STUN message type: 0x{:04x}",
            msg_type
        )));
    }
    if magic != STUN_MAGIC_COOKIE {
        return Err(MeshError::Peer("Bad STUN magic cookie".into()));
    }
    if &data[8..20] != tx_id {
        return Err(MeshError::Peer("STUN transaction ID mismatch".into()));
    }

    // Parse TLV attributes starting at offset 20.
    let attrs_end = (20 + msg_len).min(data.len());
    let attrs = &data[20..attrs_end];
    let mut offset = 0;

    while offset + 4 <= attrs.len() {
        let attr_type = u16::from_be_bytes([attrs[offset], attrs[offset + 1]]);
        let attr_len = u16::from_be_bytes([attrs[offset + 2], attrs[offset + 3]]) as usize;
        offset += 4;

        if offset + attr_len > attrs.len() {
            break;
        }
        let attr_val = &attrs[offset..offset + attr_len];

        let result = match attr_type {
            ATTR_XOR_MAPPED_ADDRESS => Some(parse_xor_mapped_address(attr_val)),
            ATTR_MAPPED_ADDRESS => Some(parse_mapped_address(attr_val)),
            _ => None,
        };

        if let Some(addr) = result {
            return addr;
        }

        // Attributes are padded to 4-byte boundaries.
        offset += (attr_len + 3) & !3;
    }

    Err(MeshError::Peer(
        "No mapped address attribute in STUN response".into(),
    ))
}

/// Parse an XOR-MAPPED-ADDRESS attribute value (RFC 5389 §15.2).
///
/// The IP and port are XOR'd with the magic cookie to prevent transparent
/// NAT rewriting of address attributes.
fn parse_xor_mapped_address(data: &[u8]) -> Result<SocketAddr> {
    if data.len() < 8 {
        return Err(MeshError::Peer("XOR-MAPPED-ADDRESS too short".into()));
    }
    if data[1] != 0x01 {
        return Err(MeshError::Peer(
            "XOR-MAPPED-ADDRESS: only IPv4 supported".into(),
        ));
    }

    let xor_port = u16::from_be_bytes([data[2], data[3]]);
    let port = xor_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);

    let xor_ip = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let ip = xor_ip ^ STUN_MAGIC_COOKIE;
    let ipv4 = Ipv4Addr::from(ip);

    Ok(SocketAddr::new(ipv4.into(), port))
}

/// Parse a legacy MAPPED-ADDRESS attribute value (RFC 3489 §11.2.1).
fn parse_mapped_address(data: &[u8]) -> Result<SocketAddr> {
    if data.len() < 8 {
        return Err(MeshError::Peer("MAPPED-ADDRESS too short".into()));
    }
    if data[1] != 0x01 {
        return Err(MeshError::Peer(
            "MAPPED-ADDRESS: only IPv4 supported".into(),
        ));
    }

    let port = u16::from_be_bytes([data[2], data[3]]);
    let ipv4 = Ipv4Addr::new(data[4], data[5], data[6], data[7]);
    Ok(SocketAddr::new(ipv4.into(), port))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn test_build_binding_request_format() {
        let tx_id = [0xABu8; 12];
        let req = build_binding_request(&tx_id);

        assert_eq!(req.len(), 20);
        // Message type
        assert_eq!(u16::from_be_bytes([req[0], req[1]]), MSG_BINDING_REQUEST);
        // Message length (no attributes)
        assert_eq!(u16::from_be_bytes([req[2], req[3]]), 0);
        // Magic cookie
        assert_eq!(
            u32::from_be_bytes([req[4], req[5], req[6], req[7]]),
            STUN_MAGIC_COOKIE
        );
        // Transaction ID
        assert_eq!(&req[8..20], &tx_id);
    }

    #[test]
    fn test_parse_xor_mapped_address_known_value() {
        // Encode 1.2.3.4:5678 as XOR-MAPPED-ADDRESS.
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        let port: u16 = 5678;
        let xor_port = port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
        let xor_ip = u32::from(ip) ^ STUN_MAGIC_COOKIE;

        let mut data = vec![0x00, 0x01]; // reserved, family = IPv4
        data.extend_from_slice(&xor_port.to_be_bytes());
        data.extend_from_slice(&xor_ip.to_be_bytes());

        let addr = parse_xor_mapped_address(&data).unwrap();
        assert_eq!(addr.port(), 5678);
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));
    }

    #[test]
    fn test_parse_mapped_address_known_value() {
        let mut data = vec![0x00, 0x01, 0x1E, 0x61]; // family=IPv4, port=7777
        data.extend_from_slice(&Ipv4Addr::new(203, 0, 113, 1).octets());

        let addr = parse_mapped_address(&data).unwrap();
        assert_eq!(addr.port(), 7777);
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(203, 0, 113, 1)));
    }

    #[test]
    fn test_parse_binding_response_with_xor_mapped() {
        let tx_id = [0x77u8; 12];
        let ip = Ipv4Addr::new(203, 0, 113, 42);
        let port: u16 = 12345;
        let xor_port = port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
        let xor_ip = u32::from(ip) ^ STUN_MAGIC_COOKIE;

        // Build XOR-MAPPED-ADDRESS attribute (type 0x0020, len 8).
        let mut attr = Vec::new();
        attr.extend_from_slice(&ATTR_XOR_MAPPED_ADDRESS.to_be_bytes());
        attr.extend_from_slice(&8u16.to_be_bytes()); // attribute value length
        attr.push(0x00); // reserved
        attr.push(0x01); // family = IPv4
        attr.extend_from_slice(&xor_port.to_be_bytes());
        attr.extend_from_slice(&xor_ip.to_be_bytes());

        // Build full response header.
        let mut resp = Vec::new();
        resp.extend_from_slice(&MSG_BINDING_RESPONSE.to_be_bytes());
        resp.extend_from_slice(&(attr.len() as u16).to_be_bytes());
        resp.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        resp.extend_from_slice(&tx_id);
        resp.extend_from_slice(&attr);

        let addr = parse_binding_response(&resp, &tx_id).unwrap();
        assert_eq!(addr.port(), 12345);
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(203, 0, 113, 42)));
    }

    #[test]
    fn test_parse_binding_response_wrong_tx_id() {
        let tx_id = [0x01u8; 12];
        let wrong_tx_id = [0x02u8; 12];

        // Minimal valid response header with wrong TX ID.
        let mut resp = Vec::new();
        resp.extend_from_slice(&MSG_BINDING_RESPONSE.to_be_bytes());
        resp.extend_from_slice(&0u16.to_be_bytes());
        resp.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        resp.extend_from_slice(&wrong_tx_id);

        assert!(parse_binding_response(&resp, &tx_id).is_err());
    }

    #[test]
    fn test_external_addr_discovery_fails_gracefully_without_network() {
        // Use port 0 — OS will assign an ephemeral port.
        // This will fail (no real network in CI) but must not panic.
        // We bind on port 0 which the OS will assign.
        let result = discover_external_addr(0);
        // In CI without internet, this is expected to fail. That's OK.
        // The important thing is that it returns Err, not panics.
        let _ = result;
    }
}
