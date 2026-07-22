//! Peer types used by the router — connection state, per-peer metrics, and
//! the observable `PeerInfo` a routing decision reads. The networking
//! `PeerManager` stays in the parent node; these are the pure data types.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Connection state of a peer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Attempting to connect
    Connecting,
    /// Handshake in progress
    Handshaking,
    /// Fully connected and ready
    Connected,
    /// Connection is closing
    Closing,
}

/// Peer metrics for AI routing
#[derive(Debug, Clone)]
pub struct PeerMetrics {
    /// Average latency (measured via ping/pong)
    pub latency: Option<Duration>,
    /// Uptime (how long peer has been connected)
    pub uptime: Duration,
    /// Number of successful pings
    pub ping_count: u32,
    /// Number of failed pings
    pub ping_failures: u32,
    /// Last ping timestamp
    pub last_ping: Option<Instant>,
}

impl Default for PeerMetrics {
    fn default() -> Self {
        Self {
            latency: None,
            uptime: Duration::ZERO,
            ping_count: 0,
            ping_failures: 0,
            last_ping: None,
        }
    }
}

impl PeerMetrics {
    /// Update latency with new measurement (exponential moving average)
    pub fn update_latency(&mut self, new_latency: Duration) {
        const ALPHA: f64 = 0.3; // Smoothing factor
        self.latency = Some(
            self.latency
                .map(|old| {
                    let old_ms = old.as_secs_f64() * 1000.0;
                    let new_ms = new_latency.as_secs_f64() * 1000.0;
                    let smoothed = ALPHA * new_ms + (1.0 - ALPHA) * old_ms;
                    Duration::from_millis(smoothed as u64)
                })
                .unwrap_or(new_latency),
        );
        self.ping_count += 1;
        self.last_ping = Some(Instant::now());
    }

    /// Record a ping failure
    pub fn record_ping_failure(&mut self) {
        self.ping_failures += 1;
    }

    /// Calculate reliability score (0.0 to 1.0)
    pub fn reliability_score(&self) -> f32 {
        let total_pings = self.ping_count + self.ping_failures;
        if total_pings == 0 {
            return 0.5; // Default score
        }
        (self.ping_count as f32 / total_pings as f32).clamp(0.0, 1.0)
    }
}

/// Information about a peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: String,
    pub address: SocketAddr,
    pub state: ConnectionState,
    pub protocol_version: Option<u8>,
    pub last_seen: Option<Instant>,
    pub connected_at: Option<Instant>,
    pub connection_attempts: u32,
    pub added_at: Instant,    // When this peer was first added
    pub metrics: PeerMetrics, // Metrics for AI routing
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(node_id: String, address: SocketAddr) -> Self {
        Self {
            node_id,
            address,
            state: ConnectionState::Disconnected,
            protocol_version: None,
            last_seen: None,
            connected_at: None,
            connection_attempts: 0,
            added_at: Instant::now(),
            metrics: PeerMetrics::default(),
        }
    }

    /// Check if peer is connected
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = Some(Instant::now());
    }

    /// Update uptime based on connected_at (call periodically to keep metrics fresh)
    pub fn update_uptime(&mut self) {
        if let Some(connected_at) = self.connected_at {
            self.metrics.uptime = connected_at.elapsed();
        } else {
            self.metrics.uptime = Duration::ZERO;
        }
    }

    /// Get current uptime
    pub fn get_uptime(&self) -> Duration {
        if let Some(connected_at) = self.connected_at {
            connected_at.elapsed()
        } else {
            Duration::ZERO
        }
    }

    /// Check if peer should be considered stale
    pub fn is_stale(&self, timeout: Duration) -> bool {
        // Don't consider peers stale if they were just added (within 30 seconds)
        if self.added_at.elapsed() < Duration::from_secs(30) {
            return false;
        }

        if let Some(last_seen) = self.last_seen {
            last_seen.elapsed() > timeout
        } else {
            // If never seen, consider stale only if added more than timeout ago
            self.added_at.elapsed() > timeout
        }
    }
}
