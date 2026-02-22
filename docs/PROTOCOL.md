# Elysium Protocol v0.2

**Status:** Draft
**Date:** 2026-02-23

---

## Overview

Elysium is a content-addressed, identity-based, delay-tolerant network protocol.

**Design principles:**
- Identity = cryptographic keys
- Addressing = content hashes
- Routing = opportunistic forwarding
- Delivery = store-and-forward

---

## 1. Identity

Every node has a persistent identity:

```
identity = ed25519_keypair
node_id = base58(sha256(public_key))
```

**Properties:**
- Self-sovereign (no registration)
- Collision-resistant (256-bit hash)
- Verifiable (signatures)

**Example:**
```
node_id: Qm7xRJP3nN8K9vZ2M4...
public_key: 0x48a3f2c1...
```

---

## 2. Addressing

Everything is content-addressed:

```
ely://<node_id>/<path>
```

**Examples:**
```
ely://Qm7xRJ.../profile      - user profile
ely://Qm7xRJ.../messages     - message inbox
ely://Qm7xRJ.../site/index   - hosted content
```

**Verification:**
```
content_hash = sha256(data)
verify(content, claimed_hash)
```

---

## 3. Wire Protocol

### Handshake

```
Client → Server: HELLO
  {
    node_id: string
    protocol_version: u8
    public_key: string
  }

Server → Client: ACK
  {
    node_id: string
    protocol_version: u8
    public_key: string
    encrypted_session_key: bytes
    nonce: bytes
  }
```

### Frame Format

```
[4 bytes length][payload]

payload = encrypted(JSON message)
encryption = AES-256-GCM
```

### Message Types

```
Data          - encrypted payload
MeshMessage   - routed message (with TTL, path)
ContentRequest - fetch content
ContentResponse - return content
Bundle        - offline message batch
NameAnnounce  - name registration
```

---

## 4. Routing

### 4.1 UCB1 Adaptive Peer Selection

Elysium selects forwarding peers using the **UCB1 multi-armed bandit** algorithm
(Auer et al., 2002). Each connected peer is treated as an arm; the reward signal
reflects delivery speed and success.

**Score function (post warm-up, n_i ≥ 5):**

```
score(i) = μ_i + sqrt( C · ln(N) / n_i )

  μ_i  — incremental average reward for peer i
  N    — total routing selections across all peers
  n_i  — times peer i has been selected
  C    — exploration constant (default: 2.0)
```

**Reward function:**

```
r = clamp(1 − 2 · latency_secs,  0.5, 1.0)   on delivery success
r = 0.0                                        on failure / timeout
```

**Cold-start (n_i < 5):** heuristic score (latency + uptime + reliability)
plus an exploration bonus of +1.0 for unvisited peers and +0.5 for partially
sampled peers. This guarantees every peer is tried before UCB1 takes over.

**Reward recording:** `record_route_success(peer, latency)` and
`record_route_failure(peer)` update both the UCB1 bandit state and the
heuristic history atomically.

### 4.2 Message Forwarding

```
MeshMessage {
  from:       string          # originating node_id
  to:         Option<string>  # destination node_id; None = broadcast
  data:       bytes
  message_id: UUID v4
  ttl:        u8              # decremented at each hop (default: 10)
  path:       [string]        # visited node_ids — loop detection
}
```

Processing pipeline:
1. Drop if TTL == 0.
2. Drop if `message_id` was seen within the past 60 s.
3. Drop if `our_node_id` appears in `path`.
4. Deliver locally if `to == our_node_id` or broadcast.
5. Select forward peers via UCB1 (or flooding for broadcasts).
6. Decrement TTL, append `our_node_id` to `path`, forward.

### 4.3 Store-and-Forward (Bundle Protocol)

```
Bundle {
  version:    u8
  created_at: timestamp
  expires_at: timestamp       # default: created_at + 30 days
  messages:   [MeshMessage]
}
```

Bundles are serialised to a single file for physical transfer (USB, SD card).
On import, each message is replayed through the normal processing pipeline.

**Use case:** two mesh islands with no real-time link exchange bundles
periodically; latency is measured in hours to days.

---

## 5. Naming

Optional human-readable names:

```
NameRecord {
  name: "alice"
  node_id: "Qm7xRJ..."
  timestamp: i64
  expires_at: i64
  signature: bytes
}
```

**Resolution:**
```
ely://alice → resolve → Qm7xRJ... → fetch content
```

**Conflict resolution:** Most recent timestamp wins.

---

## 6. Security

- **E2E encryption:** RSA-2048 handshake + AES-256-GCM session
- **Forward secrecy:** New session key per connection
- **Authentication:** All data signed by node_id
- **No trusted parties:** Peer-to-peer only

---

## 7. Transport

**Supported:**
- TCP (LAN/Internet)
- Bundle (USB/SD card)

**Planned:**
- Bluetooth LE
- LoRa
- QR codes

---

## 8. Versioning

Protocol version in handshake.  
Breaking changes → new major version.  
Backward compatibility within v0.x.

Current: **v0.1**

---

## Implementation

Reference: [github.com/borisgraudt/elysium](https://github.com/borisgraudt/elysium)

Language: Rust  
License: MIT

