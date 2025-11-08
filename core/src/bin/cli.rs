/// CLI for MeshLink node management
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let command = &args[1];

    match command.as_str() {
        "send" => {
            if args.len() < 4 {
                eprintln!("Usage: meshlink send <peer_id> <message>");
                return Ok(());
            }
            let peer_id = Some(args[2].clone());
            let message = args[3..].join(" ");
            send_message(peer_id, message)?;
        }
        "broadcast" => {
            if args.len() < 3 {
                eprintln!("Usage: meshlink broadcast <message>");
                return Ok(());
            }
            let message = args[2..].join(" ");
            send_message(None, message)?;
        }
        "peers" => {
            list_peers()?;
        }
        "status" => {
            show_status()?;
        }
        _ => {
            eprintln!("Unknown command: {}", command);
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("MeshLink CLI");
    println!();
    println!("Usage: meshlink <command> [args]");
    println!();
    println!("Commands:");
    println!("  send <peer_id> <message>    Send message to specific peer");
    println!("  broadcast <message>         Broadcast message to all peers");
    println!("  peers                       List all known peers");
    println!("  status                      Show node status");
}

fn get_api_port() -> u16 {
    // Try to detect API port from environment or use default
    // API port is 9000 + node_port, so if node is on 8080, API is on 9080
    // For CLI, we try common ports or use env var
    if let Ok(port) = std::env::var("MESHLINK_API_PORT") {
        if let Ok(p) = port.parse::<u16>() {
            return p;
        }
    }
    // Try common ports: 9082, 9083, 9080, 9081, 9000
    // Note: 8080/8081 may be occupied by nginx, so try 8082/8083 first
    // API port = 9000 + node_port
    for port in [9082, 9083, 9080, 9081, 9000] {
        match TcpStream::connect(format!("127.0.0.1:{}", port)) {
            Ok(_) => {
                eprintln!("✓ Connected to API on port {}", port);
                return port;
            }
            Err(_) => continue,
        }
    }
    // If no port found, show helpful error
    eprintln!("✗ Error: Could not find MeshLink API server");
    eprintln!("  Make sure a node is running and try:");
    eprintln!("  - MESHLINK_API_PORT=9082 cargo run --bin cli -- status");
    eprintln!("  - MESHLINK_API_PORT=9083 cargo run --bin cli -- status");
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

    writeln!(stream, "{}", request.to_string())?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(msg_id) = resp["data"]["message_id"].as_str() {
            println!("✓ Message sent! ID: {}", msg_id);
        } else {
            println!("✓ Message sent!");
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("✗ Error: {}", error);
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

    writeln!(stream, "{}", request.to_string())?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(peers) = resp["data"]["peers"].as_array() {
            if peers.is_empty() {
                println!("No peers found");
            } else {
                println!("Peers ({}):", peers.len());
                println!("{:-<60}", "");
                for peer in peers {
                    let id = peer["id"].as_str().unwrap_or("?");
                    let addr = peer["address"].as_str().unwrap_or("?");
                    let state = peer["state"].as_str().unwrap_or("?");
                    println!("  {} @ {} [{}]", id, addr, state);
                }
            }
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("✗ Error: {}", error);
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

    writeln!(stream, "{}", request.to_string())?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(data) = resp["data"].as_object() {
            println!("Node Status:");
            println!("{:-<60}", "");
            if let Some(node_id) = data["node_id"].as_str() {
                println!("  Node ID: {}", node_id);
            }
            if let Some(connected) = data["connected_peers"].as_u64() {
                println!("  Connected: {}", connected);
            }
            if let Some(total) = data["total_peers"].as_u64() {
                println!("  Total peers: {}", total);
            }
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("✗ Error: {}", error);
        std::process::exit(1);
    }

    Ok(())
}
