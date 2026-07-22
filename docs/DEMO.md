# Murmuration Complete Demo

**Full test of all features in 10 minutes**

## Prerequisites

Install `mur` first:
```bash
make install
# Or: cargo install --path core --bin mur
```

--- ## Setup (3 terminals)

### Terminal 1: Node Alice (8080)
```bash
mur start 8080
```

Copy Alice's node_id from output:
```
INFO: Created new node with ID: Qm7xRJ...
```

Save it:
```bash
ALICE_ID="Qm7xRJ..."  # Replace with actual ID
```

--- ### Terminal 2: Node Bob (8081)
```bash
mur start 8081 127.0.0.1:8080
```

Copy Bob's node_id:
```bash
BOB_ID="Qm8xSK..."  # Replace with actual ID
```

Wait for connection message:
```
INFO: Connected to peer 127.0.0.1:8080
```

--- ## Demo Script

### Terminal 3: Run all commands

#### 1. Check network status

```bash
# Alice's node (CLI auto-discovers port 17080)
mur status
mur peers

# Bob's node (switch to Terminal 2 or specify port)
MURMURATION_API_PORT=17081 mur status
MURMURATION_API_PORT=17081 mur peers
```

**Expected:** Both nodes see each other as connected.

--- #### 2. Messaging

```bash
# Alice broadcasts (CLI finds port 17080 automatically)
mur broadcast "Hello from Alice!"

# Bob checks inbox (run in Terminal 2, or specify port)
MURMURATION_API_PORT=17081 mur inbox 10
```

**Expected:** Bob sees Alice's message.

```bash
# Bob sends to Alice (in Terminal 2 or with port)
MURMURATION_API_PORT=17081 mur send $ALICE_ID "Hi Alice, this is Bob!"

# Alice checks inbox (Terminal 1)
mur inbox 10
```

**Expected:** Alice sees Bob's direct message.

--- #### 3. Live watch (keep running in background)

```bash
# In Terminal 1 or a 4th terminal:
mur watch
```

Send more messages and watch them appear in real-time!

--- #### 4. Content Publishing

```bash
# Alice publishes a website (Terminal 1)
mur publish site/index.html "<h1>Alice's Site</h1><p>Welcome to Murmuration!</p>"

# Copy the URL from output:
# Content published at: mur://Qm7xRJ.../site/index.html

# Bob fetches it (Terminal 2)
MURMURATION_API_PORT=17081 mur fetch mur://$ALICE_ID/site/index.html
```

**Expected:** Bob retrieves Alice's content.

--- #### 5. Publish from file

```bash
# Create a test file
echo "body { color: blue; }" > /tmp/style.css

# Alice publishes it (Terminal 1)
mur publish site/style.css @/tmp/style.css

# Bob fetches it (Terminal 2)
MURMURATION_API_PORT=17081 mur fetch mur://$ALICE_ID/site/style.css
```

**Expected:** Bob gets the CSS file.

--- #### 6. Naming System

```bash
# Alice registers her name (Terminal 1)
mur name register alice $ALICE_ID

# Bob registers his name (Terminal 2)
MURMURATION_API_PORT=17081 mur name register bob $BOB_ID

# Resolve names (Terminal 1)
mur name resolve alice
mur name resolve bob
```

**Expected:**
- `alice` resolves to Alice's node_id
- `bob` resolves to Bob's node_id

**Note:** Currently naming is local-only (no network propagation yet).

--- #### 7. Bundle Protocol (USB Transfer Simulation)

```bash
# Alice sends more messages (Terminal 1)
mur broadcast "Message 1 for bundle"
mur broadcast "Message 2 for bundle"
mur broadcast "Message 3 for bundle"

# Alice exports to bundle
mur bundle export /tmp/alice_bundle.bin
```

**Expected:** ` Bundle exported: 3+ messages`

```bash
# Check bundle info
mur bundle info /tmp/alice_bundle.bin
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
MURMURATION_API_PORT=17081 mur bundle import /tmp/alice_bundle.bin
```

**Expected:** ` Bundle imported: 3 delivered, 0 forwarded`

```bash
# Bob checks inbox
MURMURATION_API_PORT=17081 mur inbox 10
```

**Expected:** Bob sees all bundled messages.

--- #### 8. Interactive Chat

```bash
# Bob starts interactive chat (Terminal 2)
MURMURATION_API_PORT=17081 mur chat $ALICE_ID
```

Type messages and press Enter. They appear in Alice's `watch` terminal (from step 3).

Press `Ctrl+C` to exit chat.

--- #### 9. Ping Test

```bash
# Alice pings Bob (Terminal 1)
mur ping $BOB_ID

# Bob pings Alice (Terminal 2)
MURMURATION_API_PORT=17081 mur ping $ALICE_ID
```

**Expected:**
```
 Pong from Qm... in 2.34 ms
```

--- ## Summary

 **What we tested:**

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

--- ## Real-World Scenarios

### Scenario 1: Protest Coordination (No Internet)
1. Protesters run nodes on phones (WiFi Direct)
2. Messages propagate via mesh
3. When phones die, export bundle to USB
4. Import on fresh phones to continue

### Scenario 2: Censored Country
1. Run node at home
2. Publish news/content to mesh
3. Friends fetch via `mur://` URLs
4. Content propagates without DNS/ISP

### Scenario 3: Emergency Communication
1. Disaster cuts internet/cell towers
2. Local mesh forms via WiFi/Bluetooth
3. Bundles transferred via runners with USB drives
4. Messages reach remote areas

--- ## Cleanup

```bash
# Stop all nodes
killall mur core

# Remove test data (optional)
rm -rf .mur/
rm /tmp/alice_bundle.bin /tmp/style.css
```

--- ## Next: Build Your App

Murmuration is now a **stable platform**. Build on top of it:

- **Messenger** - Web UI or native app
- **Social Network** - Decentralized posts/feed
- **File Sharing** - BitTorrent-style over Murmuration
- **Website Hosting** - Static sites via `mur://`
- **Search Engine** - DHT-based content discovery

**The foundation is complete. Now build the future.**

