# **MeshNet: Decentralized Encrypted Communication Protocol**

> **A fully peer-to-peer, censorship-resistant communication layer** designed for autonomy, privacy, and resilience — built with **Rust** and **Python AI integration**.

---

## 🧠 Vision

Modern communication relies on centralized servers that can be censored, surveilled, or shut down.  
**MeshNet** redefines this paradigm — creating a *fully decentralized, encrypted, and intelligent* communication protocol where nodes cooperate, route messages autonomously, and survive even under complete internet isolation.

This project aims to explore how **AI-driven routing, cryptographic identity, and mesh topology** can merge into one resilient communication network.

---

## ⚙️ Architecture Overview

meshnet/
├── core/ # Core Rust-based P2P protocol
│ ├── Cargo.toml
│ └── src/
|   ├── main.rs
|   ├── lib.rs
|   ├── p2p/
│   | ├── discovery.rs
│   | ├── encryption.rs
│   | ├── peer.rs
│   | └── protocol.rs
|   └── utils/
│     ├── config.rs
│     ├── crypto.rs
│     └── logger.rs
|
├── docs/
│ ├── architecture.md
│ ├── protocol_spec.md
│ └── roadmap.md
│
└── README.md
---

## 🔒 Core Protocol Design

Each node in **MeshNet** acts as both a **client and a server**.  
Connections are authenticated via asymmetric cryptography, and messages are routed through a *multi-hop encrypted mesh.*

**Key principles:**
- **No central authority** — every peer participates equally.
- **End-to-end encryption** with rotating session keys.
- **Adaptive routing** — AI chooses optimal relays based on latency and trust score.
- **Resilience** — works in isolated LAN or Wi-Fi Direct environments.

**Handshake example:**
Node A ---> SYN + PubKeyA
Node B ---> ACK + PubKeyB + Signature
Node A ---> Encrypted session init
Secure channel established 🔐

---

## 🧩 AI Integration

The **AI router** monitors the network and:
- Predicts node reliability and packet loss.
- Learns from topology changes.
- Suggests optimal paths in near real-time.

Planned extension: integration of **federated learning** to allow each node to improve global routing without sharing raw data.

---

## 🧰 CLI Interface

The Python CLI provides a minimal shell-like environment:
meshnet> connect peer123@192.168.1.12
meshnet> send "hello world"
meshnet> peers
meshnet> status

You can chat, monitor routes, and even deploy custom modules for testing encryption or routing.

---

## 🧪 Future Goals

- 🌍 **Quantum-resistant encryption** (NTRU or Kyber)  
- 🧩 **Federated routing optimization**  
- 🔌 **Offline mesh bootstrap via Bluetooth or LoRa**  
- 🧱 **Full self-healing topology**  

---

## 💡 Research Impact

This project demonstrates:
- Real-world application of **distributed systems** and **cryptography**.
- Practical design of a **protocol stack** from scratch.
- Integration of **machine learning** into network routing.

**Potential applications:**
- Emergency communication networks.  
- Encrypted IoT swarms.  
- Decentralized cloud foundations.

---

## 🧬 Credits

**Protocol & Cryptography:** Boris Graudt
**CLI & AI Systems:** Ivan Shatalov
Built for research and innovation — *inspired by autonomy, resilience, and freedom.*

---

## 📚 License

MIT License © 2025
