# Elysium Installation Guide

## Quick Install

### macOS / Linux

```bash
# Clone repo
git clone https://github.com/borisgraudt/elysium.git
cd elysium

# Install ely to ~/.cargo/bin
make install

# Start using
ely start 8080
```

---

## Installation Options

### 1. Make Install (Recommended)

```bash
make install
```

Installs `ely` to `~/.cargo/bin` (make sure it's in your `$PATH`).

**Add to PATH if needed:**
```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc  # or ~/.bashrc
source ~/.zshrc
```

---

### 2. Cargo Install from Source

```bash
cd elysium/core
cargo install --path . --bin ely
```

---

### 3. Cargo Install from GitHub

```bash
cargo install --git https://github.com/borisgraudt/elysium.git --package meshlink_core --bin ely
```

---

### 4. Manual Build + Symlink

```bash
cd elysium/core
cargo build --release

# Symlink to /usr/local/bin (requires sudo)
sudo ln -s $(pwd)/target/release/ely /usr/local/bin/ely

# Or without sudo (user bin directory)
mkdir -p ~/bin
ln -s $(pwd)/target/release/ely ~/bin/ely
export PATH="$HOME/bin:$PATH"  # Add to ~/.zshrc or ~/.bashrc
```

---

### 5. Docker

```bash
docker pull ghcr.io/borisgraudt/elysium:main

# Run a node
docker run --rm -it \
  -p 8080:8080 \
  -p 9998:9998/udp \
  ghcr.io/borisgraudt/elysium:main start 8080
```

---

## Verify Installation

```bash
# Check version/help
ely --help

# Start a test node
ely start 8080
```

In another terminal:
```bash
# CLI auto-discovers the API port!
ely status
ely peers
ely broadcast "Hello Elysium!"
ely inbox
```

---

## Troubleshooting

### `ely: command not found`

**Solution:** Add `~/.cargo/bin` to `$PATH`:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

---

### `MESHLINK_API_PORT` still required?

**Not anymore!** CLI auto-discovers running nodes.

**How it works:**
1. Checks `MESHLINK_API_PORT` env var (if set)
2. Reads `~/.elysium_api_port` (last node saves port here)
3. Tries default `17080` (8080 + 9000)
4. Scans 17080-17089

**Override if needed:**
```bash
MESHLINK_API_PORT=17081 ely status
```

---

### Port already in use

```bash
# Find process using port 8080
lsof -i :8080

# Kill old nodes
killall ely core
```

---

### Build fails

**Install Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

**Update Rust:**
```bash
rustup update
```

---

## Next Steps

- **[Quick Demo](docs/DEMO.md)** — 10-minute full feature test
- **[Quickstart](docs/QUICKSTART.md)** — Getting started guide
- **[README](README.md)** — Project overview

**Ready to build on Elysium?** The platform is stable. Start now! 🚀

