//! Obfuscation transport implementations.
//!
//! Two modes:
//! - `tls`   — TLS 1.3 with WebSocket upgrade headers (indistinguishable from HTTPS WS)
//! - `obfs4` — Random-looking byte stream with no fixed headers (Phase 2)
pub mod obfs4;
pub mod tls;

use serde::{Deserialize, Serialize};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

pub use tls::TlsCamouflage;

/// Configured obfuscation mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ObfsMode {
    /// No obfuscation — raw TCP (default for backwards compatibility).
    #[default]
    None,
    /// TLS 1.3 camouflage with WebSocket upgrade (recommended).
    Tls,
    /// obfs4 random-looking byte stream (Phase 2).
    Obfs4,
}

/// A unified stream type that can be plain TCP or TLS-wrapped.
///
/// Implements `AsyncRead + AsyncWrite + Unpin` so it can be used wherever
/// a `TcpStream` is expected, after the obfuscation handshake completes.
pub enum ObfsStream {
    /// Unencrypted TCP (obfs_mode = "none").
    Plain(TcpStream),
    /// TLS client stream (outgoing connection).
    TlsClient(tokio_rustls::client::TlsStream<TcpStream>),
    /// TLS server stream (incoming connection).
    TlsServer(tokio_rustls::server::TlsStream<TcpStream>),
}

impl AsyncRead for ObfsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            ObfsStream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            ObfsStream::TlsClient(s) => Pin::new(s).poll_read(cx, buf),
            ObfsStream::TlsServer(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for ObfsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            ObfsStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            ObfsStream::TlsClient(s) => Pin::new(s).poll_write(cx, buf),
            ObfsStream::TlsServer(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            ObfsStream::Plain(s) => Pin::new(s).poll_flush(cx),
            ObfsStream::TlsClient(s) => Pin::new(s).poll_flush(cx),
            ObfsStream::TlsServer(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            ObfsStream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            ObfsStream::TlsClient(s) => Pin::new(s).poll_shutdown(cx),
            ObfsStream::TlsServer(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

// All inner types are Unpin, so ObfsStream is Unpin.
impl Unpin for ObfsStream {}
