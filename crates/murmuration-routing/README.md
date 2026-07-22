# murmuration-routing

[![crates.io](https://img.shields.io/crates/v/murmuration-routing.svg)](https://crates.io/crates/murmuration-routing)
[![docs.rs](https://docs.rs/murmuration-routing/badge.svg)](https://docs.rs/murmuration-routing)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

A small, dependency-light toolkit for **evaluating delay-tolerant (mesh)
routing**, extracted from the [murmuration](https://github.com/borisgraudt/murmuration)
mesh network so it can be reused and cited on its own.

It provides the mobility substrate a routing study runs on:

- **`ContactTrace`** — a set of pairwise contact intervals over a set of nodes,
  loaded from a real CRAWDAD/Infocom CSV (`load_csv`) or generated synthetically
  with **heavy-tailed (power-law) inter-contact times** (`synthetic`), matching
  the empirical property of human mobility (Chaintreau et al., 2007).
- **`earliest_arrival`** — the exact *foremost journey* (Bui-Xuan et al., 2003):
  the earliest a message created at a node can reach every other node. This is
  the oracle any store-carry-forward routing scheme should be measured against.

It also ships the learned routers themselves — a UCB1 bandit and a Q-routing
value-bootstrap forwarder — behind a storage-agnostic `RouterStore`.

## Example

```rust
use murmuration_routing::trace::ContactTrace;

// A reproducible synthetic trace: 20 nodes, ~1 day, power-law gaps.
let t = ContactTrace::synthetic(20, 86_400.0, 0.5, 30.0, 0.5, /*seed*/ 1);

// Oracle: earliest arrival from node 0 for a message created at t=0.
let arrival = t.earliest_arrival(0, 0.0);
assert_eq!(arrival[0], 0.0);

// Or load a real trace: rows of `start,end,a,b` in seconds.
let real = ContactTrace::load_csv("0,600,0,1\n120,180,1,2\n").unwrap();
assert_eq!(real.n, 3);
```

## License

MIT
