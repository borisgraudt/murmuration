use crate::error::{MeshError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;

const PEERS_FILE: &str = "peers.json";
const MAX_PEERS: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PeersFileV1 {
    version: u8,
    peers: Vec<String>,
}

fn peers_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join(PEERS_FILE)
}

/// Load cached peers from `.ely/.../peers.json` (best-effort).
pub fn load_cached_peers(data_dir: &Path) -> Result<Vec<SocketAddr>> {
    let path = peers_path(data_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(MeshError::Io)?;
    let parsed: PeersFileV1 = serde_json::from_str(&raw).map_err(MeshError::Serialization)?;
    if parsed.version != 1 {
        return Err(MeshError::Config(format!(
            "Unsupported peers file version: {}",
            parsed.version
        )));
    }

    let mut out = Vec::new();
    for p in parsed.peers {
        if let Ok(addr) = p.parse::<SocketAddr>() {
            out.push(addr);
        }
    }
    Ok(out)
}

/// Persist a discovered peer to `.ely/.../peers.json` (best-effort).
pub fn record_peer(data_dir: &Path, addr: SocketAddr) -> Result<()> {
    fs::create_dir_all(data_dir).map_err(MeshError::Io)?;
    let path = peers_path(data_dir);

    let mut peers: Vec<String> = if path.exists() {
        let raw = fs::read_to_string(&path).map_err(MeshError::Io)?;
        let parsed: PeersFileV1 = serde_json::from_str(&raw).map_err(MeshError::Serialization)?;
        parsed.peers
    } else {
        Vec::new()
    };

    let s = addr.to_string();
    if !peers.iter().any(|x| x == &s) {
        peers.push(s);
    }

    // Keep last MAX_PEERS entries (prefer newer)
    if peers.len() > MAX_PEERS {
        peers = peers.split_off(peers.len() - MAX_PEERS);
    }

    let file = PeersFileV1 { version: 1, peers };
    let json = serde_json::to_string_pretty(&file).map_err(MeshError::Serialization)?;
    fs::write(&path, json).map_err(MeshError::Io)?;
    Ok(())
}
