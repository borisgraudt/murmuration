//! Routing benchmark: UCB1 (as shipped) vs flooding, random, heuristic, and an oracle.
//!
//! This harness drives the **real** `meshlink_core::ai::router::Router` — the same
//! code paths a live node uses — rather than reimplementing UCB1. That is the whole
//! point: the numbers describe the shipped router, not a copy of it.
//!
//! # Model
//!
//! Each undirected link `(u,v)` carries two latent properties:
//!
//! * `delivery_prob ∈ [p_min, 1.0]` — probability a single transmission survives the link.
//! * `latency_ms` — propagation delay.
//!
//! Latency is **observable** (a node can ping a neighbour), so it is exposed through
//! `PeerMetrics::latency` and every strategy sees it. `delivery_prob` is **latent**:
//! nothing reports it, and a router can only estimate it by trying and observing
//! outcomes. That asymmetry is what makes this a bandit problem rather than a
//! shortest-path problem — and it is why the heuristic arm exists, to separate
//! "uses latency sensibly" from "learns reliability".
//!
//! Ping statistics are supplied as a deliberately *noisy* proxy for link quality,
//! matching reality: a 20-byte ping surviving says little about whether a full
//! message will.
//!
//! # Regret
//!
//! For a fixed destination `d`, define the optimal delivery probability from node `u`:
//!
//! ```text
//! V(d) = 1
//! V(u) = max_{v ∈ N(u)} p(u,v) · V(v)
//! ```
//!
//! computed exactly by value iteration. A node at `u` choosing next hop `v` incurs
//! instantaneous regret `V(u) − p(u,v)·V(v) ≥ 0`. Cumulative regret over all routing
//! decisions is the headline bandit metric: sublinear growth means the router is
//! learning.
//!
//! Regret is defined only for next-hop strategies. Flooding makes no choice, so its
//! regret cell is empty rather than zero.
//!
//! # Output
//!
//! * `results/benchmark.csv`      — one row per (strategy, condition, seed)
//! * `results/learning_curve.csv` — cumulative regret vs decision index
//!
//! Run: `cargo run --release --bin benchmark`

use meshlink_core::ai::router::{MeshMessage, Router};
use meshlink_core::p2p::peer::{ConnectionState, PeerInfo, PeerMetrics};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

// ─── Parameters ──────────────────────────────────────────────────────────────

/// Messages per run. Override with `BENCH_MESSAGES` to test convergence:
/// destination-conditioned methods need far more traffic than peer-keyed ones,
/// because the reward signal is split across `n` destinations.
fn n_messages() -> usize {
    std::env::var("BENCH_MESSAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2_000)
}
const TTL: usize = 12;
const SEEDS: &[u64] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

/// Subset of seeds to use; `BENCH_SEEDS=3` runs only the first three.
fn seeds() -> &'static [u64] {
    let k = std::env::var("BENCH_SEEDS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(SEEDS.len())
        .min(SEEDS.len());
    &SEEDS[..k]
}

/// Restrict to a single `n_nodes` value via `BENCH_NODES`.
fn node_filter() -> Option<usize> {
    std::env::var("BENCH_NODES").ok().and_then(|v| v.parse().ok())
}

/// Suffix for output files via `BENCH_TAG`, so successive runs do not clobber
/// each other's CSVs (e.g. `BENCH_TAG=zipf1.2` → `results/benchmark_zipf1.2.csv`).
fn tag() -> String {
    std::env::var("BENCH_TAG")
        .map(|t| format!("_{t}"))
        .unwrap_or_default()
}

/// Zipf exponent for destination popularity, via `BENCH_ZIPF`. `0.0` is uniform.
///
/// Uniform destinations are the worst case for any destination-conditioned
/// method: they maximise the number of (node, destination) pairs that must be
/// learned. Real messenger traffic is concentrated — most messages go to a
/// handful of contacts — so `s ≈ 1.0` is the more representative condition.
fn zipf_s() -> f64 {
    std::env::var("BENCH_ZIPF")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0)
}

/// Destination sampler with a Zipf-distributed popularity profile.
///
/// Rank→node is a random permutation per run, so the popular destinations are
/// not always the same node indices and cannot interact with graph generation.
struct DestSampler {
    /// Cumulative weights over ranks.
    cdf: Vec<f64>,
    /// rank → node id.
    rank_to_node: Vec<usize>,
}

impl DestSampler {
    fn new(n: usize, s: f64, rng: &mut StdRng) -> Self {
        let mut rank_to_node: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = rng.gen_range(0..=i);
            rank_to_node.swap(i, j);
        }
        let mut cdf = Vec::with_capacity(n);
        let mut acc = 0.0;
        for rank in 0..n {
            acc += 1.0 / ((rank + 1) as f64).powf(s);
            cdf.push(acc);
        }
        let total = acc;
        for c in &mut cdf {
            *c /= total;
        }
        Self { cdf, rank_to_node }
    }

    fn sample(&self, rng: &mut StdRng) -> usize {
        let x = rng.gen::<f64>();
        let rank = self.cdf.partition_point(|&c| c < x).min(self.cdf.len() - 1);
        self.rank_to_node[rank]
    }
}
const LATENCY_MIN_MS: f64 = 5.0;
const LATENCY_MAX_MS: f64 = 200.0;
/// Worst link survives only 35% of transmissions — enough spread that choosing well matters.
const P_MIN: f64 = 0.35;
/// Ping proxy noise: observed reliability = true ± this, clamped.
const PING_NOISE: f64 = 0.25;

// ─── Network ─────────────────────────────────────────────────────────────────

struct Network {
    n: usize,
    neighbours: Vec<Vec<usize>>,
    /// delivery_prob[u][v] — latent, symmetric.
    quality: HashMap<(usize, usize), f64>,
    /// latency_ms[u][v] — observable, symmetric.
    latency: HashMap<(usize, usize), f64>,
    /// Noisy ping-derived reliability estimate, observable.
    ping_est: HashMap<(usize, usize), f64>,
}

impl Network {
    /// Erdős–Rényi G(n,p) plus a random spanning tree, so the graph is always connected
    /// and delivery failures are attributable to link quality rather than partition.
    fn generate(n: usize, link_prob: f64, rng: &mut StdRng) -> Self {
        let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); n];
        let add = |a: usize, b: usize, nbrs: &mut Vec<Vec<usize>>| {
            if !nbrs[a].contains(&b) {
                nbrs[a].push(b);
                nbrs[b].push(a);
            }
        };

        for i in 0..n {
            for j in (i + 1)..n {
                if rng.gen::<f64>() < link_prob {
                    add(i, j, &mut neighbours);
                }
            }
        }

        let mut perm: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = rng.gen_range(0..=i);
            perm.swap(i, j);
        }
        for i in 1..n {
            let a = perm[i];
            let b = perm[rng.gen_range(0..i)];
            add(a, b, &mut neighbours);
        }

        let mut quality = HashMap::new();
        let mut latency = HashMap::new();
        let mut ping_est = HashMap::new();
        for u in 0..n {
            for &v in &neighbours[u] {
                if u < v {
                    let q = rng.gen_range(P_MIN..1.0);
                    let l = rng.gen_range(LATENCY_MIN_MS..LATENCY_MAX_MS);
                    let noise = rng.gen_range(-PING_NOISE..PING_NOISE);
                    let est = (q + noise).clamp(0.0, 1.0);
                    for key in [(u, v), (v, u)] {
                        quality.insert(key, q);
                        latency.insert(key, l);
                        ping_est.insert(key, est);
                    }
                }
            }
        }

        Self {
            n,
            neighbours,
            quality,
            latency,
            ping_est,
        }
    }

    fn q(&self, u: usize, v: usize) -> f64 {
        *self.quality.get(&(u, v)).unwrap_or(&0.0)
    }
    fn lat(&self, u: usize, v: usize) -> f64 {
        *self.latency.get(&(u, v)).unwrap_or(&0.0)
    }

    /// The quantity UCB1's `avg_reward` converges to for arm `v` at node `u`:
    /// `E[reward] = p(u,v) · clamp(1 − 2·latency_s, 0.5, 1.0)`.
    ///
    /// A policy that knows this exactly is the best any peer-keyed bandit can
    /// become, so it upper-bounds the whole destination-agnostic class.
    fn expected_reward(&self, u: usize, v: usize) -> f64 {
        let shaped = (1.0 - 2.0 * (self.lat(u, v) / 1000.0)).clamp(0.5, 1.0);
        self.q(u, v) * shaped
    }

    /// Exact optimal delivery probability to `dst` from every node, by value iteration.
    /// Converges in at most `n` sweeps because every step multiplies by p < 1.
    fn oracle_values(&self, dst: usize) -> Vec<f64> {
        let mut v = vec![0.0f64; self.n];
        v[dst] = 1.0;
        for _ in 0..self.n {
            let mut changed = false;
            for u in 0..self.n {
                if u == dst {
                    continue;
                }
                let best = self.neighbours[u]
                    .iter()
                    .map(|&w| self.q(u, w) * v[w])
                    .fold(0.0f64, f64::max);
                if best > v[u] + 1e-12 {
                    v[u] = best;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        v
    }

    /// Build the `PeerInfo` list a node at `u` would hold for its neighbours.
    /// Carries only observable signals: measured latency and noisy ping statistics.
    fn peer_infos(&self, u: usize, exclude: &[usize]) -> Vec<PeerInfo> {
        self.neighbours[u]
            .iter()
            .filter(|v| !exclude.contains(v))
            .map(|&v| {
                let addr: SocketAddr = format!("127.0.0.1:{}", 10_000 + v).parse().unwrap();
                let mut info = PeerInfo::new(node_name(v), addr);
                info.state = ConnectionState::Connected;
                let est = *self.ping_est.get(&(u, v)).unwrap_or(&0.5);
                let pings = 20u32;
                let ok = (est * pings as f64).round() as u32;
                info.metrics = PeerMetrics {
                    latency: Some(Duration::from_secs_f64(self.lat(u, v) / 1000.0)),
                    uptime: Duration::from_secs(3600),
                    ping_count: ok,
                    ping_failures: pings - ok,
                    last_ping: None,
                };
                info
            })
            .collect()
    }
}

fn node_name(i: usize) -> String {
    format!("n{i}")
}

fn name_to_index(name: &str) -> Option<usize> {
    name.strip_prefix('n')?.parse().ok()
}

// ─── Strategies ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Strategy {
    Flooding,
    Random,
    Heuristic,
    Ucb1,
    Ucb1Dest,
    /// Exact asymptote of UCB1: knows `p(u,v)·clamp(1−2·lat,0.5,1)` perfectly but
    /// cannot condition on the destination. Upper-bounds any policy whose state is
    /// keyed by peer alone.
    AgnosticLimit,
    /// Q-routing (Boyan & Littman, 1994) — value bootstrapping from neighbours.
    QRouting,
    Oracle,
}

impl Strategy {
    fn name(self) -> &'static str {
        match self {
            Strategy::Flooding => "flooding",
            Strategy::Random => "random",
            Strategy::Heuristic => "heuristic",
            Strategy::Ucb1 => "ucb1",
            Strategy::Ucb1Dest => "ucb1_dest",
            Strategy::AgnosticLimit => "agnostic_limit",
            Strategy::QRouting => "q_routing",
            Strategy::Oracle => "oracle",
        }
    }
    /// Flooding makes no per-hop choice, so regret is undefined for it.
    fn has_regret(self) -> bool {
        self != Strategy::Flooding
    }
}

// ─── Q-routing ───────────────────────────────────────────────────────────────

/// Q-routing learning rate.
const Q_ALPHA: f64 = 0.15;
/// Exploration rate for the ε-greedy behaviour policy.
const Q_EPSILON: f64 = 0.05;
/// Optimistic initialisation: unseen actions look maximally good, so every
/// neighbour is tried before the estimates settle.
const Q_INIT: f64 = 1.0;

/// `Q[u][(dest, neighbour)]` — node `u`'s estimate of the probability that a
/// message for `dest` handed to `neighbour` eventually arrives.
///
/// Unlike a bandit, the update target is not a terminal reward but the
/// *neighbour's own estimate* — this is the bootstrapping step that carries
/// destination information backwards through the graph.
struct QTables {
    tables: Vec<HashMap<(usize, usize), f64>>,
}

impl QTables {
    fn new(n: usize) -> Self {
        Self {
            tables: vec![HashMap::new(); n],
        }
    }

    fn get(&self, u: usize, dest: usize, v: usize) -> f64 {
        *self.tables[u].get(&(dest, v)).unwrap_or(&Q_INIT)
    }

    /// `max_w Q_v(dest, w)` — the value `v` would advertise for `dest`.
    /// A node sitting on the destination advertises certainty.
    fn best(&self, net: &Network, v: usize, dest: usize) -> f64 {
        if v == dest {
            return 1.0;
        }
        net.neighbours[v]
            .iter()
            .map(|&w| self.get(v, dest, w))
            .fold(0.0f64, f64::max)
    }

    fn update(&mut self, u: usize, dest: usize, v: usize, target: f64) {
        let cur = self.get(u, dest, v);
        self.tables[u]
            .insert((dest, v), (1.0 - Q_ALPHA) * cur + Q_ALPHA * target);
    }
}

#[derive(Default)]
struct Outcome {
    delivered: usize,
    total: usize,
    hops: usize,
    latency_ms: f64,
    transmissions: usize,
    regret: f64,
    /// Cumulative regret sampled every `CURVE_STRIDE` decisions.
    curve: Vec<(usize, f64)>,
}

const CURVE_STRIDE: usize = 25;

/// Route one message with a next-hop strategy. Returns transmissions used.
#[allow(clippy::too_many_arguments)]
async fn route_unicast(
    net: &Network,
    routers: &[Router],
    strategy: Strategy,
    src: usize,
    dst: usize,
    rng: &mut StdRng,
    out: &mut Outcome,
    decisions: &mut usize,
    qt: &mut QTables,
) {
    let values = net.oracle_values(dst);
    let mut current = src;
    let mut visited = vec![src];
    let mut hops = 0usize;
    let mut latency = 0.0f64;
    let msg = MeshMessage::new(node_name(src), Some(node_name(dst)), b"payload".to_vec());

    while hops < TTL {
        let candidates: Vec<usize> = net.neighbours[current]
            .iter()
            .copied()
            .filter(|v| !visited.contains(v))
            .collect();
        if candidates.is_empty() {
            break;
        }

        let next = match strategy {
            Strategy::Random => candidates[rng.gen_range(0..candidates.len())],
            Strategy::Oracle => *candidates
                .iter()
                .max_by(|&&a, &&b| {
                    (net.q(current, a) * values[a])
                        .partial_cmp(&(net.q(current, b) * values[b]))
                        .unwrap()
                })
                .unwrap(),
            Strategy::Heuristic => {
                // Real scoring function, but no bandit state: latency-aware, not learning.
                let infos = net.peer_infos(current, &visited);
                let best = infos.iter().max_by(|a, b| {
                    Router::calculate_peer_score(&a.metrics, None)
                        .partial_cmp(&Router::calculate_peer_score(&b.metrics, None))
                        .unwrap()
                });
                match best.and_then(|p| name_to_index(&p.node_id)) {
                    Some(v) => v,
                    None => break,
                }
            }
            Strategy::Ucb1 => {
                // The shipped selection path, including warm-up and persistence hooks.
                let infos = net.peer_infos(current, &visited);
                let picked = routers[current]
                    .get_best_forward_peers(&msg, &infos, 1)
                    .await;
                match picked.first().and_then(|id| name_to_index(id)) {
                    Some(v) => v,
                    None => break,
                }
            }
            Strategy::Ucb1Dest => {
                // Same router, same warm-up, but bandit state keyed by destination.
                let infos = net.peer_infos(current, &visited);
                let picked = routers[current]
                    .get_best_forward_peers_toward(&msg, &infos, 1, &node_name(dst))
                    .await;
                match picked.first().and_then(|id| name_to_index(id)) {
                    Some(v) => v,
                    None => break,
                }
            }
            Strategy::AgnosticLimit => *candidates
                .iter()
                .max_by(|&&a, &&b| {
                    net.expected_reward(current, a)
                        .partial_cmp(&net.expected_reward(current, b))
                        .unwrap()
                })
                .unwrap(),
            Strategy::QRouting => {
                if rng.gen::<f64>() < Q_EPSILON {
                    candidates[rng.gen_range(0..candidates.len())]
                } else {
                    *candidates
                        .iter()
                        .max_by(|&&a, &&b| {
                            qt.get(current, dst, a)
                                .partial_cmp(&qt.get(current, dst, b))
                                .unwrap()
                        })
                        .unwrap()
                }
            }
            Strategy::Flooding => unreachable!("flooding handled separately"),
        };

        // Regret is charged for the decision, independent of the coin flip that follows.
        if strategy.has_regret() {
            let inst = (values[current] - net.q(current, next) * values[next]).max(0.0);
            out.regret += inst;
            *decisions += 1;
            if *decisions % CURVE_STRIDE == 0 {
                out.curve.push((*decisions, out.regret));
            }
        }

        out.transmissions += 1;
        let hop_latency = net.lat(current, next);
        let survived = rng.gen::<f64>() < net.q(current, next);

        match strategy {
            Strategy::Ucb1 => {
                if survived {
                    routers[current]
                        .record_route_success(
                            &node_name(next),
                            Duration::from_secs_f64(hop_latency / 1000.0),
                        )
                        .await;
                } else {
                    routers[current].record_route_failure(&node_name(next)).await;
                }
            }
            Strategy::Ucb1Dest => {
                let observed =
                    survived.then(|| Duration::from_secs_f64(hop_latency / 1000.0));
                routers[current]
                    .record_route_outcome_toward(&node_name(dst), &node_name(next), observed)
                    .await;
            }
            Strategy::QRouting => {
                // Bootstrap: target is the neighbour's own estimate, gated by whether
                // the hop actually survived. E[s · V_v] = p(u,v) · V_v, which is
                // exactly the quantity the oracle maximises.
                let target = if survived {
                    qt.best(net, next, dst)
                } else {
                    0.0
                };
                qt.update(current, dst, next, target);
            }
            _ => {}
        }

        if !survived {
            return; // transmission lost; message dropped
        }

        hops += 1;
        latency += hop_latency;
        visited.push(next);
        current = next;

        if current == dst {
            out.delivered += 1;
            out.hops += hops;
            out.latency_ms += latency;
            return;
        }
    }
}

/// Flooding: every node forwards to every neighbour it has not already forwarded to.
/// Delivery succeeds if any copy survives; latency is that of the fastest surviving copy.
fn route_flooding(net: &Network, src: usize, dst: usize, rng: &mut StdRng, out: &mut Outcome) {
    // (node, hops, latency) frontier; best[u] = cheapest surviving arrival seen at u.
    let mut best: HashMap<usize, (usize, f64)> = HashMap::new();
    best.insert(src, (0, 0.0));
    let mut frontier = vec![src];

    for _ in 0..TTL {
        let mut next_frontier = Vec::new();
        for &u in &frontier {
            let (uh, ul) = best[&u];
            for &v in &net.neighbours[u] {
                out.transmissions += 1;
                if rng.gen::<f64>() >= net.q(u, v) {
                    continue; // this copy died on this link
                }
                let cand = (uh + 1, ul + net.lat(u, v));
                let better = match best.get(&v) {
                    Some(&(_, l)) => cand.1 < l,
                    None => true,
                };
                if better {
                    best.insert(v, cand);
                    next_frontier.push(v);
                }
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        frontier = next_frontier;
    }

    if let Some(&(h, l)) = best.get(&dst) {
        out.delivered += 1;
        out.hops += h;
        out.latency_ms += l;
    }
}

async fn run(net: &Network, strategy: Strategy, seed: u64) -> Outcome {
    let mut rng = StdRng::seed_from_u64(seed ^ 0x5eed);
    let routers: Vec<Router> = (0..net.n).map(|i| Router::new(node_name(i))).collect();
    let n_msg = n_messages();
    let mut out = Outcome {
        total: n_msg,
        ..Default::default()
    };
    let mut decisions = 0usize;
    let mut qt = QTables::new(net.n);
    let sampler = DestSampler::new(net.n, zipf_s(), &mut rng);

    for _ in 0..n_msg {
        let src = rng.gen_range(0..net.n);
        let dst = loop {
            let d = sampler.sample(&mut rng);
            if d != src {
                break d;
            }
        };

        if strategy == Strategy::Flooding {
            route_flooding(net, src, dst, &mut rng, &mut out);
        } else {
            route_unicast(
                net,
                &routers,
                strategy,
                src,
                dst,
                &mut rng,
                &mut out,
                &mut decisions,
                &mut qt,
            )
            .await;
        }
    }
    out
}

// ─── Statistics ──────────────────────────────────────────────────────────────

/// Mean and half-width of the 95% CI (normal approximation; 10 seeds).
fn mean_ci(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    if xs.len() < 2 {
        return (mean, 0.0);
    }
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, 1.96 * (var / n).sqrt())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let conditions: &[(usize, f64)] = &[
        (10, 0.30),
        (20, 0.20),
        (50, 0.10),
        (50, 0.20),
        (100, 0.06),
    ];
    let strategies = [
        Strategy::Flooding,
        Strategy::Random,
        Strategy::Heuristic,
        Strategy::Ucb1,
        Strategy::Ucb1Dest,
        Strategy::AgnosticLimit,
        Strategy::QRouting,
        Strategy::Oracle,
    ];

    fs::create_dir_all("results")?;
    let mut csv = fs::File::create(format!("results/benchmark{}.csv", tag()))?;
    writeln!(
        csv,
        "strategy,n_nodes,link_prob,seed,delivery_ratio,avg_hops,avg_latency_ms,transmissions_per_msg,cumulative_regret"
    )?;
    let mut curve_csv = fs::File::create(format!("results/learning_curve{}.csv", tag()))?;
    writeln!(curve_csv, "strategy,n_nodes,link_prob,seed,decisions,cumulative_regret")?;

    println!(
        "{:<10} {:>6} {:>6} {:>18} {:>14} {:>16} {:>14} {:>18}",
        "strategy", "nodes", "p", "delivery", "hops", "latency_ms", "tx/msg", "regret"
    );

    let only = node_filter();
    for &(n, link_prob) in conditions {
        if only.is_some_and(|k| k != n) {
            continue;
        }
        println!("{}", "─".repeat(110));
        for strat in strategies {
            let (mut dr, mut hp, mut lt, mut tx, mut rg) =
                (vec![], vec![], vec![], vec![], vec![]);

            for &seed in seeds() {
                let mut net_rng = StdRng::seed_from_u64(seed);
                let net = Network::generate(n, link_prob, &mut net_rng);
                let out = run(&net, strat, seed).await;

                let delivery = out.delivered as f64 / out.total as f64;
                let hops = if out.delivered > 0 {
                    out.hops as f64 / out.delivered as f64
                } else {
                    f64::NAN
                };
                let latency = if out.delivered > 0 {
                    out.latency_ms / out.delivered as f64
                } else {
                    f64::NAN
                };
                let txpm = out.transmissions as f64 / out.total as f64;

                writeln!(
                    csv,
                    "{},{},{},{},{:.4},{:.3},{:.2},{:.3},{}",
                    strat.name(),
                    n,
                    link_prob,
                    seed,
                    delivery,
                    hops,
                    latency,
                    txpm,
                    if strat.has_regret() {
                        format!("{:.3}", out.regret)
                    } else {
                        String::new()
                    }
                )?;

                for (d, r) in &out.curve {
                    writeln!(
                        curve_csv,
                        "{},{},{},{},{},{:.4}",
                        strat.name(),
                        n,
                        link_prob,
                        seed,
                        d,
                        r
                    )?;
                }

                dr.push(delivery);
                if hops.is_finite() {
                    hp.push(hops);
                }
                if latency.is_finite() {
                    lt.push(latency);
                }
                tx.push(txpm);
                if strat.has_regret() {
                    rg.push(out.regret);
                }
            }

            let (d, dci) = mean_ci(&dr);
            let (h, _) = mean_ci(&hp);
            let (l, _) = mean_ci(&lt);
            let (t, _) = mean_ci(&tx);
            let regret_cell = if rg.is_empty() {
                "—".to_string()
            } else {
                let (r, rci) = mean_ci(&rg);
                format!("{r:.1} ± {rci:.1}")
            };

            println!(
                "{:<10} {:>6} {:>6.2} {:>11.1}% ± {:.1} {:>14.2} {:>16.1} {:>14.2} {:>18}",
                strat.name(),
                n,
                link_prob,
                d * 100.0,
                dci * 100.0,
                h,
                l,
                t,
                regret_cell
            );
        }
    }

    println!("\nWrote results/benchmark{}.csv and results/learning_curve{}.csv", tag(), tag());
    Ok(())
}
