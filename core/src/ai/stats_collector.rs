/// Statistics collector for AI routing
/// Collects latency, uptime, packet loss, and trust metrics
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Peer statistics for routing decisions
#[derive(Debug, Clone)]
pub struct PeerStats {
    pub peer_id: String,
    pub latency: Option<Duration>, // Average latency
    pub uptime: Duration,          // How long peer has been connected
    pub packet_loss: f32,          // Packet loss rate (0.0 to 1.0)
    pub trust_score: f32,          // Trust score (0.0 to 1.0)
    pub last_seen: Instant,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub errors: u64,
}

impl PeerStats {
    pub fn new(peer_id: String) -> Self {
        Self {
            peer_id,
            latency: None,
            uptime: Duration::ZERO,
            packet_loss: 0.0,
            trust_score: 1.0, // Start with full trust
            last_seen: Instant::now(),
            messages_sent: 0,
            messages_received: 0,
            errors: 0,
        }
    }

    /// Calculate reliability score (0.0 to 1.0)
    pub fn reliability_score(&self) -> f32 {
        let error_rate = if self.messages_sent > 0 {
            self.errors as f32 / self.messages_sent as f32
        } else {
            0.0
        };

        // Combine trust, packet loss, and error rate
        (self.trust_score * (1.0 - self.packet_loss) * (1.0 - error_rate))
            .max(0.0)
            .min(1.0)
    }
}

/// Statistics collector for all peers
pub struct StatsCollector {
    stats: Arc<RwLock<HashMap<String, PeerStats>>>,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a ping/pong latency measurement
    pub async fn record_latency(&self, peer_id: &str, latency: Duration) {
        let mut stats = self.stats.write().await;
        let peer_stats = stats
            .entry(peer_id.to_string())
            .or_insert_with(|| PeerStats::new(peer_id.to_string()));

        // Simple moving average (could be improved with exponential moving average)
        peer_stats.latency = Some(
            peer_stats
                .latency
                .map(|old| (old + latency) / 2)
                .unwrap_or(latency),
        );
        peer_stats.last_seen = Instant::now();
    }

    /// Record a message sent
    pub async fn record_message_sent(&self, peer_id: &str) {
        let mut stats = self.stats.write().await;
        let peer_stats = stats
            .entry(peer_id.to_string())
            .or_insert_with(|| PeerStats::new(peer_id.to_string()));
        peer_stats.messages_sent += 1;
        peer_stats.last_seen = Instant::now();
    }

    /// Record a message received
    pub async fn record_message_received(&self, peer_id: &str) {
        let mut stats = self.stats.write().await;
        let peer_stats = stats
            .entry(peer_id.to_string())
            .or_insert_with(|| PeerStats::new(peer_id.to_string()));
        peer_stats.messages_received += 1;
        peer_stats.last_seen = Instant::now();
    }

    /// Record an error
    pub async fn record_error(&self, peer_id: &str) {
        let mut stats = self.stats.write().await;
        let peer_stats = stats
            .entry(peer_id.to_string())
            .or_insert_with(|| PeerStats::new(peer_id.to_string()));
        peer_stats.errors += 1;

        // Decrease trust score on errors
        peer_stats.trust_score = (peer_stats.trust_score * 0.9).max(0.1);
    }

    /// Update peer uptime
    pub async fn update_uptime(&self, peer_id: &str, uptime: Duration) {
        let mut stats = self.stats.write().await;
        if let Some(peer_stats) = stats.get_mut(peer_id) {
            peer_stats.uptime = uptime;
        }
    }

    /// Get stats for a peer
    pub async fn get_stats(&self, peer_id: &str) -> Option<PeerStats> {
        let stats = self.stats.read().await;
        stats.get(peer_id).cloned()
    }

    /// Get all stats
    pub async fn get_all_stats(&self) -> Vec<PeerStats> {
        let stats = self.stats.read().await;
        stats.values().cloned().collect()
    }

    /// Calculate routing score for a peer (higher is better)
    pub async fn calculate_score(&self, peer_id: &str) -> f32 {
        let stats = self.stats.read().await;
        if let Some(peer_stats) = stats.get(peer_id) {
            let latency_score = peer_stats
                .latency
                .map(|lat| {
                    // Lower latency = higher score (normalize to 0-1, assuming max 1s latency)
                    (1.0 - (lat.as_secs_f32().min(1.0))).max(0.0)
                })
                .unwrap_or(0.5);

            let uptime_score = (peer_stats.uptime.as_secs_f32() / 3600.0).min(1.0); // Normalize to 1 hour

            let reliability = peer_stats.reliability_score();

            // Weighted combination: 30% latency, 20% uptime, 50% reliability
            0.3 * latency_score + 0.2 * uptime_score + 0.5 * reliability
        } else {
            0.5 // Default score for unknown peers
        }
    }

    /// Remove stale stats
    pub async fn cleanup_stale(&self, timeout: Duration) {
        let mut stats = self.stats.write().await;
        let now = Instant::now();
        stats.retain(|_, peer_stats| now.duration_since(peer_stats.last_seen) < timeout);
    }
}

impl Clone for StatsCollector {
    fn clone(&self) -> Self {
        Self {
            stats: self.stats.clone(),
        }
    }
}

impl Default for StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stats_collector() {
        let collector = StatsCollector::new();

        // Record some stats
        collector
            .record_latency("peer1", Duration::from_millis(50))
            .await;
        collector.record_message_sent("peer1").await;
        collector.record_message_received("peer1").await;

        let stats = collector.get_stats("peer1").await;
        assert!(stats.is_some());
        let stats = stats.unwrap();
        assert_eq!(stats.messages_sent, 1);
        assert_eq!(stats.messages_received, 1);
        assert!(stats.latency.is_some());
    }

    #[tokio::test]
    async fn test_score_calculation() {
        let collector = StatsCollector::new();

        collector
            .record_latency("peer1", Duration::from_millis(10))
            .await;
        collector
            .update_uptime("peer1", Duration::from_secs(3600))
            .await;

        let score = collector.calculate_score("peer1").await;
        assert!(score > 0.0 && score <= 1.0);
    }
}
