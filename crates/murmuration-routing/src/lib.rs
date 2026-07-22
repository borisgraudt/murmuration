//! # murmuration-routing
//!
//! A small, dependency-light toolkit for **evaluating delay-tolerant (mesh)
//! routing** — extracted from the [murmuration](https://github.com/borisgraudt/murmuration)
//! mesh network so it can be reused and cited on its own.
//!
//! Today it provides the mobility substrate the routing study runs on:
//!
//! * [`trace::ContactTrace`] — a set of pairwise contact intervals over a set of
//!   nodes, either loaded from a real CRAWDAD/Infocom CSV
//!   ([`trace::ContactTrace::load_csv`]) or generated synthetically with
//!   **heavy-tailed (power-law) inter-contact times**
//!   ([`trace::ContactTrace::synthetic`]), matching the empirical property of
//!   human mobility (Chaintreau et al., 2007).
//! * [`trace::ContactTrace::earliest_arrival`] — the exact *foremost journey*
//!   (Bui-Xuan et al., 2003): the earliest a message created at a node can reach
//!   every other node. This is the oracle any store-carry-forward routing scheme
//!   is measured against.
//!
//! The learned routers themselves (a UCB1 bandit and a Q-routing value-bootstrap
//! forwarder) currently live in the parent crate and are being migrated here; see
//! the repository's `results/RESULTS.md` for the study these tools produced.
//!
//! ```
//! use murmuration_routing::trace::ContactTrace;
//!
//! // A reproducible synthetic trace: 20 nodes, ~1 day, power-law gaps.
//! let t = ContactTrace::synthetic(20, 86_400.0, 0.5, 30.0, 0.5, /*seed*/ 1);
//! let arrival = t.earliest_arrival(/*src*/ 0, /*t0*/ 0.0);
//! assert_eq!(arrival[0], 0.0); // the source is reachable at t0 by definition
//! ```

pub mod trace;
