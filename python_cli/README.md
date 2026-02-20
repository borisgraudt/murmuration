# Elysium Python CLI & TUI

Beautiful command-line and terminal user interfaces for Elysium nodes, inspired by Claude Code.

## Installation

### Recommended: Using virtual environment

```bash
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install -r requirements.txt
```

### Alternative: System-wide installation

```bash
pip3 install rich textual
# Or with --break-system-packages if needed
pip3 install --break-system-packages rich textual
```

## Usage

### 🎨 Advanced TUI (NEW - Recommended!)

The most beautiful and feature-rich interface:

```bash
python3 advanced_tui.py
```

Features:
- **Real-time panels:** Status, Peers, Messages
- **Live updates:** Auto-refreshing node status and peer list
- **Message streaming:** Real-time message feed
- **Command history:** Navigate with ↑/↓ arrows
- **Keyboard shortcuts:** Ctrl+C (quit), Ctrl+L (clear), Ctrl+R (refresh)
- **Claude Code theme:** Dark mode with orange accents

### 📟 Simple TUI

Basic TUI interface:

```bash
python3 tui.py
```

### 💻 Interactive CLI (REPL Mode)

```bash
python3 cli.py -i
# or
python3 cli.py --interactive
# or
python3 cli.py repl
```

This starts an interactive REPL with a beautiful terminal interface.

### ⚡ Command Mode

Quick commands without interactive mode:

```bash
# Show status
python3 cli.py status

# List peers
python3 cli.py peers

# Send message to specific peer
python3 cli.py send <peer_id> "Hello!"

# Broadcast message
python3 cli.py broadcast "Hello everyone!"

# Show inbox messages
python3 cli.py inbox 20

# Watch for new messages (live stream)
python3 cli.py watch
```

## Features

### Advanced TUI (advanced_tui.py)
- 🎨 **Claude Code inspired design** - Dark theme with orange (#f0883e) accents
- 📊 **Multi-panel layout** - Status, Peers, Messages panels
- 🔄 **Real-time updates** - Auto-refreshing status and peer list
- 💬 **Live message stream** - Watch messages appear in real-time
- ⌨️ **Command history** - Navigate with arrow keys
- 🎯 **Keyboard shortcuts** - Ctrl+C, Ctrl+L, Ctrl+R, F1
- ✨ **Modern TUI** - Built with Textual framework

### CLI & Simple TUI
- 💻 **Interactive REPL mode** - Beautiful command-line interface
- 📊 **Rich peer information** - Detailed peer status with color coding
- 🔍 **Automatic API port discovery** - Finds running nodes automatically
- ✨ **Modern UI** - Panels, tables, and beautiful formatting

## Screenshots

The CLI features:
- Dark theme matching Claude Code aesthetic
- Color-coded status indicators
- Beautiful tables and panels
- Smooth animations and transitions
- Clear error messages and help text

## Configuration

The CLI automatically discovers API ports. You can also set it manually:

```bash
export MESHLINK_API_PORT=17080
python3 cli.py status
```

## Commands

### In TUI/REPL mode:
- `status` - Show detailed node status
- `peers` - List all connected peers with details
- `send <peer_id> <message>` - Send message to specific peer
- `broadcast <message>` - Broadcast message to all peers
- `inbox [n]` - Show last N messages (default: 20)
- `watch` - Live stream incoming messages (CLI only)
- `clear` - Clear message stream (TUI only)
- `refresh` - Refresh status and peers (Advanced TUI only)
- `help` - Show available commands
- `quit` or `exit` - Exit

### Keyboard Shortcuts (Advanced TUI):
- `Ctrl+C` - Quit application
- `Ctrl+L` - Clear message stream
- `Ctrl+R` - Refresh status and peers
- `F1` - Show help
- `↑/↓` - Navigate command history

## Comparison

| Feature | CLI | Simple TUI | Advanced TUI |
|---------|-----|------------|--------------|
| Interactive mode | ✅ | ✅ | ✅ |
| Command mode | ✅ | ❌ | ❌ |
| Real-time panels | ❌ | ❌ | ✅ |
| Auto-refresh | ❌ | ❌ | ✅ |
| Message streaming | ✅ | ✅ | ✅ |
| Command history | ❌ | ❌ | ✅ |
| Keyboard shortcuts | ❌ | ❌ | ✅ |
| Split-pane layout | ❌ | ❌ | ✅ |

**Recommendation:** Use `advanced_tui.py` for the best experience!
