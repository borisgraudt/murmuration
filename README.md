# MeshNet: Decentralized Encrypted Communication Protocol

A fully peer-to-peer, censorship-resistant communication layer designed for autonomy, privacy, and resilience — built with Rust and Python AI integration.

## 🧠 Vision

Modern communication relies on centralized servers that can be censored, surveilled, or shut down. MeshNet redefines this paradigm — creating a fully decentralized, encrypted, and intelligent communication protocol where nodes cooperate, route messages autonomously, and survive even under complete internet isolation.

## ⚙️ Architecture

```
meshnet_20_10/
├── core/              # Rust P2P protocol with AI routing
├── python_cli/        # CLI for testing
├── web/               # Elysium Web (backend + frontend)
├── sites/             # Decentralized mesh sites
├── tests/             # Unit & integration tests
├── scripts/           # Helper scripts
└── docs/              # Documentation
```

## 🚀 Quick Start

### 1. Build Rust Core

```bash
cd core
cargo build --release
```

### 2. Run a Node

```bash
# Terminal 1: Node 1
cargo run --bin core 8080

# Terminal 2: Node 2
cargo run --bin core 8081 '127.0.0.1:8080'

# Terminal 3: Visualization
cargo run --bin viz
```

### 3. Use CLI

```bash
# Rust CLI
cargo run --bin cli -- status
cargo run --bin cli -- broadcast "Hello MeshNet!"

# Python CLI
cd python_cli
pip install -r requirements.txt
python cli.py status
python cli.py broadcast "Hello from Python!"
```

### 4. Run Web Interface

```bash
# Backend
cd web/backend
pip install fastapi uvicorn
python app.py

# Open browser
open http://localhost:8000
```

## 🔒 Features

- **P2P Networking**: Fully decentralized, no central servers
- **Encryption**: RSA key exchange + AES-GCM session encryption
- **AI Routing**: Intelligent message routing based on latency, uptime, and trust
- **Peer Discovery**: Automatic LAN/Wi-Fi peer discovery
- **Mesh Sites**: Decentralized websites hosted on the network
- **Web Dashboard**: Real-time network visualization and chat

## 📚 Documentation

See `docs/` directory for:
- `architecture.md` - System architecture
- `protocol_spec.md` - Protocol specification
- `ai_routing.md` - AI routing algorithm
- `web_spec.md` - Elysium Web specification
- `roadmap.md` - Development roadmap

## 🧪 Testing

```bash
# Run Rust tests
cd core
cargo test

# Run Python CLI tests
cd python_cli
python -m pytest tests/
```

## 📝 License

MIT License © 2025

## 🤝 Contributing

This is a research project. Contributions welcome!

