# Quick Start: Advanced TUI

Get started with the new Claude Code-inspired TUI in 2 minutes!

## Step 1: Install Dependencies

```bash
cd python_cli

# Create virtual environment (recommended)
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate

# Install packages
pip install -r requirements.txt
```

## Step 2: Start Murmuration Node

In another terminal:

```bash
# Start a node on port 8080
mur start 8080

# Or if mur is not installed:
cd ../core
cargo run --release --bin core -- 8080
```

## Step 3: Launch TUI

```bash
# Make sure you're in python_cli directory with venv activated
python3 advanced_tui.py
```

That's it!

## What You'll See

```
┌─────────────────────────────────────────┐
│  Murmuration - Decentralized Mesh Network │
└─────────────────────────────────────────┘

┌─────────────────┬─────────────────────────┐
│  Status  │ Message Stream  │
│ Node: abc123... │  Murmuration TUI  │
│ Peers: 2/5  │ Connected to node  │
│ Port: 17080  │ Type help for commands  │
│  │  │
│ Connected Peers │ » status  │
│ ● node1...  │  Status updated  │
│ ● node2...  │  │
└─────────────────┴─────────────────────────┘

» Command: _
```

## Quick Commands

Try these commands:

```
status  - Show node details
peers  - List connected peers
broadcast Hi - Send message to all
help  - Show all commands
clear  - Clear messages
quit  - Exit
```

## Keyboard Shortcuts

- **Ctrl+C** - Quit
- **Ctrl+L** - Clear messages
- **Ctrl+R** - Refresh
- **↑/↓** - Command history
- **F1** - Help

## Testing with Multiple Nodes

### Terminal 1: Node A
```bash
mur start 8080
```

### Terminal 2: Node B (connects to A)
```bash
mur start 8081 127.0.0.1:8080
```

### Terminal 3: TUI for Node A
```bash
cd python_cli
source venv/bin/activate
MURMURATION_API_PORT=17080 python3 advanced_tui.py
```

### Terminal 4: TUI for Node B
```bash
cd python_cli
source venv/bin/activate
MURMURATION_API_PORT=17081 python3 advanced_tui.py
```

Now send messages between them!

## Tips

1. **Auto-discovery**: TUI automatically finds running nodes
2. **Live updates**: Status and peers refresh automatically
3. **Message stream**: New messages appear in real-time
4. **History**: Use ↑/↓ to recall previous commands
5. **Multi-tasking**: Run multiple TUI instances for different nodes

## Troubleshooting

**"Could not find running node"**
- Make sure a node is running: `mur start 8080`
- Check API port: Formula is `9000 + node_port`
- Port 8080 → API 17080

**"Module not found: textual"**
```bash
pip install textual
```

**TUI is slow/laggy**
- Reduce update frequency (edit code if needed)
- Close other terminals
- Use fewer background processes

## Next Steps

- Try sending messages: `send <peer_id> Hello!`
- Broadcast to all: `broadcast Hello everyone!`
- Check inbox: `inbox 20`
- Explore the code: `advanced_tui.py`

Enjoy your beautiful mesh network interface!
