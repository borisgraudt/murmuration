/// Routing simulation: UCB1 vs Flooding vs Random on synthetic mesh topologies.
///
/// Outputs CSV to stdout:
///   strategy,n_nodes,link_prob,fail_rate,seed,delivery_ratio,avg_hops,avg_latency_ms,overhead_ratio
///
/// Run:
///   cargo run --release --bin simulate > results/simulation.csv
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet, VecDeque};

// ─── UCB1 constants (must match router.rs) ───────────────────────────────────
const UCB1_C: f64 = 2.0;
const UCB1_MIN_SAMPLES: u64 = 5;

// ─── Simulation parameters ───────────────────────────────────────────────────
const N_MESSAGES: usize = 500;
const DEFAULT_TTL: usize = 12;
const HOP_LATENCY_MIN_MS: f64 = 5.0;
const HOP_LATENCY_MAX_MS: f64 = 80.0;

// ─── Network ─────────────────────────────────────────────────────────────────

/// Adjacency list representation of a network.
#[derive(Clone)]
struct Network {
    n: usize,
    /// neighbours[i] = list of node indices reachable from i
    neighbours: Vec<Vec<usize>>,
}

impl Network {
    /// Erdős–Rényi G(n, link_prob) with guaranteed connectivity (spanning-tree backbone).
    fn erdos_renyi(n: usize, link_prob: f64, rng: &mut StdRng) -> Self {
        let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); n];

        // Random edges
        for i in 0..n {
            for j in (i + 1)..n {
                if rng.gen::<f64>() < link_prob {
                    neighbours[i].push(j);
                    neighbours[j].push(i);
                }
            }
        }

        // Spanning-tree backbone to guarantee connectivity
        let mut perm: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = rng.gen_range(0..=i);
            perm.swap(i, j);
        }
        for i in 1..n {
            let a = perm[i];
            let b = perm[rng.gen_range(0..i)];
            if !neighbours[a].contains(&b) {
                neighbours[a].push(b);
                neighbours[b].push(a);
            }
        }

        // Deduplicate
        for nbrs in &mut neighbours {
            nbrs.sort_unstable();
            nbrs.dedup();
        }

        Self { n, neighbours }
    }

    /// Remove each edge independently with probability `fail_rate`.
    fn apply_failures(&self, fail_rate: f64, rng: &mut StdRng) -> Self {
        let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); self.n];
        let mut seen: HashSet<(usize, usize)> = HashSet::new();

        for i in 0..self.n {
            for &j in &self.neighbours[i] {
                let key = if i < j { (i, j) } else { (j, i) };
                if seen.contains(&key) {
                    continue;
                }
                seen.insert(key);
                if rng.gen::<f64>() >= fail_rate {
                    neighbours[i].push(j);
                    neighbours[j].push(i);
                }
            }
        }

        // Keep at least the spanning-tree backbone alive (re-add if severed)
        // (simplification: just try to keep the network non-trivially connected)
        Self {
            n: self.n,
            neighbours,
        }
    }

    /// Return BFS shortest-path hop count from src to dst, or None if unreachable.
    fn bfs_distance(&self, src: usize, dst: usize) -> Option<usize> {
        if src == dst {
            return Some(0);
        }
        let mut visited = vec![false; self.n];
        let mut queue = VecDeque::new();
        visited[src] = true;
        queue.push_back((src, 0usize));
        while let Some((node, dist)) = queue.pop_front() {
            for &nbr in &self.neighbours[node] {
                if nbr == dst {
                    return Some(dist + 1);
                }
                if !visited[nbr] {
                    visited[nbr] = true;
                    queue.push_back((nbr, dist + 1));
                }
            }
        }
        None
    }
}

// ─── UCB1 bandit state per node ──────────────────────────────────────────────

#[derive(Default, Clone)]
struct UcbPeer {
    selections: u64,
    avg_reward: f64,
}

/// Global bandit state: node_id → (neighbour_id → UCB1 stats)
type BanditState = Vec<HashMap<usize, UcbPeer>>;

fn ucb1_score(peer: &UcbPeer, total_selections: u64) -> f64 {
    if peer.selections == 0 {
        return f64::INFINITY;
    }
    if peer.selections < UCB1_MIN_SAMPLES {
        return peer.avg_reward + 0.5; // warm-up bonus
    }
    let exploration = if total_selections > 0 {
        (UCB1_C * (total_selections as f64).ln() / peer.selections as f64).sqrt()
    } else {
        0.0
    };
    peer.avg_reward + exploration
}

fn ucb1_record_reward(
    bandit: &mut BanditState,
    node: usize,
    neighbour: usize,
    reward: f64,
) {
    let state = bandit[node].entry(neighbour).or_default();
    state.selections += 1;
    state.avg_reward += (reward - state.avg_reward) / state.selections as f64;
}

/// Node-level total selections (denominator for UCB1 exploration term).
fn total_selections(bandit: &BanditState, node: usize) -> u64 {
    bandit[node].values().map(|s| s.selections).sum()
}

// ─── Routing strategies ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Strategy {
    Flooding,
    Random,
    Ucb1,
}

impl Strategy {
    fn name(&self) -> &'static str {
        match self {
            Strategy::Flooding => "flooding",
            Strategy::Random => "random",
            Strategy::Ucb1 => "ucb1",
        }
    }
}

struct SimResult {
    delivered: usize,
    total: usize,
    total_hops: usize,
    total_latency_ms: f64,
    /// Total message-copies forwarded (measures network overhead; flooding > UCB1)
    total_transmissions: usize,
}

/// Simulate N_MESSAGES deliveries and return aggregate metrics.
fn simulate(
    net: &Network,
    strategy: Strategy,
    rng: &mut StdRng,
) -> SimResult {
    let mut bandit: BanditState = vec![HashMap::new(); net.n];

    let mut delivered = 0usize;
    let mut total_hops = 0usize;
    let mut total_latency_ms = 0.0f64;
    let mut total_transmissions = 0usize;

    for _ in 0..N_MESSAGES {
        // Pick random (src, dst) pair
        let src = rng.gen_range(0..net.n);
        let dst = loop {
            let d = rng.gen_range(0..net.n);
            if d != src {
                break d;
            }
        };

        match strategy {
            Strategy::Flooding => {
                // BFS flood — count first arrival
                let (ok, hops, txs) = simulate_flooding(net, src, dst);
                if ok {
                    delivered += 1;
                    let hop_latency =
                        hops as f64 * rng.gen_range(HOP_LATENCY_MIN_MS..HOP_LATENCY_MAX_MS);
                    total_hops += hops;
                    total_latency_ms += hop_latency;
                }
                total_transmissions += txs;
            }
            Strategy::Random => {
                let (ok, hops, txs) = simulate_random(net, src, dst, rng);
                if ok {
                    delivered += 1;
                    let hop_latency =
                        hops as f64 * rng.gen_range(HOP_LATENCY_MIN_MS..HOP_LATENCY_MAX_MS);
                    total_hops += hops;
                    total_latency_ms += hop_latency;
                }
                total_transmissions += txs;
            }
            Strategy::Ucb1 => {
                let (ok, hops, txs, path_latency_ms) =
                    simulate_ucb1(net, src, dst, &mut bandit, rng);
                if ok {
                    delivered += 1;
                    total_hops += hops;
                    total_latency_ms += path_latency_ms;
                }
                total_transmissions += txs;
            }
        }
    }

    SimResult {
        delivered,
        total: N_MESSAGES,
        total_hops,
        total_latency_ms,
        total_transmissions,
    }
}

/// Flooding: BFS from src to dst. Returns (delivered, hops, transmissions).
fn simulate_flooding(net: &Network, src: usize, dst: usize) -> (bool, usize, usize) {
    if let Some(d) = net.bfs_distance(src, dst) {
        // transmissions = all edges reachable within d hops (BFS frontier sizes)
        // Approximate: sum of frontier sizes
        let mut visited = vec![false; net.n];
        let mut queue = VecDeque::new();
        let mut txs = 0usize;
        visited[src] = true;
        queue.push_back((src, 0usize));
        while let Some((node, depth)) = queue.pop_front() {
            if depth >= d {
                break;
            }
            for &nbr in &net.neighbours[node] {
                if !visited[nbr] {
                    visited[nbr] = true;
                    txs += 1;
                    queue.push_back((nbr, depth + 1));
                }
            }
        }
        (true, d, txs.max(1))
    } else {
        (false, 0, 0)
    }
}

/// Random walk: at each hop pick a random unvisited neighbour (except sender).
/// Returns (delivered, hops, transmissions).
fn simulate_random(
    net: &Network,
    src: usize,
    dst: usize,
    rng: &mut StdRng,
) -> (bool, usize, usize) {
    let mut visited: HashSet<usize> = HashSet::new();
    let mut current = src;
    visited.insert(current);
    let mut hops = 0usize;

    while hops < DEFAULT_TTL {
        // Candidates: unvisited neighbours
        let candidates: Vec<usize> = net.neighbours[current]
            .iter()
            .filter(|&&n| !visited.contains(&n))
            .copied()
            .collect();

        if candidates.is_empty() {
            break;
        }

        let next = candidates[rng.gen_range(0..candidates.len())];
        hops += 1;
        visited.insert(next);

        if next == dst {
            return (true, hops, hops);
        }
        current = next;
    }

    (false, 0, hops)
}

/// UCB1 routing: each node selects best next-hop by UCB1 score.
/// Returns (delivered, hops, transmissions, path_latency_ms).
fn simulate_ucb1(
    net: &Network,
    src: usize,
    dst: usize,
    bandit: &mut BanditState,
    rng: &mut StdRng,
) -> (bool, usize, usize, f64) {
    let mut path: Vec<usize> = vec![src];
    let mut current = src;
    let mut path_latency_ms = 0.0f64;
    let mut hops = 0usize;

    while hops < DEFAULT_TTL {
        // Select best next-hop by UCB1 (excluding already-visited nodes)
        let total = total_selections(bandit, current);
        let visited_set: HashSet<usize> = path.iter().copied().collect();

        let candidates: Vec<usize> = net.neighbours[current]
            .iter()
            .filter(|&&n| !visited_set.contains(&n))
            .copied()
            .collect();

        if candidates.is_empty() {
            break;
        }

        // Pick best by UCB1 score (ties broken by index = deterministic in warmup)
        let next = candidates
            .iter()
            .map(|&n| {
                let peer = bandit[current].get(&n).cloned().unwrap_or_default();
                let score = ucb1_score(&peer, total);
                (n, score)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(n, _)| n)
            .unwrap();

        let hop_latency = rng.gen_range(HOP_LATENCY_MIN_MS..HOP_LATENCY_MAX_MS);
        path_latency_ms += hop_latency;
        hops += 1;
        path.push(next);

        if next == dst {
            // Update bandit: reward = clamp(1 - 2 * latency_secs, 0.5, 1.0)
            for w in path.windows(2) {
                let latency_secs: f64 = hop_latency / 1000.0;
                let reward: f64 = (1.0 - 2.0 * latency_secs).clamp(0.5, 1.0);
                ucb1_record_reward(bandit, w[0], w[1], reward);
            }
            return (true, hops, hops, path_latency_ms);
        }
        current = next;
    }

    // Delivery failed — penalise last hop if path has ≥ 2 nodes
    if path.len() >= 2 {
        let last = path.len() - 1;
        ucb1_record_reward(bandit, path[last - 1], path[last], 0.0);
    }

    (false, 0, hops, 0.0)
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    // CSV header
    println!("strategy,n_nodes,link_prob,fail_rate,seed,delivery_ratio,avg_hops,avg_latency_ms,overhead_ratio");

    let node_counts = [20, 50, 100];
    let link_probs = [0.15, 0.25]; // Erdős–Rényi edge probability
    let fail_rates = [0.0, 0.1, 0.2, 0.3]; // per-link failure probability
    let seeds: [u64; 5] = [42, 137, 256, 999, 31415];
    let strategies = [Strategy::Flooding, Strategy::Random, Strategy::Ucb1];

    for &n in &node_counts {
        for &link_prob in &link_probs {
            for &fail_rate in &fail_rates {
                for &seed in &seeds {
                    let mut rng = StdRng::seed_from_u64(seed);

                    // Build base topology
                    let base_net = Network::erdos_renyi(n, link_prob, &mut rng);

                    // Apply link failures (same RNG state, so each (seed, fail_rate) is deterministic)
                    let mut rng2 = StdRng::seed_from_u64(seed ^ 0xdeadbeef);
                    let net = base_net.apply_failures(fail_rate, &mut rng2);

                    // Compute flooding overhead as reference for overhead_ratio
                    let mut rng_flood = StdRng::seed_from_u64(seed ^ 0x1234);
                    let flood_result = simulate(&net, Strategy::Flooding, &mut rng_flood);
                    let flood_txs = flood_result.total_transmissions.max(1) as f64;

                    for &strategy in &strategies {
                        let mut rng_s = StdRng::seed_from_u64(seed ^ 0x5678 ^ strategy as u64);
                        let r = simulate(&net, strategy, &mut rng_s);

                        let delivery_ratio = r.delivered as f64 / r.total as f64;
                        let avg_hops = if r.delivered > 0 {
                            r.total_hops as f64 / r.delivered as f64
                        } else {
                            0.0
                        };
                        let avg_latency_ms = if r.delivered > 0 {
                            r.total_latency_ms / r.delivered as f64
                        } else {
                            0.0
                        };
                        let overhead_ratio = r.total_transmissions as f64 / flood_txs;

                        println!(
                            "{},{},{:.2},{:.1},{},{:.4},{:.2},{:.1},{:.4}",
                            strategy.name(),
                            n,
                            link_prob,
                            fail_rate,
                            seed,
                            delivery_ratio,
                            avg_hops,
                            avg_latency_ms,
                            overhead_ratio,
                        );
                    }
                }
            }
        }
    }
}
