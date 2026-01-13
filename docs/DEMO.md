# Elysium Complete Demo

**Full test of all features in 10 minutes**

## Prerequisites

Install `ely` first:
```bash
make install
# Or: cargo install --path core --bin ely
```

---

## Setup (3 terminals)

### Terminal 1: Node Alice (8080)
```bash
ely start 8080
```

Copy Alice's node_id from output:
```
INFO: Created new node with ID: Qm7xRJ...
```

Save it:
```bash
ALICE_ID="Qm7xRJ..."  # Replace with actual ID
```

---

### Terminal 2: Node Bob (8081)
```bash
ely start 8081 127.0.0.1:8080
```

Copy Bob's node_id:
```bash
BOB_ID="Qm8xSK..."  # Replace with actual ID
```

Wait for connection message:
```
INFO: Connected to peer 127.0.0.1:8080
```

---

## Demo Script

### Terminal 3: Run all commands

#### 1. Check network status

```bash
# Alice's node (CLI auto-discovers port 17080)
ely status
ely peers

# Bob's node (switch to Terminal 2 or specify port)
MESHLINK_API_PORT=17081 ely status
MESHLINK_API_PORT=17081 ely peers
```

**Expected:** Both nodes see each other as connected.

---

#### 2. Messaging

```bash
# Alice broadcasts (CLI finds port 17080 automatically)
ely broadcast "Hello from Alice!"

# Bob checks inbox (run in Terminal 2, or specify port)
MESHLINK_API_PORT=17081 ely inbox 10
```

**Expected:** Bob sees Alice's message.

```bash
# Bob sends to Alice (in Terminal 2 or with port)
MESHLINK_API_PORT=17081 ely send $ALICE_ID "Hi Alice, this is Bob!"

# Alice checks inbox (Terminal 1)
ely inbox 10
```

**Expected:** Alice sees Bob's direct message.

---

#### 3. Live watch (keep running in background)

```bash
# In Terminal 1 or a 4th terminal:
ely watch
```

Send more messages and watch them appear in real-time!

---

#### 4. Content Publishing

```bash
# Alice publishes a website (Terminal 1)
ely publish site/index.html "<h1>Alice's Site</h1><p>Welcome to Elysium!</p>"

# Copy the URL from output:
# ✓ Content published at: ely://Qm7xRJ.../site/index.html

# Bob fetches it (Terminal 2)
MESHLINK_API_PORT=17081 ely fetch ely://$ALICE_ID/site/index.html
```

**Expected:** Bob retrieves Alice's content.

---

#### 5. Publish from file

```bash
# Create a test file
echo "body { color: blue; }" > /tmp/style.css

# Alice publishes it (Terminal 1)
ely publish site/style.css @/tmp/style.css

# Bob fetches it (Terminal 2)
MESHLINK_API_PORT=17081 ely fetch ely://$ALICE_ID/site/style.css
```

**Expected:** Bob gets the CSS file.

---

#### 6. Naming System

```bash
# Alice registers her name (Terminal 1)
ely name register alice $ALICE_ID

# Bob registers his name (Terminal 2)
MESHLINK_API_PORT=17081 ely name register bob $BOB_ID

# Resolve names (Terminal 1)
ely name resolve alice
ely name resolve bob
```

**Expected:**
- `alice` resolves to Alice's node_id
- `bob` resolves to Bob's node_id

**Note:** Currently naming is local-only (no network propagation yet).

---

#### 7. Bundle Protocol (USB Transfer Simulation)

```bash
# Alice sends more messages (Terminal 1)
ely broadcast "Message 1 for bundle"
ely broadcast "Message 2 for bundle"
ely broadcast "Message 3 for bundle"

# Alice exports to bundle
ely bundle export /tmp/alice_bundle.bin
```

**Expected:** `✓ Bundle exported: 3+ messages`

```bash
# Check bundle info
ely bundle info /tmp/alice_bundle.bin
```

**Expected:**
```
Bundle Info:
  Messages: 3
  Created:  2024-01-13T...
  Expires:  2024-01-20T...
  Expired:  NO
```

```bash
# Simulate USB transfer: Bob imports the bundle (Terminal 2)
MESHLINK_API_PORT=17081 ely bundle import /tmp/alice_bundle.bin
```

**Expected:** `✓ Bundle imported: 3 delivered, 0 forwarded`

```bash
# Bob checks inbox
MESHLINK_API_PORT=17081 ely inbox 10
```

**Expected:** Bob sees all bundled messages.

---

#### 8. Interactive Chat

```bash
# Bob starts interactive chat (Terminal 2)
MESHLINK_API_PORT=17081 ely chat $ALICE_ID
```

Type messages and press Enter. They appear in Alice's `watch` terminal (from step 3).

Press `Ctrl+C` to exit chat.

---

#### 9. Ping Test

```bash
# Alice pings Bob (Terminal 1)
ely ping $BOB_ID

# Bob pings Alice (Terminal 2)
MESHLINK_API_PORT=17081 ely ping $ALICE_ID
```

**Expected:**
```
✓ Pong from Qm... in 2.34 ms
```

---

## Summary

✅ **What we tested:**

1. **P2P Connection** - Two nodes connected via TCP
2. **Discovery** - Automatic peer discovery via mDNS
3. **Messaging** - Direct send + broadcast
4. **Live Watch** - Real-time message streaming
5. **Content Publishing** - Store and fetch content
6. **File Upload** - Publish from local files
7. **Naming** - Human-readable names (local)
8. **Bundles** - Store-and-forward via file export/import
9. **Interactive Chat** - TUI-style messaging
10. **Ping** - Latency measurement

---

## Real-World Scenarios

### Scenario 1: Protest Coordination (No Internet)
1. Protesters run nodes on phones (WiFi Direct)
2. Messages propagate via mesh
3. When phones die, export bundle to USB
4. Import on fresh phones to continue

### Scenario 2: Censored Country
1. Run node at home
2. Publish news/content to mesh
3. Friends fetch via `ely://` URLs
4. Content propagates without DNS/ISP

### Scenario 3: Emergency Communication
1. Disaster cuts internet/cell towers
2. Local mesh forms via WiFi/Bluetooth
3. Bundles transferred via runners with USB drives
4. Messages reach remote areas

---

## Cleanup

```bash
# Stop all nodes
killall ely core

# Remove test data (optional)
rm -rf .ely/
rm /tmp/alice_bundle.bin /tmp/style.css
```

---

## Next: Build Your App

Elysium is now a **stable platform**. Build on top of it:

- **Messenger** - Web UI or native app
- **Social Network** - Decentralized posts/feed
- **File Sharing** - BitTorrent-style over Elysium
- **Website Hosting** - Static sites via `ely://`
- **Search Engine** - DHT-based content discovery

**The foundation is complete. Now build the future.** 🚀

