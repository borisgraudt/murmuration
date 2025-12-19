# MeshLink Python CLI

Beautiful command-line interface for MeshLink nodes in Claude Code style.

## Installation

### Option 1: Using pip (recommended)

```bash
pip3 install rich
```

### Option 2: Using virtual environment

```bash
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install -r requirements.txt
```

### Option 3: System-wide (may require --break-system-packages)

```bash
pip3 install --break-system-packages rich
```

## Usage

### Interactive Mode (Recommended)

```bash
python3 cli.py -i
# or
python3 cli.py --interactive
# or
python3 cli.py repl
```

This will start an interactive REPL with a beautiful terminal interface.

### Command Mode

```bash
# Show status
python3 cli.py status

# List peers
python3 cli.py peers

# Send message to specific peer
python3 cli.py send <peer_id> "Hello!"

# Broadcast message
python3 cli.py broadcast "Hello everyone!"
```

## Features

- 🎨 **Claude Code inspired design** - Dark theme with beautiful colors
- 📊 **Rich peer information** - Detailed peer status with color coding
- 💬 **Interactive REPL mode** - Beautiful command-line interface
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

- `status` - Show node status
- `peers` - List all connected peers
- `send <peer_id> <message>` - Send message to specific peer
- `broadcast <message>` - Broadcast message to all peers
- `help` - Show help (in interactive mode)
- `exit` - Exit interactive mode
