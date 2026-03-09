//! NAT traversal tests (Upgrade 2).
//!
//! Tests STUN response parsing with a mock server and verifies hole-punching
//! works on the local loopback interface.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::thread;
use std::time::Duration;

// RFC 5389 constants (duplicated here for the test helper — no test code
// should import private implementation details).
const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;
const MSG_BINDING_RESPONSE: u16 = 0x0101;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;

/// Build a minimal STUN Binding Response with an XOR-MAPPED-ADDRESS attribute.
fn mock_stun_response(tx_id: &[u8; 12], external_ip: Ipv4Addr, external_port: u16) -> Vec<u8> {
    let xor_port = external_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
    let xor_ip = u32::from(external_ip) ^ STUN_MAGIC_COOKIE;

    let mut attr = Vec::new();
    attr.extend_from_slice(&ATTR_XOR_MAPPED_ADDRESS.to_be_bytes());
    attr.extend_from_slice(&8u16.to_be_bytes()); // value length
    attr.push(0x00); // reserved
    attr.push(0x01); // family = IPv4
    attr.extend_from_slice(&xor_port.to_be_bytes());
    attr.extend_from_slice(&xor_ip.to_be_bytes());

    let mut resp = Vec::new();
    resp.extend_from_slice(&MSG_BINDING_RESPONSE.to_be_bytes());
    resp.extend_from_slice(&(attr.len() as u16).to_be_bytes());
    resp.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    resp.extend_from_slice(tx_id);
    resp.extend_from_slice(&attr);
    resp
}

/// Spawn a mock STUN server that responds to any Binding Request with a
/// fixed external address, then exits after one exchange.
fn start_mock_stun_server(external_ip: Ipv4Addr, external_port: u16) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap();

    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    thread::spawn(move || {
        let mut buf = [0u8; 1024];
        if let Ok((n, from)) = socket.recv_from(&mut buf) {
            if n >= 20 {
                // Extract transaction ID from request.
                let tx_id: [u8; 12] = buf[8..20].try_into().unwrap();
                let resp = mock_stun_response(&tx_id, external_ip, external_port);
                let _ = socket.send_to(&resp, from);
            }
        }
    });

    addr
}

/// Full round-trip: send a STUN request to a mock server and parse the response.
#[test]
fn test_stun_external_address_discovery_mock() {
    let fake_external_ip = Ipv4Addr::new(203, 0, 113, 42);
    let fake_external_port: u16 = 54321;

    let mock_server_addr = start_mock_stun_server(fake_external_ip, fake_external_port);

    // Use the mock server address directly.  We can't call `discover_external_addr`
    // (which uses the hardcoded Google STUN list), so we test the parsing logic
    // via the mock round-trip using the stdlib UdpSocket.
    let local: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let socket = UdpSocket::bind(local).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();

    // Build a minimal STUN binding request.
    let tx_id = [0xDEu8; 12];
    let mut req = [0u8; 20];
    req[0..2].copy_from_slice(&0x0001u16.to_be_bytes()); // Binding Request
    req[2..4].copy_from_slice(&0u16.to_be_bytes()); // length
    req[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    req[8..20].copy_from_slice(&tx_id);

    socket.send_to(&req, mock_server_addr).unwrap();

    let mut buf = [0u8; 512];
    let (n, _) = socket.recv_from(&mut buf).unwrap();

    // Parse the response manually.
    assert!(n >= 20, "Response must be at least 20 bytes");
    let resp_type = u16::from_be_bytes([buf[0], buf[1]]);
    assert_eq!(resp_type, MSG_BINDING_RESPONSE);

    // Verify TX ID echo.
    assert_eq!(&buf[8..20], &tx_id);

    // Extract XOR-MAPPED-ADDRESS from attribute payload.
    let attr_data = &buf[20..n];
    assert!(
        attr_data.len() >= 12,
        "Response must contain mapped address attribute"
    );

    let attr_type = u16::from_be_bytes([attr_data[0], attr_data[1]]);
    assert_eq!(attr_type, ATTR_XOR_MAPPED_ADDRESS);

    let xor_port = u16::from_be_bytes([attr_data[6], attr_data[7]]);
    let decoded_port = xor_port ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
    assert_eq!(decoded_port, fake_external_port);

    let xor_ip = u32::from_be_bytes([attr_data[8], attr_data[9], attr_data[10], attr_data[11]]);
    let decoded_ip = xor_ip ^ STUN_MAGIC_COOKIE;
    assert_eq!(Ipv4Addr::from(decoded_ip), fake_external_ip);
}

/// Hole-punching loopback test (covered more thoroughly in nat::hole_punch unit tests,
/// but exercised here as an integration test).
#[tokio::test]
async fn test_hole_punch_succeeds_on_loopback() {
    use meshlink_core::nat::{punch_hole, HolePunchConfig};
    use std::time::Duration;

    // Bind two sockets to get their ports, then release them.
    let a = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let b = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr_a = a.local_addr().unwrap();
    let addr_b = b.local_addr().unwrap();
    drop(a);
    drop(b);

    let cfg_a = HolePunchConfig {
        local_addr: addr_a,
        remote_external_addr: addr_b,
        timeout: Duration::from_secs(2),
        probe_count: 3,
    };
    let cfg_b = HolePunchConfig {
        local_addr: addr_b,
        remote_external_addr: addr_a,
        timeout: Duration::from_secs(2),
        probe_count: 3,
    };

    let (r_a, r_b) = tokio::join!(punch_hole(cfg_a), punch_hole(cfg_b));
    assert!(r_a.unwrap().success);
    assert!(r_b.unwrap().success);
}
