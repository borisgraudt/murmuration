# Murmuration: A Delay-Tolerant Mesh Network with Learned Routing

[![crates.io](https://img.shields.io/crates/v/murmuration-routing.svg)](https://crates.io/crates/murmuration-routing)
[![docs.rs](https://docs.rs/murmuration-routing/badge.svg)](https://docs.rs/murmuration-routing)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

Murmuration is a decentralized, censorship-resistant overlay network designed to
operate without fixed infrastructure. Nodes communicate directly over TCP,
discover peers via UDP multicast, and route messages with an adaptive policy that
learns from delivery outcomes at runtime.

**Target scenarios:** protest coordination, disaster relief, and bypassing
censorship — anywhere IP connectivity is unreliable or actively blocked.

> **Routing research.** The shipped router uses a UCB1 bandit, but a study in
> [`results/RESULTS.md`](results/RESULTS.md) shows *why bandit routing has a
> structural ceiling*: the reward a relay observes is destination-agnostic. We
> derive the exact bound, show UCB1 saturates it, and show that value
> bootstrapping (Q-routing) breaks it — roughly doubling delivery under realistic
> traffic. The reusable evaluation toolkit is published as the
> [`murmuration-routing`](https://crates.io/crates/murmuration-routing) crate.

---

## Key properties

| Property | Mechanism |
|---|---|
| Infrastructure-free | Direct TCP connections; UDP multicast discovery |
| End-to-end encryption | RSA-2048 key exchange + AES-256-GCM session cipher |
| Adaptive routing | UCB1 bandit (Auer et al., 2002) over peer reward history |
| Content addressing | `mur://<sha256(pubkey)>/<path>` — self-verifying URLs |
| Delay tolerance | Store-and-forward bundle protocol (USB/file transfer) |
| Human-readable names | Local name registry: `alice` → node\_id |

---

## Architecture

```
┌──────────────────────────────────────────────┐
│  Applications  (messaging, content, naming)  │
├──────────────────────────────────────────────┤
│  Routing       (UCB1 adaptive + flooding)    │
├──────────────────────────────────────────────┤
│  Identity      (RSA-2048, node_id = SHA256)  │
├──────────────────────────────────────────────┤
│  Transport     (TCP, UDP multicast)          │
└──────────────────────────────────────────────┘
```

**Peer selection** (after warm-up, n ≥ 5 samples per peer):

```
score(i) = μ_i + sqrt( 2 · ln(N) / n_i )
```

where μ\_i is the incremental-average delivery reward for peer *i*,
N is the total number of routing decisions, and n\_i is the number of
times peer *i* has been selected. During warm-up the heuristic score
(latency + uptime + reliability) is used with an exploration bonus
that forces all peers to be tried at least once.

Rewards: `r = clamp(1 − 2·latency_s,  0.5, 1.0)` on success; `r = 0` on failure.

---

## Repository layout

```
core/            Rust implementation (node, CLI, benchmarks, tests)
  src/
    ai/          UCB1 router, stats collector, routing logger
    p2p/         Protocol frames, encryption, peer manager, discovery
    node.rs      Core event loop (~1400 lines)
    api.rs       JSON-RPC TCP API
    web_gateway.rs  HTTP gateway for mur:// URLs
    bundle.rs    Store-and-forward protocol
  benches/       Criterion benchmarks (routing.rs)
  tests/         Integration and unit tests
python_cli/      Python TUI (Rich) — alternative interface
web/frontend/    Static topology dashboard
docs/            Architecture, protocol spec, quickstart
scripts/         Demo helpers
```

---

## Installation

**From source:**
```bash
git clone https://github.com/borisgraudt/murmuration.git
cd murmuration
make install          # builds and installs the `mur` binary
```

**Via cargo:**
```bash
cargo install --git https://github.com/borisgraudt/murmuration.git \
  --package murmuration --bin mur
```

**Docker:**
```bash
docker pull ghcr.io/borisgraudt/murmuration:main
docker run --rm -it -p 8080:8080 -p 9998:9998/udp \
  ghcr.io/borisgraudt/murmuration:main start 8080
```

---

## Quick demo (3 nodes, one machine)

```bash
# Terminal 1 — bootstrap node
mur start 8080 --gateway 8000

# Terminal 2
mur start 8081 127.0.0.1:8080 --gateway 8001

# Terminal 3
mur start 8082 127.0.0.1:8081 --gateway 8002

# Publish content (from node 1)
MURMURATION_API_PORT=17080 mur publish site/hello.html "<h1>Hello Mesh</h1>"

# Fetch across the mesh (from node 3)
MURMURATION_API_PORT=17082 mur fetch mur://<node1_id>/site/hello.html

# View in browser
open http://localhost:8000/mur/<node1_id>/site/hello.html

# Broadcast message
MURMURATION_API_PORT=17080 mur broadcast "hello from node 1"

# Offline bundle transfer (USB simulation)
MURMURATION_API_PORT=17080 mur bundle export /tmp/msgs.bundle
MURMURATION_API_PORT=17082 mur bundle import /tmp/msgs.bundle
```

API port formula: `MURMURATION_API_PORT = 9000 + P2P_PORT` (e.g. 8080 → 17080).

---

## Benchmarks

```bash
cargo bench --bench routing
# HTML report: target/criterion/routing/report/index.html
```

Benchmarks cover: `calculate_peer_score`, cold-start peer selection (N=4/8/16/24),
UCB1 peer selection after warmup, and message deduplication.

---

## Tests

```bash
cargo test --release
```

Key test files:
- `tests/test_routing_adaptive.rs` — UCB1 exploration, exploitation, cold-start
- `tests/test_multi_node.rs` — 3-node and 5-node mesh formation
- `tests/test_stability.rs` — timeout, error handling, reconnect

---

## Routing study

The routing benchmark drives the **real** shipped `Router` (not a reimplementation)
against an exact oracle, and adds Q-routing as the fix for UCB1's ceiling.

```bash
cargo run --release --bin benchmark     # static-graph study → results/benchmark_*.csv
cargo run --release --bin trace_bench   # delay-tolerant, contact-trace mobility
python3 results/make_figures.py         # regenerate the 7 figures
```

Full write-up, tables, and figures: [`results/RESULTS.md`](results/RESULTS.md).
Paper draft: [`paper/`](paper/). The contact-trace / oracle toolkit is reusable as
the [`murmuration-routing`](https://crates.io/crates/murmuration-routing) crate.

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — layer model, component diagram
- [Protocol Spec](docs/PROTOCOL.md) — wire format, message types, routing
- [Quickstart](docs/QUICKSTART.md) — step-by-step setup
- [Troubleshooting](docs/TROUBLESHOOTING.md) — common issues
- [Q-routing](docs/Q_ROUTING.md) — the destination-aware fix and its status

---

## License

MIT
