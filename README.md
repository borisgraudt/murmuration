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
- **Encryption**: RSA-2048 key exchange + AES-256-GCM session encryption
- **🧠 AI Routing**: Self-learning adaptive routing based on latency, uptime, reliability, and route history
- **🔐 PQC Encryption**: Post-quantum cryptography support (Kyber768 planned)
- **Peer Discovery**: Automatic LAN/Wi-Fi peer discovery
- **Mesh Sites**: Decentralized websites hosted on the network
- **Web Dashboard**: Real-time network visualization and chat
- **CLI Interface**: Beautiful Rust and Python CLIs with rich terminal UI

## 🧠 AI Routing

MeshLink uses self-learning adaptive routing to optimize message delivery:

- **Peer Scoring**: Calculates scores based on latency, uptime, reliability, and route success rate
- **Adaptive Learning**: Uses exponential moving average (α=0.7, β=0.3) to blend historical and current performance
- **Top-N Selection**: Forwards messages to top 3 peers based on scores
- **Route History**: Tracks success/failure rates for continuous improvement

See [docs/ai_routing.md](docs/ai_routing.md) for detailed algorithm description.

## 🔐 PQC Encryption

Post-quantum cryptography support for future-proof security:

- **Current**: RSA-2048 OAEP for key exchange (quantum-vulnerable but fast)
- **Planned**: Kyber768 for post-quantum security (NIST-standardized)
- **Hybrid Approach**: RSA for compatibility, PQC for future-proofing
- **Fallback**: Automatic fallback to RSA if PQC unavailable

See [docs/crypto_benchmark.md](docs/crypto_benchmark.md) for performance benchmarks.

## 🎥 Demo

### Quick Demo

```bash
# Run demo script (3 nodes)
./scripts/demo_local.sh
```

### Manual Demo

```bash
# Terminal 1: Node 1
cargo run --bin core --release -- 8082

# Terminal 2: Node 2
cargo run --bin core --release -- 8083 127.0.0.1:8082

# Terminal 3: Node 3
cargo run --bin core --release -- 8084 127.0.0.1:8082

# Terminal 4: Send message
MESHLINK_API_PORT=17082 cargo run --bin cli -- broadcast "MeshNet AI+PQC demo"
```

### Visualization

```bash
# Run network visualization
cargo run --bin viz
```

## 📜 Whitepaper

Read the full technical whitepaper: [docs/whitepaper_v1.md](docs/whitepaper_v1.md)

**Highlights**:
- Problem statement (centralization, censorship, quantum threat)
- Architecture and protocol design
- AI-routing algorithm with adaptive learning
- Post-quantum cryptography implementation
- Test results and performance metrics
- Future work and roadmap

## 📚 Documentation

See `docs/` directory for:
- `whitepaper_v1.md` - Full technical whitepaper
- `architecture.md` - System architecture
- `protocol_spec.md` - Protocol specification
- `ai_routing.md` - AI routing algorithm details
- `crypto_benchmark.md` - Cryptographic performance benchmarks
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

