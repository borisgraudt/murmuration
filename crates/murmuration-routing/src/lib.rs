//! # murmuration-routing
//!
//! A small, dependency-light toolkit for **evaluating delay-tolerant (mesh)
//! routing** — extracted from the [murmuration](https://github.com/borisgraudt/murmuration)
//! mesh network so it can be reused and cited on its own.
//!
//! It contains both the learned routers and the substrate to evaluate them:
//!
//! * [`Router`] — adaptive next-hop selection: a **UCB1 bandit**
//!   ([`Router::get_best_forward_peers`]) plus **Q-routing** value bootstrapping
//!   ([`Router::q_select_toward`], [`Router::q_advertised_value`],
//!   [`Router::q_record`]). Learned state persists through an optional
//!   [`RouterStore`], so the crate itself does no I/O.
//! * [`peer`] — the observable peer types a routing decision reads
//!   ([`peer::PeerInfo`], [`peer::PeerMetrics`]).
//! * [`trace::ContactTrace`] — contact traces for delay-tolerant evaluation:
//!   load a real CRAWDAD/Infocom CSV ([`trace::ContactTrace::load_csv`]) or
//!   generate one with **heavy-tailed (power-law) inter-contact times**
//!   ([`trace::ContactTrace::synthetic`]), matching human mobility (Chaintreau
//!   et al., 2007).
//! * [`trace::ContactTrace::earliest_arrival`] — the exact *foremost journey*
//!   (Bui-Xuan et al., 2003): the oracle any store-carry-forward scheme is
//!   measured against.
//!
//! See the repository's `results/RESULTS.md` for the study these tools produced:
//! bandit routing has a destination-agnostic ceiling, and Q-routing breaks it.
//!
//! ```
//! use murmuration_routing::trace::ContactTrace;
//!
//! // A reproducible synthetic trace: 20 nodes, ~1 day, power-law gaps.
//! let t = ContactTrace::synthetic(20, 86_400.0, 0.5, 30.0, 0.5, /*seed*/ 1);
//! let arrival = t.earliest_arrival(/*src*/ 0, /*t0*/ 0.0);
//! assert_eq!(arrival[0], 0.0); // the source is reachable at t0 by definition
//! ```

pub mod error;
pub mod peer;
pub mod router;
pub mod routing_logger;
pub mod stats;
pub mod trace;

pub use router::{MeshMessage, MurmurationAddress, Router, RouterStore};
