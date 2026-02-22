/// Criterion benchmarks for the Elysium routing subsystem.
///
/// Run with:
///   cargo bench --bench routing
///
/// Results are saved to target/criterion/routing/
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use meshlink_core::ai::router::{MeshMessage, Router};
use meshlink_core::p2p::peer::{ConnectionState, PeerInfo};
use std::time::Duration;
use tokio::runtime::Runtime;

// ── helpers ──────────────────────────────────────────────────────────────────

fn make_connected_peer(id: &str, port: u16, latency_ms: u64, uptime_secs: u64) -> PeerInfo {
    let mut peer = PeerInfo::new(id.to_string(), format!("127.0.0.1:{port}").parse().unwrap());
    peer.metrics.update_latency(Duration::from_millis(latency_ms));
    peer.metrics.uptime = Duration::from_secs(uptime_secs);
    peer.state = ConnectionState::Connected;
    peer
}

fn make_peers(n: usize) -> Vec<PeerInfo> {
    (0..n)
        .map(|i| {
            make_connected_peer(
                &format!("peer_{i}"),
                9000 + i as u16,
                10 + (i as u64 * 7) % 200, // latency 10–200 ms, deterministic
                3600 + i as u64 * 60,
            )
        })
        .collect()
}

fn broadcast_msg(from: &str) -> MeshMessage {
    MeshMessage::new(from.to_string(), None, b"benchmark payload".to_vec())
}

// ── benchmarks ───────────────────────────────────────────────────────────────

/// Throughput of `calculate_peer_score` — pure CPU, no async.
/// Uses `None` for route_stats (cold-start, most common hot path).
fn bench_peer_score(c: &mut Criterion) {
    let peer = make_connected_peer("p0", 9000, 25, 3600);

    c.bench_function("calculate_peer_score", |b| {
        b.iter(|| {
            black_box(Router::calculate_peer_score(
                black_box(&peer.metrics),
                black_box(None),
            ))
        })
    });
}

/// `get_best_forward_peers` with varying fleet sizes (cold-start, no UCB1 history).
fn bench_cold_start_selection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let msg = broadcast_msg("external_sender");

    let mut group = c.benchmark_group("cold_start_selection");
    for n in [4usize, 8, 16, 24] {
        let peers = make_peers(n);
        let router = Router::new("bench_node".to_string());

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(
                        router
                            .get_best_forward_peers(black_box(&msg), black_box(&peers), 3)
                            .await,
                    )
                })
            })
        });
    }
    group.finish();
}

/// `get_best_forward_peers` after UCB1 warmup (all peers have ≥ 5 samples).
fn bench_ucb1_selection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let msg = broadcast_msg("external_sender");

    let mut group = c.benchmark_group("ucb1_selection");
    for n in [4usize, 8, 16, 24] {
        let peers = make_peers(n);
        let router = Router::new("bench_node".to_string());

        // Warm up UCB1 state: record 10 successes / failures alternating per peer
        rt.block_on(async {
            for peer in &peers {
                for j in 0..10u32 {
                    if j % 3 == 0 {
                        router.record_route_failure(&peer.node_id).await;
                    } else {
                        router
                            .record_route_success(&peer.node_id, Duration::from_millis(20))
                            .await;
                    }
                }
            }
        });

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(
                        router
                            .get_best_forward_peers(black_box(&msg), black_box(&peers), 3)
                            .await,
                    )
                })
            })
        });
    }
    group.finish();
}

/// Round-trip: mark_seen + should_process for deduplication hot-path.
fn bench_dedup(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let router = Router::new("bench_node".to_string());
    let msg = broadcast_msg("sender");

    // Pre-seed seen set
    rt.block_on(async {
        router.mark_seen(&msg.message_id).await;
    });

    c.bench_function("should_process_duplicate", |b| {
        b.iter(|| {
            rt.block_on(async { black_box(router.should_process(black_box(&msg)).await) })
        })
    });
}

// ── criterion boilerplate ─────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_peer_score,
    bench_cold_start_selection,
    bench_ucb1_selection,
    bench_dedup,
);
criterion_main!(benches);
