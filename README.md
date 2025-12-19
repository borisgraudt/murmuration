## meshlink

Decentralized, encrypted P2P mesh node in Rust — with a CLI and a minimalist web topology view.

### What you should see at the end

- **Two+ nodes connect** (RSA handshake, AES-GCM session keys).
- **`peers` shows real connections** (Connected / Handshaking / Disconnected).
- **Sending a message** works (direct or broadcast).
- **Web topology page** shows nodes + edges, and animates message flow when we wire events (today it polls peers/status).

### Repo layout

```text
core/         Rust node + binaries (core, cli, viz, ely)
python_cli/   Python CLI (Claude Code-ish terminal UI)
web/frontend/ Static web topology (GitHub Pages-ready)
docs/         Protocol + architecture notes
scripts/      Local run helpers
```

### Quick demo (local)

Open **two terminals**:

```bash
cd core
MESHLINK_API_PORT=17080 cargo run --bin core --release -- 8080
```

```bash
cd core
MESHLINK_API_PORT=17081 cargo run --bin core --release -- 8081 127.0.0.1:8080
```

Then **CLI** (third terminal):

```bash
cd python_cli
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt

MESHLINK_API_PORT=17080 python3 cli.py -i
```

Try:
- `status`
- `peers`
- `broadcast hello`

Then **web view** (fourth terminal):

```bash
cd web/frontend
python3 -m http.server 8081
```

Open `http://localhost:8081`.

### Notes (frugal but important)

- **API port formula**: `MESHLINK_API_PORT = 9000 + P2P_PORT` (e.g. 8080 → 17080).
- If you see `Address already in use`, stop old nodes: `killall core` (macOS/Linux).
- If peers don’t connect, see `docs/TROUBLESHOOTING.md`.

### Documentation

- `docs/protocol_spec.md`
- `docs/architecture.md`
- `docs/ai_routing.md`

### CI / Pages

- Rust CI: `.github/workflows/rust.yml`
- GitHub Pages deploy: `.github/workflows/pages.yml` (deploys `web/frontend/`)

