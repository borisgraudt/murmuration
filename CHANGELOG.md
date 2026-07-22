# Changelog

All notable changes to Murmuration will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-07-22 — Routing study + published crate

### Added
- **Routing research** (`results/RESULTS.md`): derived the exact
  *destination-agnostic ceiling* of any peer-keyed bandit router; showed the
  shipped UCB1 saturates it (~6× below a destination-aware oracle) with linear
  regret; showed **Q-routing breaks the ceiling** (~2× delivery under
  concentrated traffic, disjoint CIs); added a hyperparameter robustness sweep and
  a delay-tolerant contact-trace benchmark. Seven publication figures.
- **`murmuration-routing` crate** published to crates.io — a dependency-light DTN
  evaluation toolkit (contact traces, heavy-tailed synthetic mobility, exact
  foremost-journey oracle).
- **Q-routing** in the real `Router` (`q_select_toward`, `q_advertised_value`,
  `q_record`) with unit tests, plus the `RoutingEstimate` protocol message.
  Live-node wiring documented as experimental (`docs/Q_ROUTING.md`).
- Paper draft (`paper/`), Docker Compose demo (`docker-compose.yml`, `demo/`).

### Changed
- **Renamed** the project elysium → **Murmuration**: crate `meshlink_core` →
  `murmuration`, URL scheme `ely://` → `mur://`, binary `ely` → `mur`, env vars
  `MESHLINK_*`/`ELYSIUM_*` → `MURMURATION_*`.

### Removed
- Stale `docs/roadmap.md`; the superseded `simulate` benchmark binary; emoji from
  docs; large regenerable data files.

## [0.1.0] - 2024-12 — MVP Release

### Added
- **Core P2P Protocol**
  - TCP-based peer-to-peer connections
  - Secure handshake with RSA-2048 key exchange
  - AES-256-GCM session encryption
  - Heartbeat mechanism for connection keepalive
  - Automatic peer discovery via UDP broadcast

- **AI-Driven Adaptive Routing**
  - Peer scoring based on latency, uptime, reliability
  - Route history tracking for adaptive learning
  - Exponential moving average for score updates
  - Top-N peer selection (default: 3 peers)
  - Loop prevention via path tracking
  - TTL-based message expiration

- **CLI Interface**
  - Rust CLI with colored output
  - Python CLI with rich terminal UI
  - Interactive REPL mode
  - Commands: send, broadcast, peers, status

- **Network Visualization**
  - TUI application for real-time network topology
  - Message flow visualization
  - Node connection status
  - UDP-based event streaming

- **AI-Routing Logging**
  - JSONL log format for ML training
  - Routing decisions with peer metrics
  - Message context (TTL, path, broadcast/directed)

- **Post-Quantum Cryptography (Planned)**
  - PQC module structure (encryption_pqc.rs)
  - Kyber768 support (planned, not yet stable)
  - Fallback to RSA for compatibility

- **Documentation**
  - Whitepaper v1.0
  - Architecture documentation
  - Protocol specification
  - Demo scripts

- **CI/CD**
  - GitHub Actions workflow
  - Formatting checks (cargo fmt)
  - Linting (cargo clippy)
  - Test suite
  - Release builds

### Changed
- Improved handshake protocol (incoming vs outgoing connections)
- Enhanced message forwarding with AI-routing
- Better error handling and logging
- Graceful shutdown on Ctrl+C

### Fixed
- Connection state management
- Message channel cleanup
- Handshake race conditions
- Port conflicts in tests

### Security
- RSA-2048 OAEP for key exchange
- AES-256-GCM for message encryption
- Authenticated encryption (integrity + confidentiality)
- Post-quantum cryptography planned (Kyber768)

---

## [Unreleased]

### Planned
- Complete PQC implementation (Kyber768)
- Web interface (Murmuration)
- Mobile clients
- Scalability improvements (1000+ nodes)
- Performance optimizations
- Security audit

---

[1.0.0]: https://github.com/murmuration/murmuration/releases/tag/v1.0.0


