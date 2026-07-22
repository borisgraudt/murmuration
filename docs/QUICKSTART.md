# Murmuration Quick Start Guide

**The Internet Without Internet**

## Quick Demo (3 minutes)

### Step 0: Install mur

```bash
make install
# Or: cargo install --path core --bin mur
```

Now you can use `mur` directly instead of `cargo run --bin mur --release --`

--- ### Step 1: Start two nodes

**Option A: Foreground (logs visible, Ctrl+C to stop)**
```bash
# Terminal 1
mur start 8080

# Terminal 2 (new terminal)
mur start 8081 127.0.0.1:8080
```

**Option B: Background (daemon mode - can use same terminal)**
```bash
# Terminal 1
mur start 8080 -d
# Output:  Node started in background (PID: 12345)
# → Logs: .mur/node-8080/node-8080.log
# → Stop with: kill 12345

# Now you can use the same terminal for commands:
mur status
mur broadcast "hello"

# Terminal 2
mur start 8081 127.0.0.1:8080 -d
```

### Step 2: Send messages

**Terminal 3:**
```bash
# Send a broadcast message (CLI auto-finds the API port!)
mur broadcast "Hello Murmuration!"

# Check your inbox
mur inbox 10

# Watch live messages (Ctrl+C to exit)
mur watch
```

**No `MURMURATION_API_PORT` needed!** CLI automatically discovers running nodes.

### Step 3: Publish content

```bash
# Publish some content
mur publish site/index.html "<h1>Hello World</h1>"

# Output:  Content published at: mur://Qm7xRJ.../site/index.html

# Fetch it back
mur fetch mur://Qm7xRJ.../site/index.html
```

### Step 4: Register names

```bash
# Register a human-readable name
mur name register alice Qm7xRJ...

# Resolve it
mur name resolve alice
# Output:  alice → Qm7xRJ...
```

### Step 5: Export/import bundles (USB transfer)

```bash
# Export messages to bundle
mur bundle export /tmp/messages.bundle
# Output:  Bundle exported: 3 messages

# Check bundle info
mur bundle info /tmp/messages.bundle

# Import on another node (Terminal 2)
# First, get Terminal 2's API port with: mur status
# Then specify it explicitly:
MURMURATION_API_PORT=17081 mur bundle import /tmp/messages.bundle
# Output:  Bundle imported: 3 delivered, 0 forwarded

# Or switch to Terminal 2 and run directly:
mur bundle import /tmp/messages.bundle
```

--- ## All CLI Commands

### Node Management

**Start a node:**
```bash
mur start <p2p_port> [peer1] [peer2] ...
```

**Check status:**
```bash
mur status
```

**List peers:**
```bash
mur peers
```

### Messaging

**Send to specific peer:**
```bash
mur send <peer_id> <message>
```

**Broadcast to all:**
```bash
mur broadcast <message>
```

**Check inbox:**
```bash
mur inbox [count]  # Default: 20 messages
```

**Live watch (stream messages):**
```bash
mur watch  # Press Ctrl+C to exit
```

**Interactive chat:**
```bash
mur chat <peer_id|broadcast>
```

**Ping a peer:**
```bash
mur ping <peer_id> [timeout_ms]
```

### Content Addressing

**Publish content:**
```bash
mur publish <path> <content>
mur publish site/index.html "<h1>Hello</h1>"
mur publish site/style.css @style.css  # Read from file
```

**Fetch content:**
```bash
mur fetch mur://<node_id>/<path>
```

### Naming System

**Register name:**
```bash
mur name register <name> <node_id>
```

**Resolve name:**
```bash
mur name resolve <name>
```

### Bundle Protocol (Store-and-Forward)

**Export messages to bundle:**
```bash
mur bundle export <output_file>
```

**Import bundle:**
```bash
mur bundle import <input_file>
```

**Show bundle info:**
```bash
mur bundle info <bundle_file>
```

--- ## Configuration

### API Port Auto-Discovery

**No configuration needed!** CLI automatically finds the running node.

**How it works:**
1. Checks `MURMURATION_API_PORT` env var (if set)
2. Reads `~/.murmuration_api_port` (last node saves its port here)
3. Tries default port `17080` (most common: 8080 → 17080)
4. Scans ports 17080-17089

**API Port Formula:** `9000 + P2P_PORT`
- P2P port 8080 → API port 17080
- P2P port 8081 → API port 17081

**Override if needed:**
```bash
MURMURATION_API_PORT=17081 mur status
```

### Environment Variables

```bash
MURMURATION_API_PORT=17080  # API port
MURMURATION_DISCOVERY_PORT=9998  # Discovery port (default)
MURMURATION_NO_DISCOVERY=1  # Disable mDNS discovery
MURMURATION_MAX_CONNECTIONS=10  # Max peer connections
MURMURATION_CONNECT_COOLDOWN_MS=5000  # Connection retry cooldown
```

### Data Directory

Node data is stored in `.mur/node-<port>/`:
- `identity.json` - Node ID and keys
- `content.db` - Published content
- `messages.db` - Message history
- `names.db` - Name registry
- `peers.cache` - Discovered peers

--- ## Use Cases

### 1. Offline Messenger
Run nodes on phones/laptops with WiFi Direct, exchange messages without internet.

### 2. Censorship Bypass
Use bundles to transfer messages via USB/SD card when network is blocked.

### 3. Delay-Tolerant Networking
Messages are stored and forwarded when peers come online.

### 4. Content Publishing
Publish websites/files that propagate through the mesh.

--- ## Troubleshooting

### Port already in use

```bash
# Check what's using the port
lsof -i :8080

# Kill old nodes
killall mur core
```

### Nodes not connecting

1. Check logs: `RUST_LOG=info mur start 8080`
2. Try connecting explicitly: `mur start 8081 127.0.0.1:8080`
3. Check firewall settings

### API not found

```bash
# CLI tries ports 17070-17100 automatically
# Or set explicitly:
MURMURATION_API_PORT=17080 mur status
```

### Messages not showing in inbox

1. Check node is running: `mur status`
2. Wait for discovery (~5 seconds)
3. Check API port matches node port

--- ## Next Steps

- Read [PROTOCOL.md](PROTOCOL.md) for wire protocol details
- Read [ARCHITECTURE.md](ARCHITECTURE.md) for system design
- See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) for common issues

**Ready to build on Murmuration?** The platform is stable. Build messengers, websites, search engines on top of it.
