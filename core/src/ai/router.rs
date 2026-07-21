/// Routing logic for mesh messages
/// Implements UCB1 (Upper Confidence Bound) adaptive routing.
/// Reference: Auer et al., "Finite-time Analysis of the Multiarmed Bandit Problem", 2002.
///
/// UCB1 state is persisted to sled under the key "ucb1_state" so that learned peer
/// quality survives node restarts.
use crate::error::{MeshError, Result};
use crate::p2p::protocol::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// UCB1 exploration constant (standard value = 2.0).
const UCB1_C: f64 = 2.0;
/// Number of selections required before switching from heuristic warm-up to pure UCB1.
const UCB1_MIN_SAMPLES: u64 = 5;

/// Elysium address format: ely://<node_id>
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElysiumAddress {
    pub node_id: String,
}

impl ElysiumAddress {
    /// Parse from string format: ely://<node_id>
    pub fn from_string(addr: &str) -> Result<Self> {
        if let Some(stripped) = addr.strip_prefix("ely://") {
            Ok(Self {
                node_id: stripped.to_string(),
            })
        } else {
            Err(MeshError::Protocol(format!(
                "Invalid Elysium address format: {}",
                addr
            )))
        }
    }
}

impl fmt::Display for ElysiumAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ely://{}", self.node_id)
    }
}

/// Mesh message for routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    pub from: String,
    pub to: Option<String>, // None = broadcast
    pub data: Vec<u8>,
    pub message_id: String,
    pub ttl: u8,
    pub path: Vec<String>, // Route path for loop detection
}

impl MeshMessage {
    /// Create a new mesh message
    pub fn new(from: String, to: Option<String>, data: Vec<u8>) -> Self {
        Self {
            from,
            to,
            data,
            message_id: uuid::Uuid::new_v4().to_string(),
            ttl: 10, // Default TTL
            path: Vec::new(),
        }
    }

    /// Convert to protocol message
    pub fn to_protocol_message(&self) -> Message {
        Message::MeshMessage {
            from: self.from.clone(),
            to: self.to.clone(),
            data: self.data.clone(),
            message_id: self.message_id.clone(),
            ttl: self.ttl,
            path: self.path.clone(),
        }
    }

    /// Create from protocol message
    pub fn from_protocol_message(msg: &Message) -> Option<Self> {
        if let Message::MeshMessage {
            from,
            to,
            data,
            message_id,
            ttl,
            path,
        } = msg
        {
            Some(Self {
                from: from.clone(),
                to: to.clone(),
                data: data.clone(),
                message_id: message_id.clone(),
                ttl: *ttl,
                path: path.clone(),
            })
        } else {
            None
        }
    }
}

const UCB1_DB_KEY: &[u8] = b"ucb1_state_v1";

/// Router for mesh message routing.
/// Peer selection uses UCB1 (multi-armed bandit) once sufficient samples exist;
/// falls back to a heuristic score during cold-start.
/// UCB1 state is persisted to sled so learned topology survives restarts.
pub struct Router {
    our_node_id: String,
    seen_messages: Arc<RwLock<HashMap<String, Instant>>>, // message_id -> timestamp
    message_cache: Arc<RwLock<HashMap<String, MeshMessage>>>, // Cache for deduplication
    route_history: Arc<RwLock<HashMap<String, RouteStats>>>, // peer_id -> heuristic stats
    ucb_state: Arc<RwLock<UcbState>>,                     // UCB1 bandit state
    /// Optional sled tree for persisting UCB1 state across restarts.
    db: Option<Arc<sled::Tree>>,
}

/// Statistics for a route (peer)
#[derive(Debug, Clone)]
pub struct RouteStats {
    success_count: u32,
    failure_count: u32,
    total_latency: Duration,
    sample_count: u32,
    last_updated: Instant,
}

/// Per-peer UCB1 state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UcbPeerStats {
    /// n_i: number of times this peer was selected for routing.
    selections: u64,
    /// μ_i: running average reward (incremental update).
    avg_reward: f64,
}

/// Global UCB1 bandit state shared across all routing decisions.
/// Serializable so it can be persisted to sled and survive restarts.
///
/// # Destination conditioning
///
/// `peers` is keyed by peer alone, which makes it a *destination-agnostic*
/// estimate: it answers "is this neighbour generally reliable?", not "is this
/// neighbour a good step toward D?". Those are different questions, and only the
/// second one is routing. Benchmarking showed the agnostic form plateaus far
/// below an oracle that conditions on the destination, so `by_dest` keeps a
/// separate bandit per destination; see `get_best_forward_peers_toward`.
///
/// Both are retained: the agnostic estimate is still the right prior for peer
/// health, and keeping it preserves the on-disk format written by earlier
/// versions.
#[derive(Debug, Default, Serialize, Deserialize)]
struct UcbState {
    /// N: total routing selections across all peers.
    total_selections: u64,
    peers: HashMap<String, UcbPeerStats>,
    /// destination node_id → bandit conditioned on that destination.
    #[serde(default)]
    by_dest: HashMap<String, DestBandit>,
}

/// A UCB1 bandit scoped to a single destination.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DestBandit {
    total_selections: u64,
    peers: HashMap<String, UcbPeerStats>,
}

impl DestBandit {
    fn ucb1_score(&self, peer_id: &str) -> f64 {
        match self.peers.get(peer_id) {
            None => f64::INFINITY,
            Some(s) if s.selections == 0 => f64::INFINITY,
            Some(s) => {
                let exploration = if self.total_selections > 0 {
                    (UCB1_C * (self.total_selections as f64).ln() / s.selections as f64).sqrt()
                } else {
                    0.0
                };
                s.avg_reward + exploration
            }
        }
    }

    fn record_reward(&mut self, peer_id: &str, reward: f64) {
        self.total_selections += 1;
        let s = self.peers.entry(peer_id.to_string()).or_default();
        s.selections += 1;
        s.avg_reward += (reward - s.avg_reward) / s.selections as f64;
    }

    fn selections(&self, peer_id: &str) -> u64 {
        self.peers.get(peer_id).map_or(0, |s| s.selections)
    }
}

impl UcbState {
    /// UCB1 score for peer_i: μ_i + sqrt(C * ln(N) / n_i).
    /// Caller must ensure selections >= UCB1_MIN_SAMPLES before calling.
    fn ucb1_score(&self, peer_id: &str) -> f64 {
        match self.peers.get(peer_id) {
            None => f64::INFINITY,
            Some(s) if s.selections == 0 => f64::INFINITY,
            Some(s) => {
                let exploration = if self.total_selections > 0 {
                    (UCB1_C * (self.total_selections as f64).ln() / s.selections as f64).sqrt()
                } else {
                    0.0
                };
                s.avg_reward + exploration
            }
        }
    }

    /// Record routing outcome for peer_i using incremental average update.
    fn record_reward(&mut self, peer_id: &str, reward: f64) {
        self.total_selections += 1;
        let s = self.peers.entry(peer_id.to_string()).or_default();
        s.selections += 1;
        // Incremental mean: μ ← μ + (r - μ) / n
        s.avg_reward += (reward - s.avg_reward) / s.selections as f64;
    }

    /// Return how many times peer has been selected (0 if unknown).
    fn selections(&self, peer_id: &str) -> u64 {
        self.peers.get(peer_id).map_or(0, |s| s.selections)
    }
}

impl Router {
    /// Create a new router (no persistence).
    pub fn new(our_node_id: String) -> Self {
        Self {
            our_node_id,
            seen_messages: Arc::new(RwLock::new(HashMap::new())),
            message_cache: Arc::new(RwLock::new(HashMap::new())),
            route_history: Arc::new(RwLock::new(HashMap::new())),
            ucb_state: Arc::new(RwLock::new(UcbState::default())),
            db: None,
        }
    }

    /// Create a router backed by `sled_tree` for UCB1 state persistence.
    /// Previously learned peer quality is loaded immediately.
    pub fn with_db(our_node_id: String, sled_tree: sled::Tree) -> Self {
        let db = Arc::new(sled_tree);
        let initial_state = db
            .get(UCB1_DB_KEY)
            .ok()
            .flatten()
            .and_then(|bytes| serde_json::from_slice::<UcbState>(&bytes).ok())
            .unwrap_or_default();
        Self {
            our_node_id,
            seen_messages: Arc::new(RwLock::new(HashMap::new())),
            message_cache: Arc::new(RwLock::new(HashMap::new())),
            route_history: Arc::new(RwLock::new(HashMap::new())),
            ucb_state: Arc::new(RwLock::new(initial_state)),
            db: Some(db),
        }
    }

    /// Persist the current UCB1 state to sled (best-effort; logs on failure).
    async fn persist_ucb_state(&self) {
        if let Some(db) = &self.db {
            let state = self.ucb_state.read().await;
            match serde_json::to_vec(&*state) {
                Ok(bytes) => {
                    if let Err(e) = db.insert(UCB1_DB_KEY, bytes) {
                        warn!("Failed to persist UCB1 state: {}", e);
                    }
                }
                Err(e) => warn!("Failed to serialize UCB1 state: {}", e),
            }
        }
    }

    /// Check if message should be processed (deduplication and TTL check)
    pub async fn should_process(&self, message: &MeshMessage) -> bool {
        // Check TTL
        if message.ttl == 0 {
            debug!("Message {} dropped: TTL expired", message.message_id);
            return false;
        }

        // Check if we've seen this message recently (within 60 seconds)
        let seen = self.seen_messages.read().await;
        if let Some(timestamp) = seen.get(&message.message_id) {
            if timestamp.elapsed() < Duration::from_secs(60) {
                debug!("Message {} dropped: already seen", message.message_id);
                return false;
            }
        }
        drop(seen);

        // Check if we're in the path (loop detection)
        if message.path.contains(&self.our_node_id) {
            debug!("Message {} dropped: loop detected", message.message_id);
            return false;
        }

        true
    }

    /// Mark message as seen
    pub async fn mark_seen(&self, message_id: &str) {
        let mut seen = self.seen_messages.write().await;
        seen.insert(message_id.to_string(), Instant::now());

        // Cleanup old entries (older than 5 minutes)
        seen.retain(|_, timestamp| timestamp.elapsed() < Duration::from_secs(300));
    }

    /// Check if message is for us
    pub fn is_for_us(&self, message: &MeshMessage) -> bool {
        match &message.to {
            None => true, // Broadcast
            Some(to) => to == &self.our_node_id,
        }
    }

    /// Prepare message for forwarding (decrement TTL, add to path)
    pub fn prepare_for_forwarding(&self, message: &MeshMessage) -> MeshMessage {
        let mut forward_msg = message.clone();
        forward_msg.ttl = forward_msg.ttl.saturating_sub(1);
        forward_msg.path.push(self.our_node_id.clone());
        forward_msg
    }

    /// Calculate routing score for a peer based on metrics (higher is better)
    /// Uses adaptive learning: score = α*old_score + β*new_score
    pub fn calculate_peer_score(
        peer_metrics: &crate::p2p::peer::PeerMetrics,
        route_stats: Option<&RouteStats>,
    ) -> f64 {
        // Latency score: lower latency = higher score (normalize to 0-1, assuming max 1s latency)
        let latency_score = peer_metrics
            .latency
            .map(|lat| {
                let lat_secs = lat.as_secs_f64();
                (1.0 - (lat_secs.min(1.0))).max(0.0)
            })
            .unwrap_or(0.5); // Default score if no latency data

        // Uptime score: longer uptime = higher score (normalize to 1 hour)
        let uptime_score = (peer_metrics.uptime.as_secs_f64() / 3600.0).min(1.0);

        // Reliability score: based on ping success rate
        let reliability = peer_metrics.reliability_score() as f64;

        // Route success rate from history
        let route_success_rate = if let Some(stats) = route_stats {
            let total = stats.success_count + stats.failure_count;
            if total > 0 {
                stats.success_count as f64 / total as f64
            } else {
                0.5
            }
        } else {
            0.5 // Default if no history
        };

        // Base score: 30% latency, 15% uptime, 30% reliability, 25% route success
        let base_score = 0.3 * latency_score
            + 0.15 * uptime_score
            + 0.3 * reliability
            + 0.25 * route_success_rate;

        // Adaptive learning: if we have previous score, blend it
        if let Some(stats) = route_stats {
            if stats.sample_count > 0 {
                let avg_latency = if stats.sample_count > 0 {
                    stats.total_latency.as_secs_f64() / stats.sample_count as f64
                } else {
                    0.0
                };
                let historical_score = (1.0 - (avg_latency.min(1.0))).max(0.0);

                // Exponential moving average: α=0.7 (old), β=0.3 (new)
                const ALPHA: f64 = 0.7;
                const BETA: f64 = 0.3;
                return ALPHA * historical_score + BETA * base_score;
            }
        }

        base_score
    }

    /// Get list of peers to forward to (flooding: all except sender)
    pub fn get_forward_peers(&self, message: &MeshMessage, all_peers: &[String]) -> Vec<String> {
        all_peers
            .iter()
            .filter(|peer_id| {
                // Don't forward to sender
                **peer_id != message.from &&
                // Don't forward to nodes already in path (loop prevention)
                !message.path.contains(peer_id)
            })
            .cloned()
            .collect()
    }

    /// Get best peers to forward to using UCB1 adaptive routing.
    ///
    /// Peer selection strategy:
    /// - **Warm-up** (selections < UCB1_MIN_SAMPLES): heuristic score + exploration bonus.
    ///   Unvisited peers receive the highest bonus, ensuring all peers are tried first.
    /// - **Exploitation** (selections >= UCB1_MIN_SAMPLES): pure UCB1 score.
    ///
    /// Returns peers sorted by score (best first), limited to top `max_peers`.
    pub async fn get_best_forward_peers(
        &self,
        message: &MeshMessage,
        peer_infos: &[crate::p2p::peer::PeerInfo],
        max_peers: usize,
    ) -> Vec<String> {
        let route_history = self.route_history.read().await;
        let ucb = self.ucb_state.read().await;

        let mut scored_peers: Vec<(String, f64)> = peer_infos
            .iter()
            .filter(|peer| {
                peer.node_id != message.from
                    && !message.path.contains(&peer.node_id)
                    && peer.is_connected()
            })
            .map(|peer| {
                let n_i = ucb.selections(&peer.node_id);
                let score = if n_i < UCB1_MIN_SAMPLES {
                    // Cold-start: heuristic score + exploration bonus.
                    // Unvisited peers (n_i == 0) get +1.0 so they are always tried first.
                    let heuristic =
                        Self::calculate_peer_score(&peer.metrics, route_history.get(&peer.node_id));
                    let bonus = if n_i == 0 { 1.0 } else { 0.5 };
                    heuristic + bonus
                } else {
                    ucb.ucb1_score(&peer.node_id)
                };
                (peer.node_id.clone(), score)
            })
            .collect();

        scored_peers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_peers
            .into_iter()
            .take(max_peers)
            .map(|(peer_id, _)| peer_id)
            .collect()
    }

    /// Destination-conditioned peer selection.
    ///
    /// Identical to [`Self::get_best_forward_peers`] except that the bandit state
    /// consulted is the one scoped to `dest`. A neighbour that is an excellent step
    /// toward one destination is often a poor step toward another, so scoring peers
    /// with a single destination-agnostic estimate discards the signal that actually
    /// determines routing quality.
    ///
    /// Warm-up behaviour is unchanged: until a peer has `UCB1_MIN_SAMPLES`
    /// observations *for this destination*, the heuristic score plus an exploration
    /// bonus is used, so unvisited peers are still tried first.
    pub async fn get_best_forward_peers_toward(
        &self,
        message: &MeshMessage,
        peer_infos: &[crate::p2p::peer::PeerInfo],
        max_peers: usize,
        dest: &str,
    ) -> Vec<String> {
        let route_history = self.route_history.read().await;
        let ucb = self.ucb_state.read().await;
        let empty = DestBandit::default();
        let bandit = ucb.by_dest.get(dest).unwrap_or(&empty);

        let mut scored_peers: Vec<(String, f64)> = peer_infos
            .iter()
            .filter(|peer| {
                peer.node_id != message.from
                    && !message.path.contains(&peer.node_id)
                    && peer.is_connected()
            })
            .map(|peer| {
                let n_i = bandit.selections(&peer.node_id);
                let score = if n_i < UCB1_MIN_SAMPLES {
                    let heuristic =
                        Self::calculate_peer_score(&peer.metrics, route_history.get(&peer.node_id));
                    let bonus = if n_i == 0 { 1.0 } else { 0.5 };
                    heuristic + bonus
                } else {
                    bandit.ucb1_score(&peer.node_id)
                };
                (peer.node_id.clone(), score)
            })
            .collect();

        scored_peers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_peers
            .into_iter()
            .take(max_peers)
            .map(|(peer_id, _)| peer_id)
            .collect()
    }

    /// Record a delivery outcome against the bandit scoped to `dest`.
    ///
    /// `success` carries the observed hop latency; `None` records a failure.
    /// Reward matches the destination-agnostic path: `clamp(1 - 2*latency, 0.5, 1.0)`
    /// on success, `0.0` on failure.
    pub async fn record_route_outcome_toward(
        &self,
        dest: &str,
        peer_id: &str,
        success: Option<Duration>,
    ) {
        let reward = match success {
            Some(latency) => (1.0 - 2.0 * latency.as_secs_f64()).clamp(0.5, 1.0),
            None => 0.0,
        };
        {
            let mut state = self.ucb_state.write().await;
            state
                .by_dest
                .entry(dest.to_string())
                .or_default()
                .record_reward(peer_id, reward);
        }
        self.persist_ucb_state().await;
    }

    /// Record successful route (for adaptive learning).
    /// Updates both the heuristic history and the UCB1 bandit state.
    /// Reward is latency-weighted: r = clamp(1 - 2*latency_secs, 0.5, 1.0).
    pub async fn record_route_success(&self, peer_id: &str, latency: Duration) {
        let mut history = self.route_history.write().await;
        let stats = history
            .entry(peer_id.to_string())
            .or_insert_with(|| RouteStats {
                success_count: 0,
                failure_count: 0,
                total_latency: Duration::ZERO,
                sample_count: 0,
                last_updated: Instant::now(),
            });

        stats.success_count += 1;
        stats.total_latency += latency;
        stats.sample_count += 1;
        stats.last_updated = Instant::now();
        drop(history);

        // UCB1: reward decreases with latency; clamped to [0.5, 1.0] for successful delivery.
        let reward = (1.0 - 2.0 * latency.as_secs_f64()).clamp(0.5, 1.0);
        self.ucb_state.write().await.record_reward(peer_id, reward);
        self.persist_ucb_state().await;
    }

    /// Record failed route (for adaptive learning).
    /// Updates both the heuristic history and the UCB1 bandit state (reward = 0).
    pub async fn record_route_failure(&self, peer_id: &str) {
        let mut history = self.route_history.write().await;
        let stats = history
            .entry(peer_id.to_string())
            .or_insert_with(|| RouteStats {
                success_count: 0,
                failure_count: 0,
                total_latency: Duration::ZERO,
                sample_count: 0,
                last_updated: Instant::now(),
            });

        stats.failure_count += 1;
        stats.last_updated = Instant::now();
        drop(history);

        // UCB1: failure → reward = 0.
        self.ucb_state.write().await.record_reward(peer_id, 0.0);
        self.persist_ucb_state().await;
    }

    /// Cleanup old cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.message_cache.write().await;
        cache.retain(|_, _msg| {
            // Keep messages that are less than 5 minutes old
            true // For now, keep all cached messages
        });
    }
}

impl Clone for Router {
    fn clone(&self) -> Self {
        Self {
            our_node_id: self.our_node_id.clone(),
            seen_messages: self.seen_messages.clone(),
            message_cache: self.message_cache.clone(),
            route_history: self.route_history.clone(),
            ucb_state: self.ucb_state.clone(),
            db: self.db.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elysium_address_parse() {
        let addr = ElysiumAddress::from_string("ely://node123").unwrap();
        assert_eq!(addr.node_id, "node123");

        let invalid = ElysiumAddress::from_string("invalid");
        assert!(invalid.is_err());
    }

    #[test]
    fn test_elysium_address_to_string() {
        let addr = ElysiumAddress {
            node_id: "node123".to_string(),
        };
        assert_eq!(addr.to_string(), "ely://node123");
    }

    #[tokio::test]
    async fn test_router_should_process() {
        let router = Router::new("our-node".to_string());
        let message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());

        // New message should be processed
        assert!(router.should_process(&message).await);

        // Mark as seen
        router.mark_seen(&message.message_id).await;

        // Should not process again immediately
        assert!(!router.should_process(&message).await);
    }

    #[tokio::test]
    async fn test_router_is_for_us() {
        let router = Router::new("our-node".to_string());

        // Broadcast message
        let broadcast = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        assert!(router.is_for_us(&broadcast));

        // Directed to us
        let directed = MeshMessage::new(
            "peer1".to_string(),
            Some("our-node".to_string()),
            b"test".to_vec(),
        );
        assert!(router.is_for_us(&directed));

        // Directed to someone else
        let other = MeshMessage::new(
            "peer1".to_string(),
            Some("other-node".to_string()),
            b"test".to_vec(),
        );
        assert!(!router.is_for_us(&other));
    }

    #[tokio::test]
    async fn test_router_prepare_for_forwarding() {
        let router = Router::new("our-node".to_string());
        let message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        let original_ttl = message.ttl;

        let forwarded = router.prepare_for_forwarding(&message);

        assert_eq!(forwarded.ttl, original_ttl - 1);
        assert!(forwarded.path.contains(&"our-node".to_string()));
    }

    #[tokio::test]
    async fn test_router_get_forward_peers() {
        let router = Router::new("our-node".to_string());
        let message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        let all_peers = vec![
            "peer1".to_string(),
            "peer2".to_string(),
            "peer3".to_string(),
        ];

        let forward_peers = router.get_forward_peers(&message, &all_peers);

        // Should not include sender (peer1) or our node
        assert!(!forward_peers.contains(&"peer1".to_string()));
        assert!(forward_peers.contains(&"peer2".to_string()));
        assert!(forward_peers.contains(&"peer3".to_string()));
    }

    #[tokio::test]
    async fn test_router_loop_detection() {
        let router = Router::new("our-node".to_string());
        let mut message = MeshMessage::new("peer1".to_string(), None, b"test".to_vec());
        message.path.push("our-node".to_string());

        // Should not process if we're in the path
        assert!(!router.should_process(&message).await);
    }
}
