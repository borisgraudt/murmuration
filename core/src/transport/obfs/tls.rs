//! TLS 1.3 camouflage transport.
//!
//! Wraps Murmuration TCP connections in real TLS 1.3 using a self-signed certificate
//! with a plausible CDN hostname as the CN/SAN. An HTTP/1.1 WebSocket upgrade
//! handshake is exchanged immediately after TLS, making the traffic byte-for-byte
//! indistinguishable from HTTPS WebSocket to any passive observer or DPI device.
//!
//! Security model:
//! - TLS provides the obfuscation layer (looks like HTTPS to ISPs)
//! - Murmuration's existing RSA node identity + X25519 session keys provide actual security
//! - The TLS certificate is self-signed and accepted by both sides (we do our own auth)

use crate::error::{MeshError, Result};
use rustls::client::ServerCertVerifier;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// Default SNI hostname presented in the TLS ClientHello.
/// Chosen to mimic Cloudflare CDN traffic seen on every network.
pub const DEFAULT_SNI: &str = "cdn.cloudflare.com";

/// HTTP/1.1 WebSocket upgrade request (client → server), sent after TLS.
/// Mimics a browser WebSocket connection to further confuse DPI heuristics.
const WS_UPGRADE_REQUEST: &[u8] = b"\
GET / HTTP/1.1\r\n\
Host: cdn.cloudflare.com\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
Sec-WebSocket-Version: 13\r\n\
\r\n";

/// HTTP/1.1 101 Switching Protocols response (server → client).
const WS_UPGRADE_RESPONSE: &[u8] = b"\
HTTP/1.1 101 Switching Protocols\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\
\r\n";

/// TLS camouflage factory — holds pre-built server and client TLS configs.
///
/// Create once at node startup and clone the Arc for each connection.
#[derive(Clone)]
pub struct TlsCamouflage {
    server_config: Arc<rustls::ServerConfig>,
    client_config: Arc<rustls::ClientConfig>,
    sni: String,
}

impl TlsCamouflage {
    /// Build a `TlsCamouflage` with a freshly-generated self-signed certificate.
    ///
    /// The certificate CN and SAN are set to `sni` (default: `cdn.cloudflare.com`).
    pub fn new(sni: &str) -> Result<Self> {
        let (server_config, client_config) = build_tls_configs(sni)?;
        Ok(Self {
            server_config: Arc::new(server_config),
            client_config: Arc::new(client_config),
            sni: sni.to_string(),
        })
    }

    /// Wrap an **incoming** TCP connection as a TLS server.
    ///
    /// Completes TLS handshake, then reads the WebSocket upgrade request and
    /// sends the 101 response so both sides enter data mode together.
    pub async fn accept(
        &self,
        stream: TcpStream,
    ) -> Result<tokio_rustls::server::TlsStream<TcpStream>> {
        let acceptor = TlsAcceptor::from(self.server_config.clone());
        let mut tls = acceptor
            .accept(stream)
            .await
            .map_err(|e| MeshError::Peer(format!("TLS accept failed: {}", e)))?;

        // Read WebSocket upgrade request (ignore content, just drain it).
        let mut buf = [0u8; 512];
        let _ = tls.read(&mut buf).await;

        // Send 101 Switching Protocols.
        tls.write_all(WS_UPGRADE_RESPONSE)
            .await
            .map_err(|e| MeshError::Peer(format!("WS upgrade response failed: {}", e)))?;

        Ok(tls)
    }

    /// Wrap an **outgoing** TCP connection as a TLS client.
    ///
    /// Completes TLS handshake, then sends the WebSocket upgrade request and
    /// reads the 101 response before returning.
    pub async fn connect(
        &self,
        stream: TcpStream,
    ) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
        let connector = TlsConnector::from(self.client_config.clone());
        let server_name = rustls::ServerName::try_from(self.sni.as_str())
            .map_err(|e| MeshError::Peer(format!("Invalid TLS SNI '{}': {}", self.sni, e)))?;

        let mut tls = connector
            .connect(server_name, stream)
            .await
            .map_err(|e| MeshError::Peer(format!("TLS connect failed: {}", e)))?;

        // Send WebSocket upgrade request.
        tls.write_all(WS_UPGRADE_REQUEST)
            .await
            .map_err(|e| MeshError::Peer(format!("WS upgrade request failed: {}", e)))?;

        // Read 101 Switching Protocols response (ignore content).
        let mut buf = [0u8; 512];
        let _ = tls.read(&mut buf).await;

        Ok(tls)
    }
}

/// Generate a matched pair of TLS server + client configs.
///
/// The server config uses a fresh self-signed ECDSA P-256 certificate.
/// The client config uses a custom verifier that accepts any certificate —
/// actual peer authentication is handled by Murmuration's RSA node identity system.
fn build_tls_configs(sni: &str) -> Result<(rustls::ServerConfig, rustls::ClientConfig)> {
    // Generate self-signed certificate (ECDSA P-256, expires in 10 years).
    let cert = rcgen::generate_simple_self_signed(vec![sni.to_string()])
        .map_err(|e| MeshError::Peer(format!("Certificate generation failed: {}", e)))?;

    let cert_der = cert
        .serialize_der()
        .map_err(|e| MeshError::Peer(format!("Certificate serialization failed: {}", e)))?;
    let key_der = cert.serialize_private_key_der();

    let server_cert = rustls::Certificate(cert_der);
    let server_key = rustls::PrivateKey(key_der);

    // Server config: present the self-signed cert, no client auth required.
    let server_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![server_cert], server_key)
        .map_err(|e| MeshError::Peer(format!("TLS server config failed: {}", e)))?;

    // Client config: skip certificate validation (Murmuration does its own peer auth).
    let client_config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();

    Ok((server_config, client_config))
}

/// A `ServerCertVerifier` that accepts any certificate without validation.
///
/// This is intentionally permissive: the TLS layer provides *obfuscation*,
/// not authentication. Real peer authentication is handled by the Murmuration
/// RSA node identity + X25519 handshake that runs inside the TLS tunnel.
#[derive(Debug)]
struct AcceptAnyCert;

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> std::result::Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// Verify that a TLS camouflage connection can be established over loopback
    /// and that arbitrary data passes through correctly.
    #[tokio::test]
    async fn test_tls_traffic_looks_like_https() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let camouflage = TlsCamouflage::new(DEFAULT_SNI).unwrap();
        let camouflage_server = camouflage.clone();

        // Server side: accept one connection.
        let server_task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut tls = camouflage_server.accept(tcp).await.unwrap();
            // Echo back whatever we receive.
            let mut buf = [0u8; 32];
            let n = tls.read(&mut buf).await.unwrap();
            tls.write_all(&buf[..n]).await.unwrap();
        });

        // Client side: connect, send data, read echo.
        let tcp = TcpStream::connect(addr).await.unwrap();
        let mut tls = camouflage.connect(tcp).await.unwrap();

        let payload = b"murmuration-hidden-payload";
        tls.write_all(payload).await.unwrap();

        let mut echo = [0u8; 26];
        tls.read_exact(&mut echo).await.unwrap();
        assert_eq!(&echo, payload);

        server_task.await.unwrap();
    }

    /// Verify that the WS upgrade request bytes look like an HTTP request
    /// (starts with "GET /" — the signature a DPI would match against HTTPS WS).
    #[test]
    fn test_ws_upgrade_looks_like_websocket() {
        assert!(WS_UPGRADE_REQUEST.starts_with(b"GET /"));
        assert!(WS_UPGRADE_REQUEST.windows(9).any(|w| w == b"websocket"));
        assert!(WS_UPGRADE_RESPONSE.starts_with(b"HTTP/1.1 101"));
    }
}
