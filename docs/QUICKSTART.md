# Elysium Quick Start Guide

**The Internet Without Internet**

## 🚀 Quick Demo (3 minutes)

### Step 0: Install ely

```bash
make install
# Or: cargo install --path core --bin ely
```

Now you can use `ely` directly instead of `cargo run --bin ely --release --`

---

### Step 1: Start two nodes

**Terminal 1:**
```bash
ely start 8080
```

Wait for:
```
INFO: Created new node with ID: Qm7xRJ...
INFO: Listening on: 0.0.0.0:8080
INFO: API server listening on: 0.0.0.0:17080
```

**Terminal 2:**
```bash
ely start 8081 127.0.0.1:8080
```

Wait for:
```
INFO: Connected to peer 127.0.0.1:8080
```

### Step 2: Send messages

**Terminal 3:**
```bash
# Send a broadcast message (CLI auto-finds the API port!)
ely broadcast "Hello Elysium!"

# Check your inbox
ely inbox 10

# Watch live messages (Ctrl+C to exit)
ely watch
```

**No `MESHLINK_API_PORT` needed!** CLI automatically discovers running nodes.

### Step 3: Publish content

```bash
# Publish some content
ely publish site/index.html "<h1>Hello World</h1>"

# Output: ✓ Content published at: ely://Qm7xRJ.../site/index.html

# Fetch it back
ely fetch ely://Qm7xRJ.../site/index.html
```

### Step 4: Register names

```bash
# Register a human-readable name
ely name register alice Qm7xRJ...

# Resolve it
ely name resolve alice
# Output: ✓ alice → Qm7xRJ...
```

### Step 5: Export/import bundles (USB transfer)

```bash
# Export messages to bundle
ely bundle export /tmp/messages.bundle
# Output: ✓ Bundle exported: 3 messages

# Check bundle info
ely bundle info /tmp/messages.bundle

# Import on another node (Terminal 2)
# First, get Terminal 2's API port with: ely status
# Then specify it explicitly:
MESHLINK_API_PORT=17081 ely bundle import /tmp/messages.bundle
# Output: ✓ Bundle imported: 3 delivered, 0 forwarded

# Or switch to Terminal 2 and run directly:
ely bundle import /tmp/messages.bundle
```

---

## 📖 All CLI Commands

### Node Management

**Start a node:**
```bash
ely start <p2p_port> [peer1] [peer2] ...
```

**Check status:**
```bash
ely status
```

**List peers:**
```bash
ely peers
```

### Messaging

**Send to specific peer:**
```bash
ely send <peer_id> <message>
```

**Broadcast to all:**
```bash
ely broadcast <message>
```

**Check inbox:**
```bash
ely inbox [count]      # Default: 20 messages
```

**Live watch (stream messages):**
```bash
ely watch              # Press Ctrl+C to exit
```

**Interactive chat:**
```bash
ely chat <peer_id|broadcast>
```

**Ping a peer:**
```bash
ely ping <peer_id> [timeout_ms]
```

### Content Addressing

**Publish content:**
```bash
ely publish <path> <content>
ely publish site/index.html "<h1>Hello</h1>"
ely publish site/style.css @style.css    # Read from file
```

**Fetch content:**
```bash
ely fetch ely://<node_id>/<path>
```

### Naming System

**Register name:**
```bash
ely name register <name> <node_id>
```

**Resolve name:**
```bash
ely name resolve <name>
```

### Bundle Protocol (Store-and-Forward)

**Export messages to bundle:**
```bash
ely bundle export <output_file>
```

**Import bundle:**
```bash
ely bundle import <input_file>
```

**Show bundle info:**
```bash
ely bundle info <bundle_file>
```

---

## 🔧 Configuration

### API Port Auto-Discovery

**No configuration needed!** CLI automatically finds the running node.

**How it works:**
1. Checks `MESHLINK_API_PORT` env var (if set)
2. Reads `~/.elysium_api_port` (last node saves its port here)
3. Tries default port `17080` (most common: 8080 → 17080)
4. Scans ports 17080-17089

**API Port Formula:** `9000 + P2P_PORT`
- P2P port 8080 → API port 17080
- P2P port 8081 → API port 17081

**Override if needed:**
```bash
MESHLINK_API_PORT=17081 ely status
```

### Environment Variables

```bash
MESHLINK_API_PORT=17080              # API port
MESHLINK_DISCOVERY_PORT=9998         # Discovery port (default)
MESHLINK_NO_DISCOVERY=1              # Disable mDNS discovery
MESHLINK_MAX_CONNECTIONS=10          # Max peer connections
MESHLINK_CONNECT_COOLDOWN_MS=5000    # Connection retry cooldown
```

### Data Directory

Node data is stored in `.ely/node-<port>/`:
- `identity.json` - Node ID and keys
- `content.db` - Published content
- `messages.db` - Message history
- `names.db` - Name registry
- `peers.cache` - Discovered peers

---

## 💡 Use Cases

### 1. Offline Messenger
Run nodes on phones/laptops with WiFi Direct, exchange messages without internet.

### 2. Censorship Bypass
Use bundles to transfer messages via USB/SD card when network is blocked.

### 3. Delay-Tolerant Networking
Messages are stored and forwarded when peers come online.

### 4. Content Publishing
Publish websites/files that propagate through the mesh.

---

## 🐛 Troubleshooting

### Port already in use

```bash
# Check what's using the port
lsof -i :8080

# Kill old nodes
killall ely core
```

### Nodes not connecting

1. Check logs: `RUST_LOG=info ely start 8080`
2. Try connecting explicitly: `ely start 8081 127.0.0.1:8080`
3. Check firewall settings

### API not found

```bash
# CLI tries ports 17070-17100 automatically
# Or set explicitly:
MESHLINK_API_PORT=17080 ely status
```

### Messages not showing in inbox

1. Check node is running: `ely status`
2. Wait for discovery (~5 seconds)
3. Check API port matches node port

---

## 📚 Next Steps

- Read [PROTOCOL.md](PROTOCOL.md) for wire protocol details
- Read [ARCHITECTURE.md](ARCHITECTURE.md) for system design
- See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common issues

**Ready to build on Elysium?** The platform is stable. Build messengers, websites, search engines on top of it.
