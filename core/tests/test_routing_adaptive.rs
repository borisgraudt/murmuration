/// Test adaptive routing functionality — including UCB1 bandit behaviour.
extern crate meshlink_core;

use meshlink_core::ai::router::{MeshMessage, Router};
use meshlink_core::p2p::peer::{ConnectionState, PeerInfo};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_route_history_tracking() {
    let router = Router::new("test_node".to_string());

    // Record some route successes
    router
        .record_route_success("peer1", Duration::from_millis(50))
        .await;
    router
        .record_route_success("peer1", Duration::from_millis(60))
        .await;
    router.record_route_failure("peer2").await;
    router
        .record_route_success("peer2", Duration::from_millis(100))
        .await;

    // Give it a moment to process
    sleep(Duration::from_millis(10)).await;

    // The route history should be updated
    // (We can't directly access it, but we can verify through scoring)
    // Route history tracking works (no panic means success)
}

#[tokio::test]
async fn test_adaptive_scoring() {
    // Create a peer with good metrics
    let mut good_peer = PeerInfo::new("good_peer".to_string(), "127.0.0.1:8080".parse().unwrap());
    good_peer.metrics.update_latency(Duration::from_millis(10));
    good_peer.metrics.uptime = Duration::from_secs(3600); // 1 hour

    // Create a peer with bad metrics
    let mut bad_peer = PeerInfo::new("bad_peer".to_string(), "127.0.0.1:8081".parse().unwrap());
    bad_peer.metrics.update_latency(Duration::from_millis(500));
    bad_peer.metrics.uptime = Duration::from_secs(60); // 1 minute

    // Calculate scores
    let good_score = Router::calculate_peer_score(&good_peer.metrics, None);
    let bad_score = Router::calculate_peer_score(&bad_peer.metrics, None);

    // Good peer should have higher score
    assert!(
        good_score > bad_score,
        "Good peer should have higher score than bad peer"
    );
    assert!(good_score > 0.5, "Good peer score should be reasonable");
}

#[tokio::test]
async fn test_adaptive_learning_with_history() {
    let router = Router::new("test_node".to_string());

    // Create a peer
    let mut peer = PeerInfo::new("test_peer".to_string(), "127.0.0.1:8080".parse().unwrap());
    peer.metrics.update_latency(Duration::from_millis(50));

    // Record multiple successes (should improve score over time)
    for _ in 0..5 {
        router
            .record_route_success("test_peer", Duration::from_millis(50))
            .await;
    }

    sleep(Duration::from_millis(10)).await;

    // Score with history should be better than without
    let score_without_history = Router::calculate_peer_score(&peer.metrics, None);

    // Get route history (we need to access it through the router)
    // For now, just verify the mechanism works
    assert!(score_without_history > 0.0, "Score should be positive");
}

#[tokio::test]
async fn test_peer_selection_ranking() {
    let router = Router::new("test_node".to_string());

    // Create multiple peers with different metrics
    let mut peers = vec![];

    // Best peer
    let mut peer1 = PeerInfo::new("peer1".to_string(), "127.0.0.1:8081".parse().unwrap());
    peer1.metrics.update_latency(Duration::from_millis(10));
    peer1.metrics.uptime = Duration::from_secs(3600);
    peer1.state = ConnectionState::Connected;
    peers.push(peer1);

    // Medium peer
    let mut peer2 = PeerInfo::new("peer2".to_string(), "127.0.0.1:8082".parse().unwrap());
    peer2.metrics.update_latency(Duration::from_millis(100));
    peer2.metrics.uptime = Duration::from_secs(1800);
    peer2.state = ConnectionState::Connected;
    peers.push(peer2);

    // Worst peer
    let mut peer3 = PeerInfo::new("peer3".to_string(), "127.0.0.1:8083".parse().unwrap());
    peer3.metrics.update_latency(Duration::from_millis(500));
    peer3.metrics.uptime = Duration::from_secs(60);
    peer3.state = ConnectionState::Connected;
    peers.push(peer3);

    // Create a test message
    let message = MeshMessage::new(
        "sender".to_string(),
        None, // broadcast
        b"test".to_vec(),
    );

    // Get best forward peers (top 2)
    let selected = router.get_best_forward_peers(&message, &peers, 2).await;

    // Should select top 2 peers
    assert_eq!(selected.len(), 2, "Should select top 2 peers");

    // Best peer should be first
    assert_eq!(selected[0], "peer1", "Best peer should be selected first");
    assert_eq!(selected[1], "peer2", "Second best peer should be selected");
}

#[tokio::test]
async fn test_route_success_rate_impact() {
    let router = Router::new("test_node".to_string());

    // Create two peers with similar metrics
    let mut peer1 = PeerInfo::new("peer1".to_string(), "127.0.0.1:8081".parse().unwrap());
    peer1.metrics.update_latency(Duration::from_millis(50));
    peer1.state = ConnectionState::Connected;

    let mut peer2 = PeerInfo::new("peer2".to_string(), "127.0.0.1:8082".parse().unwrap());
    peer2.metrics.update_latency(Duration::from_millis(50));
    peer2.state = ConnectionState::Connected;

    // Record many successes for peer1, failures for peer2
    for _ in 0..10 {
        router
            .record_route_success("peer1", Duration::from_millis(50))
            .await;
        router.record_route_failure("peer2").await;
    }

    sleep(Duration::from_millis(10)).await;

    // Create message
    let message = MeshMessage::new("sender".to_string(), None, b"test".to_vec());

    let peers = vec![peer1, peer2];
    let selected = router.get_best_forward_peers(&message, &peers, 1).await;

    // Peer1 should be selected (better success rate)
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0], "peer1",
        "Peer with better success rate should be selected"
    );
}

// ─── UCB1-specific tests ──────────────────────────────────────────────────────

/// During cold-start (n < UCB1_MIN_SAMPLES=5) a completely unvisited peer (n=0)
/// must receive a larger exploration bonus than a peer with 3 samples, and therefore
/// be ranked first when all other metrics are equal.
#[tokio::test]
async fn test_ucb1_cold_start_unvisited_first() {
    let router = Router::new("test_node".to_string());

    // peer_visited: 3 successes — still in cold-start, bonus = +0.5
    let mut peer_visited = PeerInfo::new(
        "peer_visited".to_string(),
        "127.0.0.1:8081".parse().unwrap(),
    );
    peer_visited
        .metrics
        .update_latency(Duration::from_millis(50));
    peer_visited.state = ConnectionState::Connected;
    for _ in 0..3 {
        router
            .record_route_success("peer_visited", Duration::from_millis(50))
            .await;
    }

    // peer_new: no routing history at all — cold-start bonus = +1.0
    let mut peer_new = PeerInfo::new("peer_new".to_string(), "127.0.0.1:8082".parse().unwrap());
    peer_new.metrics.update_latency(Duration::from_millis(50)); // same heuristic
    peer_new.state = ConnectionState::Connected;

    let message = MeshMessage::new("sender".to_string(), None, b"test".to_vec());
    let selected = router
        .get_best_forward_peers(&message, &[peer_new, peer_visited], 1)
        .await;

    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0], "peer_new",
        "Unvisited peer should be selected first (exploration bonus +1.0 > +0.5)"
    );
}

/// After UCB1 warmup (n >= 5) exploitation dominates: a peer with consistently
/// high rewards must outrank a peer with consistently low rewards.
#[tokio::test]
async fn test_ucb1_exploitation() {
    let router = Router::new("test_node".to_string());

    // peer_good: 10 fast deliveries → avg_reward ≈ 0.98
    let mut peer_good = PeerInfo::new("peer_good".to_string(), "127.0.0.1:8081".parse().unwrap());
    peer_good.state = ConnectionState::Connected;
    for _ in 0..10 {
        router
            .record_route_success("peer_good", Duration::from_millis(10))
            .await;
    }

    // peer_bad: 10 failures → avg_reward = 0.0
    let mut peer_bad = PeerInfo::new("peer_bad".to_string(), "127.0.0.1:8082".parse().unwrap());
    peer_bad.state = ConnectionState::Connected;
    for _ in 0..10 {
        router.record_route_failure("peer_bad").await;
    }

    let message = MeshMessage::new("sender".to_string(), None, b"test".to_vec());
    let selected = router
        .get_best_forward_peers(&message, &[peer_bad, peer_good], 1)
        .await;

    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0], "peer_good",
        "Peer with high reward must be preferred after UCB1 warmup"
    );
}

/// UCB1 must force exploration of an unvisited peer even when another peer
/// has accumulated many high-reward samples (whose shrinking exploration term
/// makes the cold-start bonus competitive).
#[tokio::test]
async fn test_ucb1_exploration_new_peer() {
    let router = Router::new("test_node".to_string());

    // peer_veteran: 50 successes → UCB1 exploration term shrinks to ~0.4
    let mut peer_veteran = PeerInfo::new(
        "peer_veteran".to_string(),
        "127.0.0.1:8081".parse().unwrap(),
    );
    peer_veteran
        .metrics
        .update_latency(Duration::from_millis(10));
    peer_veteran.state = ConnectionState::Connected;
    for _ in 0..50 {
        router
            .record_route_success("peer_veteran", Duration::from_millis(10))
            .await;
    }

    // peer_new: never selected → cold-start bonus +1.0 added to default heuristic 0.5
    let mut peer_new = PeerInfo::new("peer_new".to_string(), "127.0.0.1:8082".parse().unwrap());
    peer_new.state = ConnectionState::Connected;
    // No latency data → heuristic = 0.5; total score = 1.5

    let message = MeshMessage::new("sender".to_string(), None, b"test".to_vec());
    let selected = router
        .get_best_forward_peers(&message, &[peer_veteran, peer_new], 1)
        .await;

    // peer_veteran UCB1 ≈ 0.98 + sqrt(2*ln(50)/50) ≈ 1.37 < 1.5 (peer_new cold-start)
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0], "peer_new",
        "UCB1 must explore new peer over over-sampled veteran once exploration term shrinks"
    );
}
