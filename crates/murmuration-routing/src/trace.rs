//! Contact traces for delay-tolerant routing evaluation.
//!
//! The routing benchmark's first model was a *static* Erdős–Rényi graph. Real
//! mesh networks are not static: nodes meet intermittently, and the interesting
//! property of human mobility — the one that makes delay-tolerant routing hard —
//! is that inter-contact times are **heavy-tailed** (Chaintreau et al., "Impact
//! of Human Mobility on Opportunistic Forwarding Algorithms", IEEE TMC 2007).
//!
//! This module defines a single contact-trace format and two producers:
//!
//! * [`ContactTrace::load_csv`] — read a real trace (CRAWDAD / Infocom / Reality
//!   Mining, exported as `start,end,a,b` rows in seconds).
//! * [`ContactTrace::synthetic`] — generate a reproducible trace whose
//!   inter-contact times follow a truncated power law, so the benchmark is
//!   runnable and reproducible with no gated dataset download, while a real
//!   trace drops in through the *same* type.
//!
//! A trace is a list of undirected contact intervals `[start, end)` between two
//! node indices. Routing over it is store-carry-forward: a message can only move
//! from `u` to `v` during an interval in which `u` and `v` are in contact.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// One pairwise contact: nodes `a` and `b` are connected during `[start, end)`.
/// `a < b` by convention so a contact has one canonical representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contact {
    pub start: f64,
    pub end: f64,
    pub a: usize,
    pub b: usize,
}

/// A set of pairwise contacts over `n` nodes, sorted by start time.
#[derive(Debug, Clone)]
pub struct ContactTrace {
    pub n: usize,
    pub duration: f64,
    pub contacts: Vec<Contact>,
}

impl ContactTrace {
    /// Parse a CRAWDAD-style CSV: one `start,end,a,b` row per contact, times in
    /// seconds, node ids as 0-based integers. A leading non-numeric header row
    /// is skipped. This is the format real Infocom/Haggle traces are exported to.
    pub fn load_csv(text: &str) -> Result<Self, String> {
        let mut contacts = Vec::new();
        let mut max_node = 0usize;
        let mut duration = 0.0f64;
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split(',').map(str::trim).collect();
            if cols.len() < 4 {
                return Err(format!("line {}: expected 4 columns, got {}", i + 1, cols.len()));
            }
            // Skip a header row (first field not a number).
            if i == 0 && cols[0].parse::<f64>().is_err() {
                continue;
            }
            let start: f64 = cols[0].parse().map_err(|_| format!("line {}: bad start", i + 1))?;
            let end: f64 = cols[1].parse().map_err(|_| format!("line {}: bad end", i + 1))?;
            let a: usize = cols[2].parse().map_err(|_| format!("line {}: bad node a", i + 1))?;
            let b: usize = cols[3].parse().map_err(|_| format!("line {}: bad node b", i + 1))?;
            if a == b {
                continue; // self-contact is meaningless
            }
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            max_node = max_node.max(b);
            duration = duration.max(end);
            contacts.push(Contact { start, end, a, b });
        }
        if contacts.is_empty() {
            return Err("no contacts parsed".into());
        }
        contacts.sort_by(|x, y| x.start.partial_cmp(&y.start).unwrap());
        Ok(Self { n: max_node + 1, duration, contacts })
    }

    /// Generate a reproducible synthetic trace over `n` nodes for `duration`
    /// seconds. Each unordered pair meets repeatedly; the gaps between successive
    /// contacts are drawn from a **truncated power law** with exponent `alpha`
    /// (Chaintreau et al. report `alpha ≈ 0.5` for real human traces), which is
    /// the heavy-tailed property static graphs miss. `mean_contact` sets how long
    /// a meeting lasts.
    ///
    /// Not every pair is socially active; `pair_active_prob` thins the pair set so
    /// the contact graph has community structure rather than being complete.
    pub fn synthetic(
        n: usize,
        duration: f64,
        alpha: f64,
        mean_contact: f64,
        pair_active_prob: f64,
        seed: u64,
    ) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut contacts = Vec::new();
        // Minimum inter-contact gap (seconds); the power law is truncated below
        // this so contacts do not collapse onto each other.
        let gap_min = mean_contact.max(1.0);
        let gap_max = duration; // truncation ceiling

        for a in 0..n {
            for b in (a + 1)..n {
                if rng.gen::<f64>() >= pair_active_prob {
                    continue; // this pair never meets
                }
                let mut t = rng.gen::<f64>() * gap_min; // random phase
                loop {
                    let gap = sample_truncated_power_law(&mut rng, alpha, gap_min, gap_max);
                    t += gap;
                    if t >= duration {
                        break;
                    }
                    // Contact length: exponential around mean_contact.
                    let len = -mean_contact * (1.0 - rng.gen::<f64>()).ln();
                    let end = (t + len).min(duration);
                    contacts.push(Contact { start: t, end, a, b });
                    t = end;
                }
            }
        }
        contacts.sort_by(|x, y| x.start.partial_cmp(&y.start).unwrap());
        Self { n, duration, contacts }
    }

    /// Earliest-arrival time at every node for a message created at `src` at
    /// time `t0` (the *foremost journey*, Bui-Xuan et al., 2003). This is the
    /// exact optimum any delay-tolerant routing could achieve, so it is the
    /// oracle the trace-driven benchmark compares practical strategies against.
    ///
    /// `f64::INFINITY` means unreachable from `src` after `t0`. Contacts are
    /// processed in start-time order; a contact `[s,e)` relays a message that
    /// reached a holder by time `e`, arriving at the peer at `max(s, holder)`.
    /// Store-carry-forward is implicit: a node holds the message until a useful
    /// contact appears.
    pub fn earliest_arrival(&self, src: usize, t0: f64) -> Vec<f64> {
        let mut arrival = vec![f64::INFINITY; self.n];
        arrival[src] = t0;
        for c in &self.contacts {
            if c.end < t0 {
                continue;
            }
            // Snapshot both holders *before* relaxing, so a message cannot ride
            // a→b then b→a within the same contact.
            let (ra, rb) = (arrival[c.a], arrival[c.b]);
            if ra <= c.end {
                let arrive = c.start.max(ra);
                if arrive <= c.end && arrive < arrival[c.b] {
                    arrival[c.b] = arrive;
                }
            }
            if rb <= c.end {
                let arrive = c.start.max(rb);
                if arrive <= c.end && arrive < arrival[c.a] {
                    arrival[c.a] = arrive;
                }
            }
        }
        arrival
    }

    /// Empirical mean inter-contact time across all active pairs — a summary
    /// statistic used to check a trace looks heavy-tailed rather than periodic.
    pub fn mean_inter_contact(&self) -> f64 {
        use std::collections::HashMap;
        let mut last: HashMap<(usize, usize), f64> = HashMap::new();
        let mut gaps = Vec::new();
        for c in &self.contacts {
            if let Some(&prev_end) = last.get(&(c.a, c.b)) {
                gaps.push(c.start - prev_end);
            }
            last.insert((c.a, c.b), c.end);
        }
        if gaps.is_empty() {
            0.0
        } else {
            gaps.iter().sum::<f64>() / gaps.len() as f64
        }
    }
}

/// Inverse-CDF sample from a power law `p(x) ∝ x^{-(1+alpha)}` truncated to
/// `[lo, hi]`. `alpha > 0`. Heavy tail: most gaps are near `lo`, but rare gaps
/// span the whole trace — exactly the property that stresses DTN routing.
fn sample_truncated_power_law(rng: &mut StdRng, alpha: f64, lo: f64, hi: f64) -> f64 {
    let u: f64 = rng.gen();
    // CDF inversion for p(x) ∝ x^{-(1+alpha)} on [lo, hi].
    let lo_a = lo.powf(-alpha);
    let hi_a = hi.powf(-alpha);
    (lo_a - u * (lo_a - hi_a)).powf(-1.0 / alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_csv_parses_and_orders() {
        let csv = "start,end,a,b\n30,40,1,0\n0,10,0,2\n";
        let t = ContactTrace::load_csv(csv).unwrap();
        assert_eq!(t.n, 3);
        assert_eq!(t.duration, 40.0);
        // sorted by start; (1,0) canonicalised to (0,1)
        assert_eq!(t.contacts[0].start, 0.0);
        assert_eq!((t.contacts[1].a, t.contacts[1].b), (0, 1));
    }

    #[test]
    fn load_csv_rejects_empty() {
        assert!(ContactTrace::load_csv("\n\n").is_err());
    }

    #[test]
    fn synthetic_is_reproducible() {
        let a = ContactTrace::synthetic(20, 10_000.0, 0.5, 30.0, 0.4, 7);
        let b = ContactTrace::synthetic(20, 10_000.0, 0.5, 30.0, 0.4, 7);
        assert_eq!(a.contacts.len(), b.contacts.len());
        assert_eq!(a.contacts.first(), b.contacts.first());
    }

    #[test]
    fn synthetic_has_heavy_tailed_gaps() {
        // A power-law gap distribution has mean well above its floor because rare
        // huge gaps drag it up; a periodic trace would sit near the floor.
        let t = ContactTrace::synthetic(30, 200_000.0, 0.5, 30.0, 0.5, 1);
        let mean_gap = t.mean_inter_contact();
        assert!(
            mean_gap > 30.0 * 5.0,
            "expected heavy tail to lift mean gap well above the floor, got {mean_gap}"
        );
    }

    #[test]
    fn earliest_arrival_follows_temporal_path() {
        // 0—1 during [0,10], 1—2 during [20,30], 1—2 during [5,8].
        // From 0 at t=0: reach 1 at 0. Reach 2 only via the [20,30] contact
        // (the [5,8] one is usable too: holder 1 ready at 0 ≤ 8 → arrive 5).
        let csv = "0,10,0,1\n20,30,1,2\n5,8,1,2\n";
        let tr = ContactTrace::load_csv(csv).unwrap();
        let arr = tr.earliest_arrival(0, 0.0);
        assert_eq!(arr[0], 0.0);
        assert_eq!(arr[1], 0.0);
        assert_eq!(arr[2], 5.0, "should take the earliest usable 1—2 contact");
    }

    #[test]
    fn earliest_arrival_respects_time_order() {
        // 1—2 happens BEFORE 0—1, so a message from 0 can never reach 2:
        // by the time 0 reaches 1 (t=20), the 1—2 contact [5,8] is long gone.
        let csv = "5,8,1,2\n20,30,0,1\n";
        let tr = ContactTrace::load_csv(csv).unwrap();
        let arr = tr.earliest_arrival(0, 0.0);
        assert_eq!(arr[1], 20.0);
        assert!(arr[2].is_infinite(), "no forward-time path 0→2 exists");
    }

    #[test]
    fn synthetic_thins_pairs() {
        // With low activity probability, far fewer than all C(n,2) pairs appear.
        let t = ContactTrace::synthetic(40, 50_000.0, 0.5, 30.0, 0.1, 3);
        let pairs: std::collections::HashSet<(usize, usize)> =
            t.contacts.iter().map(|c| (c.a, c.b)).collect();
        let all_pairs = 40 * 39 / 2;
        assert!(pairs.len() < all_pairs / 2, "expected sparse pair set, got {}", pairs.len());
    }
}
