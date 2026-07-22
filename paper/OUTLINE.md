# The Destination-Agnostic Ceiling in Bandit Mesh Routing

Working paper outline. Target: a networking or ML systems **workshop** (CoNEXT
student workshop, IMC poster, or ICLR Tiny Papers depending on framing). ~6 pages.
This file maps each section to the evidence that already exists in `results/`,
so writing is filling prose around settled numbers — not new experiments.

Status tags: [done] evidence exists · [todo] experiment to run · [write] prose to write

--- ## Abstract
Bandit-based next-hop selection is common in adaptive mesh/DTN routing. We show
its ceiling is structural: the reward a relay can observe is destination-agnostic,
and we derive the exact best achievable delivery rate of any peer-keyed policy.
The shipped UCB1 router already sits at that ceiling, ~6× below a
destination-aware oracle. We show value bootstrapping (Q-routing) breaks the
ceiling — ~2× the delivery rate under realistic concentrated traffic — and that
the result is robust to hyperparameters.

## 1. Introduction
- Adaptive mesh routing picks next hops from observed outcomes; bandits (UCB1)
  are a natural, popular choice, and are what elysium/nyx shipped.
- Contribution: (i) an exact upper bound on the whole peer-keyed class
  (`agnostic_limit`); (ii) empirical proof UCB1 saturates it; (iii) Q-routing as
  the fix, with a significant win under concentrated traffic; (iv) a hyperparameter
  robustness study.
- Honest framing: this began as "does UCB1 beat static baselines?" — the answer
  is no, and the *why* is the paper.

## 2. Background & Related Work
- UCB1 (Auer et al., 2002); bandit routing in mesh/DTN literature.
- Q-routing (Boyan & Littman, 1994) — value bootstrapping.
- Human mobility & inter-contact times (Chaintreau et al., 2007) → motivates the
  trace-driven evaluation in §6.
- DTN store-carry-forward.

## 3. System & Threat-Free Model — [done] (`RESULTS.md` "Method")
- Links carry observable latency and **latent** delivery probability; reliability
  is learnable only by trying — this asymmetry makes it a learning problem.
- The benchmark drives the **real shipped Router**, not a reimplementation.

## 4. The Destination-Agnostic Ceiling — [done] core theoretical contribution
- UCB1's `avg_reward` converges to `p(u,v)·shaped_latency`; a policy that knows
  this exactly upper-bounds every peer-keyed method (`agnostic_limit`).
- Oracle by value iteration gives the achievable optimum.
- **Fig 1** (`fig1_ceiling`): UCB1 tracks the ceiling; band above it is unreachable.
- Result: ceiling is ~6× below oracle (finding 2); UCB1 attains 57–87% of it
  (finding 1); regret is linear (finding 4, `fig5_regret`).

## 5. Q-routing Breaks the Ceiling — [done], [todo] live-network validation pending
- Bootstrapping from a neighbour's advertised value carries destination info
  backward; not a bandit.
- Naive destination-conditioned UCB1 fails via sample fragmentation (finding 5).
- **Fig 3** convergence; **Fig 7** the decisive 150k / concentrated result
  (~2× ceiling, disjoint CIs both densities).
- Implemented in the real Router (`q_select_toward`, `q_advertised_value`,
  `q_record`) + `RoutingEstimate` protocol msg; unit-tested.  **Live multi-node
  validation is the one remaining experiment** (see `docs/Q_ROUTING.md`).

## 6. Realistic Traffic and Mobility — [done] traffic, [done] mobility (static-graph); [todo] real CRAWDAD trace
- Concentrated (Zipf) destinations flip the ranking (finding 6b, **Fig 2**):
  agnostic policies degrade as traffic concentrates, Q-routing improves — the
  curves cross because they depend on concentration with opposite sign.
- **Contact-trace mobility** (`core/src/trace.rs`, done): re-run the study over
  heavy-tailed synthetic traces and, if obtainable, a real CRAWDAD/Infocom trace.
  This is the headline "reviewer will ask" experiment and the current top TODO.

## 7. Robustness — [done] (`fig6_hyperparameter_sweep`, finding 6c)
- UCB1 never reaches its ceiling for any exploration constant `C`.
- Q-routing beats the ceiling for every `(α, ε)` tested.

## 8. Limitations & Honesty (mostly written across `RESULTS.md`)
- Static-graph vs trace-driven gap (being closed in §6).
- Live-network Q-routing not yet validated.
- Stationary oracle stops being the right reference under churn (future work).

## 9. Conclusion
Routing is a sequential decision problem, not a bandit problem; the reward a
relay sees is destination-agnostic and that, not the algorithm, is the ceiling.

--- ## Figure inventory (all in `results/figures/`, SVG)
| Fig | File | Section |
|---|---|---|
| 1 | fig1_ceiling | §4 |
| 2 | fig2_zipf_crossover | §6 |
| 3 | fig3_convergence | §5 |
| 4 | fig4_delivery_vs_overhead | §4/§5 |
| 5 | fig5_regret | §4 |
| 6 | fig6_hyperparameter_sweep | §7 |
| 7 | fig7_hightraffic_concentrated | §5 |

## Remaining experiments before submission
1.  **Real trace** (§6) — synthetic heavy-tailed DTN done (`trace_bench`, finding
  8). Remaining: feed a real CRAWDAD/Infocom trace via `ContactTrace::load_csv`,
  and port the Q-routing bootstrap into the DTN forwarder (PRoPHET stands in now).
2.  **Live multi-node Q-routing** (§5) — wire `RoutingEstimate` into `node.rs`,
  extend `tests/test_multi_node.rs` to assert the delivery-rate gain.
3.  Prose for abstract, §1, §2, §8, §9.
