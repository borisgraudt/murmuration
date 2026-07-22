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
    fn synthetic_thins_pairs() {
        // With low activity probability, far fewer than all C(n,2) pairs appear.
        let t = ContactTrace::synthetic(40, 50_000.0, 0.5, 30.0, 0.1, 3);
        let pairs: std::collections::HashSet<(usize, usize)> =
            t.contacts.iter().map(|c| (c.a, c.b)).collect();
        let all_pairs = 40 * 39 / 2;
        assert!(pairs.len() < all_pairs / 2, "expected sparse pair set, got {}", pairs.len());
    }
}
