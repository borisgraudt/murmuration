# MeshLink Python CLI

Beautiful command-line interface for MeshLink nodes.

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

### Interactive Mode

```bash
python3 cli.py -i
# or
python3 cli.py --interactive
# or
python3 cli.py repl
```

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

- 🎨 Beautiful terminal UI with colors and tables
- 📊 Rich peer information display
- 💬 Interactive REPL mode
- 🔍 Automatic API port discovery


