# Q-routing in the node

Status, as of this document: **library logic implemented and unit-tested;
live multi-node wiring is designed but NOT yet validated on a running mesh.**
This file is the honest boundary between the two.

## Why

The routing benchmark (`core/src/bin/benchmark.rs`, results in `results/`)
established that the shipped UCB1 router is bounded by a *destination-agnostic
ceiling* it has already saturated, and that **Q-routing** — value bootstrapping
from a neighbour's own estimate — is the mechanism that crosses that ceiling
under concentrated (messenger-like) traffic. This brings that mechanism from the
benchmark into the real `Router`.

## What is implemented and verified

In `crates/murmuration-routing/src/router.rs`, on the real `Router` type the node uses:

- `QRoutingState` — `q[(dest, neighbour)] → delivery-probability estimate`, with
  optimistic initialisation (`Q_INIT = 1.0`) and the Boyan–Littman update at
  learning rate `Q_ALPHA = 0.15`.
- `q_select_toward(msg, peers, max_peers, dest)` — next-hop choice by Q-value,
  loop-safe (excludes sender and path), connected peers only.
- `q_advertised_value(dest, neighbours)` — the value this node reports upstream:
  `max` over its neighbours' estimates.
- `q_record(dest, peer, delivered, downstream_value)` — the bootstrap update;
  target is the neighbour's advertised value on success, 0 on failure.

Wire support:

- `Message::RoutingEstimate { message_id, from, dest, value_permille }` in
  `core/src/p2p/protocol.rs`. The estimate is quantised to per-mille so the
  `Message` enum keeps `Eq`; convert with `protocol::permille` /
  `protocol::from_permille`. Old peers do not recognise the variant and ignore
  it, so a mixed network degrades to plain forwarding rather than breaking.

Unit tests (`cargo test --lib ai::router`, all passing):

- `test_q_optimistic_init` — unseen pairs advertise `Q_INIT`; no neighbours → 0.
- `test_q_record_bootstraps_downstream_value` — success moves Q toward the
  advertised value; repeated failure drives it to ~0.
- `test_q_select_prefers_higher_value` — the better-scoring neighbour is chosen.
- `test_q_select_excludes_sender_and_path` — loop-freedom holds.

## What is NOT yet done (the live-network step)

The following require a running multi-node network to validate, and are
deliberately not claimed as working until that happens:

1. **Emit `RoutingEstimate`.** When a node forwards a directed `MeshMessage`
   toward `dest`, the chosen next hop should reply with
   `RoutingEstimate { dest, value: q_advertised_value(dest, neighbours) }`.
   Insertion point: the mesh-message handler in `core/src/node.rs`
   (`handle_mesh_message`, ~line 2139) and the forwarding path (~line 2267).
2. **Consume `RoutingEstimate`.** On receipt, call
   `q_record(dest, peer, delivered=true, from_permille(value))`. On a delivery
   timeout with no estimate, call `q_record(dest, peer, false, 0.0)`.
3. **Switch selection.** Replace / gate `get_best_forward_peers` with
   `q_select_toward` behind a config flag, so UCB1 remains the default until
   Q-routing is proven on real sockets.
4. **Validate.** Run the existing `tests/test_multi_node.rs` harness extended to
   assert delivery-rate improvement under concentrated traffic, mirroring the
   benchmark's finding on a live mesh.

Until step 4 passes, the paper's claim must read "Q-routing implemented in the
router and validated in simulation; live-network evaluation is in progress" —
never "murmuration uses Q-routing."

## Design of the exchange

```
        forward(msg toward D)
   u ───────────────────────────▶ v
                                   │  v computes best_over(D, its neighbours)
        RoutingEstimate(D, value)  │
   u ◀───────────────────────────┘
   u: q_record(D, v, delivered, value)
```

One estimate per forwarded message; no separate control traffic. The estimate
rides back on the same path the ack already uses, so the overhead is one small
message per hop — the same order as the existing `MessageAck`.
