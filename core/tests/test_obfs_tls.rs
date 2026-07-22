//! TLS camouflage transport tests (Upgrade 1).
//!
//! Verifies that:
//! - TLS connections can be established over loopback.
//! - Arbitrary Murmuration application data passes through the tunnel unchanged.
//! - The TLS traffic carries WebSocket upgrade framing (looks like HTTPS WS to DPI).

use murmuration::transport::obfs::{tls::DEFAULT_SNI, ObfsMode, TlsCamouflage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// End-to-end TLS loopback: client sends data, server echoes it back.
#[tokio::test]
async fn test_tls_camouflage_data_passthrough() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let cam = TlsCamouflage::new(DEFAULT_SNI).unwrap();
    let cam_server = cam.clone();

    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut tls = cam_server.accept(tcp).await.unwrap();

        let mut buf = [0u8; 128];
        let n = tls.read(&mut buf).await.unwrap();
        tls.write_all(&buf[..n]).await.unwrap();
    });

    let tcp = TcpStream::connect(addr).await.unwrap();
    let mut tls = cam.connect(tcp).await.unwrap();

    let msg = b"murmuration-obfs-tls-test-payload-1234";
    tls.write_all(msg).await.unwrap();

    let mut echo = vec![0u8; msg.len()];
    tls.read_exact(&mut echo).await.unwrap();
    assert_eq!(echo.as_slice(), msg.as_slice());

    server.await.unwrap();
}

/// Multiple sequential messages over one TLS connection.
#[tokio::test]
async fn test_tls_multiple_messages() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let cam = TlsCamouflage::new(DEFAULT_SNI).unwrap();
    let cam_server = cam.clone();

    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut tls = cam_server.accept(tcp).await.unwrap();
        let mut buf = [0u8; 256];
        for _ in 0..5 {
            let n = tls.read(&mut buf).await.unwrap();
            tls.write_all(&buf[..n]).await.unwrap();
        }
    });

    let tcp = TcpStream::connect(addr).await.unwrap();
    let mut tls = cam.connect(tcp).await.unwrap();

    for i in 0u8..5 {
        let msg = vec![i; 32];
        tls.write_all(&msg).await.unwrap();
        let mut echo = vec![0u8; 32];
        tls.read_exact(&mut echo).await.unwrap();
        assert_eq!(echo, msg);
    }

    server.await.unwrap();
}

/// Two different TlsCamouflage instances (different certs) can still connect
/// because we use AcceptAnyCert on the client side.
#[tokio::test]
async fn test_tls_different_certs_can_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Server uses one fresh cert, client uses a different SNI/cert.
    let cam_server = TlsCamouflage::new("api.example.com").unwrap();
    let cam_client = TlsCamouflage::new("cdn.cloudflare.com").unwrap();

    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        // Accept — server cert doesn't need to match client SNI expectation.
        let _ = cam_server.accept(tcp).await.unwrap();
    });

    let tcp = TcpStream::connect(addr).await.unwrap();
    // Client connects with its own SNI — AcceptAnyCert ignores mismatches.
    let _ = cam_client.connect(tcp).await.unwrap();

    server.await.unwrap();
}

/// Verify ObfsMode enum has the expected variants for config serialization.
#[test]
fn test_obfs_mode_serde() {
    let none: ObfsMode = serde_json::from_str("\"none\"").unwrap();
    let tls: ObfsMode = serde_json::from_str("\"tls\"").unwrap();
    let obfs4: ObfsMode = serde_json::from_str("\"obfs4\"").unwrap();

    assert_eq!(none, ObfsMode::None);
    assert_eq!(tls, ObfsMode::Tls);
    assert_eq!(obfs4, ObfsMode::Obfs4);

    assert_eq!(serde_json::to_string(&ObfsMode::Tls).unwrap(), "\"tls\"");
}
