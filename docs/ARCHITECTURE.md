# Elysium Architecture

**Frugal, minimal, robust.**

---

## Layer Model

```
┌─────────────────────────────┐
│  Applications               │  Messenger, Sites, Search
├─────────────────────────────┤
│  Naming                     │  ely://alice → node_id
├─────────────────────────────┤
│  Content                    │  hash → data, propagation
├─────────────────────────────┤
│  Routing                    │  mesh forwarding, bundles
├─────────────────────────────┤
│  Identity & Crypto          │  keys, signatures, encryption
├─────────────────────────────┤
│  Transport                  │  TCP, Bluetooth, LoRa, USB
└─────────────────────────────┘
```

---

## Core Components

### Node (`core/src/node.rs`)
- Main P2P node logic
- Connection management
- Message routing
- API server

### Identity (`core/src/identity.rs`)
- Key pair generation (RSA-2048)
- Persistent identity storage
- Signature/verification

### Content Store (`core/src/content_store.rs`)
- Key-value storage (sled)
- Content addressing
- Local cache

### Router (`core/src/ai/router.rs`)
- Peer scoring (latency, uptime)
- Path selection
- Loop prevention

### Transport (`core/src/p2p/`)
- Discovery (UDP broadcast)
- Connections (TCP)
- Encryption (AES-GCM)

---

## Data Flow

### Publishing Content

```
1. User publishes file
2. Store in local content_store
3. Generate hash: sha256(content)
4. Address: ely://self/<path>
5. Announce to connected peers
```

### Fetching Content

```
1. Request: ely://alice/profile
2. Resolve alice → node_id
3. Check local cache
4. If not cached: request from network
5. Verify hash
6. Return content
```

### Sending Message

```
1. Compose message
2. Encrypt for recipient
3. Route through mesh (AI-selected peers)
4. Deliver or queue if offline
5. ACK on delivery
```

### Store-and-Forward

```
1. Export pending messages → bundle
2. Transfer physically (USB)
3. Import on destination network
4. Deliver to recipients
```

---

## Security Model

**Threat model:**
- No trusted parties
- Adversarial network
- Censorship resistance
- Offline capability

**Guarantees:**
- E2E encryption (RSA + AES-256-GCM)
- Authenticated data (signatures)
- No metadata leakage (planned: onion routing)
- No central points of failure

---

## Design Decisions

### Why Rust?
- Memory safety
- Performance (zero-cost abstractions)
- Concurrency (tokio async)
- Strong type system

### Why content addressing?
- Self-verifying (hash = address)
- Deduplication (same content = same hash)
- Offline-capable (pre-computed addresses)

### Why store-and-forward?
- Works without real-time connectivity
- Survives network partitions
- Supports offline mesh networks

### Why no blockchain?
- Unnecessary complexity
- Energy waste
- Centralization risk (mining pools)
- Simple key-value store sufficient

---

## Performance Characteristics

**Latency:**
- Local: <1ms
- LAN (1 hop): <10ms
- Mesh (3 hops): 30-100ms
- Bundle (offline): hours to days

**Throughput:**
- TCP: ~100 Mbps (gigabit LAN)
- Bluetooth: ~1 Mbps
- LoRa: 0.3-50 Kbps
- USB: limited by write speed

**Storage:**
- Message history: ~1 MB per 1000 messages
- Content cache: configurable (default: 1 GB)
- Identity keys: <1 KB

---

## Future Work

- Anonymous routing (onion-style)
- Multi-transport (Bluetooth, LoRa)
- Content replication (gossip protocol)
- Reputation system (prevent spam)
- Mobile app (iOS/Android)

---

**Built for freedom of communication.**



