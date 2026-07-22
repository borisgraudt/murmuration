//! Delay-tolerant routing over contact traces (the §6 "mobility" experiment).
//!
//! The main `benchmark` runs on a *static* graph. This one runs on a
//! time-varying **contact trace** with store-carry-forward semantics: a message
//! only advances when its current holders are in radio contact with someone, and
//! must otherwise be carried until the next useful meeting. That is the regime
//! delay-tolerant routing actually lives in, and where a static-graph result
//! could be an artefact.
//!
//! Strategies, from bound to bound:
//!
//! * `oracle`   — earliest-arrival foremost journey (`ContactTrace::earliest_arrival`).
//!                The exact optimum; nothing can deliver more, or sooner.
//! * `epidemic` — replicate to every contact. Practical delivery ceiling, but at
//!                the highest possible overhead.
//! * `prophet`  — PRoPHET (Lindgren et al., 2003): forward a copy to a peer only
//!                if the peer is *more likely to meet the destination*, estimated
//!                online from encounter history with ageing + transitivity. This
//!                is the DTN analogue of destination-conditioned routing — the arm
//!                that must learn the destination structure to do well.
//! * `direct`   — never relay; deliver only on meeting the destination. Lower
//!                bound; one transmission at most.
//!
//! Reports delivery ratio, mean delay over delivered messages, and transmissions
//! per message (overhead). Run: `cargo run --release --bin trace_bench`.

use murmuration::trace::ContactTrace;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;

// ─── Parameters ──────────────────────────────────────────────────────────────

const N_NODES: usize = 40;
const DURATION: f64 = 200_000.0; // seconds of trace
const ALPHA: f64 = 0.5; // power-law exponent for inter-contact gaps (Chaintreau)
const MEAN_CONTACT: f64 = 30.0;
const PAIR_ACTIVE: f64 = 0.5;
const N_MESSAGES: usize = 600;
/// TTL: deliver within this many seconds of creation. Override with
/// `TRACE_DEADLINE`. The discriminating regime is a few multiples of the mean
/// inter-contact gap — too long and everyone reaches 100%, too short and only
/// direct contacts matter.
fn deadline() -> f64 {
    std::env::var("TRACE_DEADLINE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2_000.0)
}
const SEEDS: &[u64] = &[1, 2, 3, 4, 5];
const ZIPF_S: f64 = 1.2; // concentrated destinations, matching the static study

// PRoPHET constants (paper defaults).
const P_ENCOUNTER: f64 = 0.75;
const P_BETA: f64 = 0.25; // transitivity weight
const P_GAMMA: f64 = 0.98; // ageing per aged time unit
const P_AGE_UNIT: f64 = 3_600.0; // seconds per ageing step

struct Msg {
    src: usize,
    dst: usize,
    gen: f64,
}

#[derive(Default, Clone)]
struct Stat {
    delivered: usize,
    total: usize,
    delay_sum: f64,
    transmissions: usize,
}

impl Stat {
    fn row(&self, name: &str) {
        let dr = 100.0 * self.delivered as f64 / self.total.max(1) as f64;
        let delay = if self.delivered > 0 {
            self.delay_sum / self.delivered as f64
        } else {
            f64::NAN
        };
        let tx = self.transmissions as f64 / self.total.max(1) as f64;
        println!("{name:<10} {dr:>8.1}% {delay:>14.0} {tx:>14.2}");
    }
}

fn zipf_destinations(n: usize, s: f64, rng: &mut StdRng) -> Vec<f64> {
    let mut cdf = Vec::with_capacity(n);
    let mut acc = 0.0;
    for r in 0..n {
        acc += 1.0 / ((r + 1) as f64).powf(s);
        cdf.push(acc);
    }
    for c in &mut cdf {
        *c /= acc;
    }
    // random rank→node permutation
    let mut perm: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        perm.swap(i, rng.gen_range(0..=i));
    }
    let _ = perm; // permutation applied by caller via sample()
    cdf
}

fn sample_dst(cdf: &[f64], perm: &[usize], rng: &mut StdRng) -> usize {
    let x: f64 = rng.gen();
    let rank = cdf.partition_point(|&c| c < x).min(cdf.len() - 1);
    perm[rank]
}

/// Oracle: earliest-arrival within the deadline.
fn run_oracle(trace: &ContactTrace, msgs: &[Msg]) -> Stat {
    let mut st = Stat {
        total: msgs.len(),
        ..Default::default()
    };
    for m in msgs {
        let arr = trace.earliest_arrival(m.src, m.gen);
        let a = arr[m.dst];
        if a.is_finite() && a - m.gen <= deadline() {
            st.delivered += 1;
            st.delay_sum += a - m.gen;
        }
        st.transmissions += 1; // informational
    }
    st
}

/// Epidemic and direct share a copy-set simulation; `relay_all` toggles between
/// "copy to everyone" (epidemic) and "only deliver to the destination" (direct).
fn run_replication(trace: &ContactTrace, msgs: &[Msg], relay_all: bool) -> Stat {
    let mut st = Stat {
        total: msgs.len(),
        ..Default::default()
    };
    for m in msgs {
        let mut holding = vec![false; trace.n];
        holding[m.src] = true;
        let mut delivered_at: Option<f64> = None;
        for c in &trace.contacts {
            if c.start < m.gen {
                continue;
            }
            if c.start - m.gen > deadline() {
                break;
            }
            let (ha, hb) = (holding[c.a], holding[c.b]);
            if ha == hb {
                continue; // both hold or neither holds → nothing to transfer
            }
            let peer = if ha { c.b } else { c.a };
            let useful = relay_all || peer == m.dst;
            if useful {
                holding[peer] = true;
                st.transmissions += 1;
                if peer == m.dst && delivered_at.is_none() {
                    delivered_at = Some(c.start);
                    if !relay_all {
                        break; // direct: done at first delivery
                    }
                }
            }
        }
        if let Some(t) = delivered_at {
            st.delivered += 1;
            st.delay_sum += t - m.gen;
        }
    }
    st
}

/// PRoPHET: destination-conditioned delivery predictability learned online.
///
/// State `p[node][dst]` is one global, continuously-ageing estimate shared by all
/// messages — the realistic setup. A copy moves from holder to peer when the peer
/// is more likely to eventually meet the destination.
fn run_prophet(trace: &ContactTrace, msgs: &[Msg]) -> Stat {
    let n = trace.n;
    let mut p = vec![vec![0.0f64; n]; n];
    let mut last_age = vec![0.0f64; n]; // per-pair ageing is approximated per node-row

    // Copies in flight: message index → set of holders. Only messages within their
    // live window are simulated; we inject/expire as the timeline advances.
    let mut holders: Vec<Vec<bool>> = msgs.iter().map(|_| vec![false; n]).collect();
    let mut done: Vec<Option<f64>> = vec![None; msgs.len()];
    let mut st = Stat {
        total: msgs.len(),
        ..Default::default()
    };

    // index messages by generation time for injection
    let mut by_gen: Vec<usize> = (0..msgs.len()).collect();
    by_gen.sort_by(|&a, &b| msgs[a].gen.partial_cmp(&msgs[b].gen).unwrap());
    let mut next_inject = 0usize;

    let age = |p: &mut Vec<Vec<f64>>, last_age: &mut Vec<f64>, node: usize, now: f64| {
        let steps = ((now - last_age[node]) / P_AGE_UNIT).floor();
        if steps > 0.0 {
            let factor = P_GAMMA.powf(steps);
            for x in p[node].iter_mut() {
                *x *= factor;
            }
            last_age[node] = now;
        }
    };

    for c in &trace.contacts {
        let now = c.start;
        // Inject messages whose gen time has arrived.
        while next_inject < by_gen.len() && msgs[by_gen[next_inject]].gen <= now {
            let mi = by_gen[next_inject];
            holders[mi][msgs[mi].src] = true;
            next_inject += 1;
        }

        let (a, b) = (c.a, c.b);
        age(&mut p, &mut last_age, a, now);
        age(&mut p, &mut last_age, b, now);

        // Encounter update: a and b just met.
        p[a][b] += (1.0 - p[a][b]) * P_ENCOUNTER;
        p[b][a] += (1.0 - p[b][a]) * P_ENCOUNTER;
        // Transitivity: meeting b tells a about b's contacts.
        for d in 0..n {
            if d != a && d != b {
                p[a][d] += (1.0 - p[a][d]) * p[a][b] * p[b][d] * P_BETA;
                p[b][d] += (1.0 - p[b][d]) * p[b][a] * p[a][d] * P_BETA;
            }
        }

        // Forwarding: for every live message, move a copy toward the better peer.
        for mi in 0..msgs.len() {
            if done[mi].is_some() {
                continue;
            }
            let m = &msgs[mi];
            if now < m.gen || now - m.gen > deadline() {
                continue;
            }
            let (ha, hb) = (holders[mi][a], holders[mi][b]);
            if ha == hb {
                continue;
            }
            let (holder, peer) = if ha { (a, b) } else { (b, a) };
            // Deliver on contact with the destination.
            if peer == m.dst {
                holders[mi][peer] = true;
                st.transmissions += 1;
                done[mi] = Some(now);
                continue;
            }
            // Otherwise relay only to a peer with higher predictability for dst.
            if p[peer][m.dst] > p[holder][m.dst] {
                holders[mi][peer] = true;
                st.transmissions += 1;
            }
        }
    }

    for (mi, d) in done.iter().enumerate() {
        if let Some(t) = d {
            st.delivered += 1;
            st.delay_sum += t - msgs[mi].gen;
        }
    }
    st
}

fn main() {
    let dl = deadline();
    println!(
        "DTN routing over synthetic contact traces (n={N_NODES}, power-law gaps α={ALPHA}, \
         Zipf destinations s={ZIPF_S}, deadline={dl:.0}s, {} seeds)\n",
        SEEDS.len()
    );
    println!("{:<10} {:>9} {:>14} {:>14}", "strategy", "delivery", "delay_s", "tx/msg");
    println!("{}", "─".repeat(52));

    // Accumulate across seeds.
    let mut acc: HashMap<&str, Stat> = HashMap::new();
    let mut mean_gap = 0.0;
    for &seed in SEEDS {
        let trace = ContactTrace::synthetic(N_NODES, DURATION, ALPHA, MEAN_CONTACT, PAIR_ACTIVE, seed);
        mean_gap += trace.mean_inter_contact();
        let mut rng = StdRng::seed_from_u64(seed ^ 0xD7);
        let cdf = zipf_destinations(N_NODES, ZIPF_S, &mut rng);
        let mut perm: Vec<usize> = (0..N_NODES).collect();
        for i in (1..N_NODES).rev() {
            perm.swap(i, rng.gen_range(0..=i));
        }
        let msgs: Vec<Msg> = (0..N_MESSAGES)
            .map(|_| {
                let src = rng.gen_range(0..N_NODES);
                let dst = loop {
                    let d = sample_dst(&cdf, &perm, &mut rng);
                    if d != src {
                        break d;
                    }
                };
                let gen = rng.gen::<f64>() * (DURATION - deadline()).max(1.0);
                Msg { src, dst, gen }
            })
            .collect();

        for (name, st) in [
            ("oracle", run_oracle(&trace, &msgs)),
            ("epidemic", run_replication(&trace, &msgs, true)),
            ("prophet", run_prophet(&trace, &msgs)),
            ("direct", run_replication(&trace, &msgs, false)),
        ] {
            let e = acc.entry(name).or_default();
            e.delivered += st.delivered;
            e.total += st.total;
            e.delay_sum += st.delay_sum;
            e.transmissions += st.transmissions;
        }
    }

    for name in ["oracle", "epidemic", "prophet", "direct"] {
        acc[name].row(name);
    }
    println!(
        "\nmean inter-contact gap ≈ {:.0}s (heavy-tailed; floor {:.0}s)",
        mean_gap / SEEDS.len() as f64,
        MEAN_CONTACT
    );
}
