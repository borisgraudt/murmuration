## Elysium

**The Internet Without Internet**

Decentralized, censorship-resistant mesh network. Works offline.

### What is this?

Not a messenger. Not blockchain. **A new network layer.**

- ✅ Works without internet
- ✅ Censorship-resistant
- ✅ End-to-end encrypted
- ✅ Delay-tolerant (hours/days)
- ✅ Content-addressed

**Use cases:** Protest coordination, emergency communication, bypassing censorship.

### Features

- 🔐 **E2E Encryption** - RSA + AES-GCM
- 🌐 **P2P Mesh** - Direct connections, no servers
- 📡 **Auto-Discovery** - mDNS local network discovery
- 💬 **Messaging** - Direct, broadcast, persistent inbox
- 📦 **Content Addressing** - Publish/fetch via `ely://` URLs
- 🏷️ **Naming System** - Human-readable names
- 💾 **Store-and-Forward** - Bundle protocol for offline transfer
- 🔄 **Auto-Reconnect** - Resilient connections
- 📊 **Live Streaming** - Real-time message watch

### Repo layout

```text
core/         Rust node + binaries (core, cli, viz, ely)
python_cli/   Python CLI (Claude Code-ish terminal UI)
web/frontend/ Static web topology (GitHub Pages-ready)
docs/         Protocol + architecture notes
scripts/      Local run helpers
```

### Quick demo

**Start a node:**
```bash
cd core
cargo run --bin ely --release -- start 8080
```

**Connect another node:**
```bash
cargo run --bin ely --release -- start 8081 127.0.0.1:8080
```

**Send messages:**
```bash
# In another terminal
MESHLINK_API_PORT=17080 cargo run --bin ely -- broadcast "hello mesh"
MESHLINK_API_PORT=17080 cargo run --bin ely -- inbox 10
MESHLINK_API_PORT=17080 cargo run --bin ely -- watch
```

**Publish content:**
```bash
MESHLINK_API_PORT=17080 cargo run --bin ely -- publish site/index.html "<h1>Hello World</h1>"
MESHLINK_API_PORT=17080 cargo run --bin ely -- fetch ely://<node_id>/site/index.html
```

**Register names:**
```bash
MESHLINK_API_PORT=17080 cargo run --bin ely -- name register alice <node_id>
MESHLINK_API_PORT=17080 cargo run --bin ely -- name resolve alice
```

**Export/import bundles:**
```bash
MESHLINK_API_PORT=17080 cargo run --bin ely -- bundle export /tmp/messages.bundle
MESHLINK_API_PORT=17081 cargo run --bin ely -- bundle import /tmp/messages.bundle
```

**That's it.** No servers, no cloud, no accounts.

### Install without Docker

If you have Rust installed, you can install `ely` into your PATH:

```bash
cargo install --git https://github.com/borisgraudt/elysium.git --package meshlink_core --bin ely
```

Then:

```bash
ely start 8080
```

### Notes (frugal but important)

- **API port formula**: `MESHLINK_API_PORT = 9000 + P2P_PORT` (e.g. 8080 → 17080).
- If you see `Address already in use`, stop old nodes: `killall core` (macOS/Linux).
- If peers don’t connect, see `docs/TROUBLESHOOTING.md`.

### Documentation

- **[Quick Demo](docs/DEMO.md)** — 10-minute full feature test
- **[Quickstart](docs/QUICKSTART.md)** — getting started guide
- **[Protocol Spec](docs/PROTOCOL.md)** — wire protocol, addressing, security
- **[Architecture](docs/ARCHITECTURE.md)** — layers, components, design
- **[Troubleshooting](docs/TROUBLESHOOTING.md)** — common issues

### CI / Pages

- Rust CI: `.github/workflows/rust.yml`
- GitHub Pages deploy: `.github/workflows/pages.yml` (deploys `web/frontend/`)
- GitHub Packages (GHCR): `.github/workflows/packages.yml` (pushes Docker image to `ghcr.io/<owner>/<repo>`)

### GitHub Packages (GHCR) usage

Pull:

```bash
docker pull ghcr.io/borisgraudt/elysium:main
```

Run a node (example P2P port 8080):

```bash
docker run --rm -it \
  -p 8080:8080 \
  -p 9998:9998/udp \
  ghcr.io/borisgraudt/elysium:main start 8080
```

