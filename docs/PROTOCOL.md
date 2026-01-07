# Elysium Protocol v0.1

**Status:** Draft  
**Date:** 2025-01-07

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

**Mesh forwarding:**
- AI-based peer selection (latency, uptime)
- TTL countdown (max 8 hops)
- Path tracking (loop prevention)
- Best-effort delivery

**Store-and-forward:**
- Queue messages when offline
- Export as bundle (USB/SD)
- Import on destination network
- Expiry: 7 days

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

