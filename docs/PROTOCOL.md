# Murmuration Protocol v0.3

**Status:** Draft
**Date:** 2026-03-06
**Wire version:** 2

---

## Overview

Murmuration is a content-addressed, identity-based, delay-tolerant network protocol.

**Design principles:**
- Identity = cryptographic keys
- Addressing = content hashes
- Routing = opportunistic forwarding with adaptive learning
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

---

## 2. Addressing

```
mur://<node_id>/<path>
```

**Examples:**
```
mur://Qm7xRJ.../profile      - user profile
mur://Qm7xRJ.../messages     - message inbox
mur://Qm7xRJ.../site/index   - hosted content
```

**Verification:**
```
content_hash = sha256(data)
verify(content, claimed_hash)
```

---

## 3. Wire Protocol

### 3.1 Handshake (v2 — X25519 Forward Secrecy)

Protocol version 2 adds ephemeral Diffie-Hellman key agreement. Both sides
generate a fresh X25519 keypair per connection and advertise it in the
handshake. The resulting shared secret is fed through HKDF-SHA256 to derive
the per-session AES-256-GCM key. If either peer omits `ephemeral_pubkey`
(v1 legacy peer), the responder falls back to RSA-2048/OAEP key encapsulation.

```
Initiator → Responder: Handshake
  {
    node_id:          string
    protocol_version: u8              // 2
    public_key:       string          // RSA-2048 public key, base64
    ephemeral_pubkey: string          // X25519 ephemeral public key, base64
  }

Responder → Initiator: HandshakeAck
  {
    node_id:               string
    protocol_version:      u8
    public_key:            string
    ephemeral_pubkey:      string     // responder's X25519 ephemeral key
    encrypted_session_key: bytes      // fallback only (v1 path)
    nonce:                 bytes      // fallback only
  }
```

**Session key derivation (v2 path):**
```
dh_shared   = X25519(initiator_ephemeral_secret, responder_ephemeral_public)
session_key = HKDF-SHA256(ikm=dh_shared, salt=[], info="murmuration-session-v2")[..32]
```

Both sides independently compute the same `session_key`; no ciphertext is
exchanged for the key itself.

### 3.2 Frame Format

```
[4 bytes length BE][payload]

payload = encrypt_aes_gcm(padded_message, session_key, fresh_nonce)
```

### 3.3 Message Padding

All payloads are padded to a multiple of 256 bytes before encryption:

```
padded = [real_len: u16 BE][plaintext][random_bytes...]
         └────────────────────────────────────────────┘
                  length is a multiple of 256
```

This defeats traffic-analysis attacks that infer message length from
ciphertext size. The receiver reads `real_len` from the first two bytes to
recover the original message. Peers that lack this prefix are handled
gracefully (the raw decrypted bytes are used as-is).

### 3.4 Message Types

```
Data            - hop-by-hop encrypted frame
MeshMessage     - routed message (with TTL, path)
ContentRequest  - fetch content by hash
ContentResponse - return content
Bundle          - offline message batch
NameAnnounce    - name registration
```

---

## 4. Routing

### 4.1 UCB1 Adaptive Peer Selection

Murmuration selects forwarding peers using the **UCB1 multi-armed bandit** algorithm
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

**Persistence:** UCB1 bandit state is serialised to a sled embedded database
at `.mur/node-<port>/ucb1/` and loaded at startup, so learned routing
preferences survive process restarts.

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

---

## 5. End-to-End Encryption (DM Payloads)

Hop-by-hop encryption protects against eavesdropping by intermediate nodes,
but the relay nodes still process plaintext payloads. E2E encryption provides
an additional layer: only the intended recipient can decrypt the payload.

**Wire format** (stored in `MeshMessage.data`):

```
[0xE2][JSON(E2EPayload)]

E2EPayload {
  encrypted_key:  bytes   // RSA-OAEP encrypted AES key
  nonce:          bytes   // AES-GCM nonce (12 bytes)
  ciphertext:     bytes   // AES-256-GCM ciphertext
}
```

**Encryption:**
```
aes_key    = random(32 bytes)
ciphertext = AES-256-GCM(aes_key, nonce, plaintext)
enc_key    = RSA-OAEP-SHA256(recipient_pubkey, aes_key)
```

**Decryption:** recipient decrypts `enc_key` with their RSA private key, then
decrypts the ciphertext. Nodes that are not the intended recipient see
`0xE2`-prefixed opaque bytes.

---

## 6. Group Messaging

Group messages are encrypted with a shared symmetric key derived
deterministically from the group identity, requiring no central key server.

**Key derivation:**

```
key_material = SHA-256("murmuration-group-v1:" || group_id || "|" || sorted_member_ids)
group_key    = Key<AES-256-GCM>(key_material)
```

The member list is sorted before hashing, so any permutation of member IDs
produces the same key.

**Wire format** (stored in `MeshMessage.data`):

```
[0x6B][JSON(GroupPayload)]

GroupPayload {
  group_id:   string
  nonce:      bytes     // AES-GCM nonce (12 bytes)
  ciphertext: bytes     // AES-256-GCM ciphertext
}
```

**Membership changes** require a new `group_id` and therefore a new key.
No forward secrecy within a group epoch (future work: double-ratchet).

---

## 7. Naming

Optional human-readable names:

```
NameRecord {
  name:       "alice"
  node_id:    "Qm7xRJ..."
  timestamp:  i64
  expires_at: i64
  signature:  bytes
}
```

**Resolution:**
```
mur://alice → resolve → Qm7xRJ... → fetch content
```

**Conflict resolution:** Most recent timestamp wins.

---

## 8. Security Summary

| Property | Mechanism |
|----------|-----------|
| Hop-by-hop encryption | AES-256-GCM with per-session key |
| Forward secrecy | X25519 ephemeral DH + HKDF-SHA256 (v2) |
| E2E confidentiality | RSA-OAEP + AES-256-GCM (DM payloads) |
| Group confidentiality | Deterministic AES-256-GCM group key |
| Traffic analysis resistance | Fixed 256-byte payload blocks |
| Authentication | Ed25519 node identity; RSA-2048 for key transport |
| No trusted parties | Peer-to-peer only |

---

## 9. Transport

**Supported:**
- TCP (LAN/Internet)
- Bundle (USB/SD card)

**Planned:**
- Bluetooth LE
- LoRa
- QR codes

---

## 10. Versioning

Protocol version is advertised in the handshake `protocol_version` field.

| Version | Changes |
|---------|---------|
| 1 | RSA-2048/OAEP key encapsulation, AES-256-GCM session |
| 2 | X25519 ephemeral DH + HKDF-SHA256; message padding; E2E; group messaging |

Nodes that receive a handshake without `ephemeral_pubkey` fall back to v1
key encapsulation, ensuring backward compatibility.

---

## Implementation

Reference: [github.com/borisgraudt/murmuration](https://github.com/borisgraudt/murmuration)

Language: Rust
License: MIT
