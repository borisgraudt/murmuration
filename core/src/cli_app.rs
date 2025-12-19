use colored::*;
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

/// Shared CLI implementation for both `cli` and `ely` binaries.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let bin = args
        .first()
        .map(|s| s.as_str())
        .unwrap_or("ely")
        .to_string();

    if args.len() < 2 {
        print_usage(&bin);
        return Ok(());
    }

    let command = &args[1];

    match command.as_str() {
        "send" => {
            if args.len() < 4 {
                eprintln!("{}", format!("Usage: {} send <peer_id> <message>", bin).yellow());
                return Ok(());
            }
            let peer_id = Some(normalize_peer_id(&args[2]).to_string());
            let message = args[3..].join(" ");
            send_message(peer_id, message)?;
        }
        "broadcast" => {
            if args.len() < 3 {
                eprintln!("{}", format!("Usage: {} broadcast <message>", bin).yellow());
                return Ok(());
            }
            let message = args[2..].join(" ");
            send_message(None, message)?;
        }
        "ping" => {
            if args.len() < 3 {
                eprintln!("{}", format!("Usage: {} ping <peer_id> [timeout_ms]", bin).yellow());
                return Ok(());
            }
            let peer_id = normalize_peer_id(&args[2]).to_string();
            let timeout_ms = args
                .get(3)
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(1500);
            ping(peer_id, timeout_ms)?;
        }
        "peers" => {
            list_peers()?;
        }
        "status" => {
            show_status()?;
        }
        _ => {
            eprintln!("{} Unknown command: {}", "✗".red().bold(), command.red());
            print_usage(&bin);
        }
    }

    Ok(())
}

fn normalize_peer_id(s: &str) -> &str {
    s.strip_prefix("ely://").unwrap_or(s)
}

fn print_usage(bin: &str) {
    println!("{}", "⚡ MeshLink CLI".bright_cyan().bold());
    println!();
    println!("{}", "Usage:".bright_white().bold());
    println!("  {} <command> [args]", bin.cyan());
    println!();
    println!("{}", "Commands:".bright_white().bold());
    println!(
        "  {} <peer_id> <message>    Send message to specific peer",
        "send".cyan()
    );
    println!(
        "  {} <message>              Broadcast message to all peers",
        "broadcast".cyan()
    );
    println!(
        "  {} <peer_id> [timeout_ms] Ping a peer and print RTT",
        "ping".cyan()
    );
    println!("  {}                        List all known peers", "peers".cyan());
    println!("  {}                        Show node status", "status".cyan());
}

fn get_api_port() -> u16 {
    if let Ok(port) = std::env::var("MESHLINK_API_PORT") {
        if let Ok(p) = port.parse::<u16>() {
            return p;
        }
    }
    // Default scheme: 9000 + P2P port (e.g. 8080 -> 17080)
    // Try the common local range first.
    for port in 17070..=17100 {
        match TcpStream::connect(format!("127.0.0.1:{}", port)) {
            Ok(_) => {
                eprintln!(
                    "{} Connected to API on port {}",
                    "✓".green(),
                    port.to_string().cyan()
                );
                return port;
            }
            Err(_) => continue,
        }
    }
    eprintln!(
        "{}",
        "✗ Error: Could not find MeshLink API server".red().bold()
    );
    eprintln!("  Make sure a node is running and try:");
    eprintln!(
        "  {} {}",
        "-".dimmed(),
        "MESHLINK_API_PORT=17080 cargo run --bin ely -- status".yellow()
    );
    std::process::exit(1);
}

fn send_message(peer_id: Option<String>, message: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = if let Some(pid) = peer_id {
        serde_json::json!({
            "command": "send",
            "peer_id": pid,
            "message": message
        })
    } else {
        serde_json::json!({
            "command": "broadcast",
            "message": message
        })
    };

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(msg_id) = resp["data"]["message_id"].as_str() {
            println!("{} Message sent! ID: {}", "✓".green().bold(), msg_id.cyan());
        } else {
            println!("{} Message sent!", "✓".green().bold());
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn ping(peer_id: String, timeout_ms: u64) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = serde_json::json!({
        "command": "ping",
        "peer_id": peer_id,
        "timeout_ms": timeout_ms
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        let latency_ms = resp["data"]["latency_ms"].as_f64().unwrap_or(-1.0);
        if latency_ms >= 0.0 {
            println!(
                "{} RTT to peer: {} ms",
                "✓".green().bold(),
                format!("{:.2}", latency_ms).cyan()
            );
        } else {
            println!("{}", "✓ Ping OK".green().bold());
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn list_peers() -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = serde_json::json!({
        "command": "peers"
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(peers) = resp["data"]["peers"].as_array() {
            if peers.is_empty() {
                println!("{}", "No peers found".yellow());
            } else {
                println!(
                    "{}",
                    format!("Connected Peers ({})", peers.len())
                        .bright_cyan()
                        .bold()
                );
                println!("{}", "─".repeat(60).dimmed());
                for peer in peers {
                    let id = peer["id"].as_str().unwrap_or("?").cyan();
                    let addr = peer["address"].as_str().unwrap_or("?").green();
                    let state_str = peer["state"].as_str().unwrap_or("?");
                    let state = if state_str.contains("Connected") {
                        state_str.green()
                    } else if state_str.contains("Connecting") {
                        state_str.yellow()
                    } else {
                        state_str.red()
                    };
                    println!("  {} @ {} [{}]", id, addr, state);
                }
            }
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn show_status() -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = serde_json::json!({
        "command": "status"
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(data) = resp["data"].as_object() {
            println!(
                "{}",
                "╭─ MeshLink Node Status ────────────────────────────────────────╮".bright_cyan()
            );
            if let Some(node_id) = data["node_id"].as_str() {
                println!(
                    "{} {}",
                    "│".bright_cyan(),
                    format!("Node ID:   {}", node_id.cyan()).bright_white()
                );
            }
            if let Some(connected) = data["connected_peers"].as_u64() {
                let total = data["total_peers"].as_u64().unwrap_or(0);
                println!(
                    "{} {}",
                    "│".bright_cyan(),
                    format!(
                        "Connected: {}/{} peers",
                        connected.to_string().green(),
                        total.to_string().dimmed()
                    )
                    .bright_white()
                );
            }
            println!(
                "{}",
                "╰───────────────────────────────────────────────────────────────╯".bright_cyan()
            );
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}


