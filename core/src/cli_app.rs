use colored::*;
use crate::{Config, Node};
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

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
        "start" => {
            // `ely start <p2p_port> [peer1] [peer2] ... [-d|--daemon]`
            // We reuse `Config::from_args` by shifting args left (so port becomes args[1]).
            if args.len() < 3 {
                eprintln!(
                    "{}",
                    format!("Usage: {} start <p2p_port> [peer1] [peer2] ... [-d|--daemon]", bin).yellow()
                );
                return Ok(());
            }
            let daemon = args.iter().any(|a| a == "-d" || a == "--daemon");
            start_node(&args[2..], daemon)?;
        }
        "chat" => {
            if args.len() < 3 {
                eprintln!("{}", format!("Usage: {} chat <peer_id|broadcast>", bin).yellow());
                return Ok(());
            }
            let target = args[2].clone();
            chat(target)?;
        }
        "send" => {
            if args.len() < 4 {
                eprintln!(
                    "{}",
                    format!("Usage: {} send <peer_id> <message>", bin).yellow()
                );
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
                eprintln!(
                    "{}",
                    format!("Usage: {} ping <peer_id> [timeout_ms]", bin).yellow()
                );
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
        "inbox" => {
            let n = args
                .get(2)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20);
            inbox(n)?;
        }
        "watch" => {
            watch()?;
        }
        "publish" => {
            if args.len() < 4 {
                eprintln!(
                    "{}",
                    format!("Usage: {} publish <path> <content|@file>", bin).yellow()
                );
                return Ok(());
            }
            let path = args[2].clone();
            let content_arg = args[3..].join(" ");
            publish(path, content_arg)?;
        }
        "fetch" => {
            if args.len() < 3 {
                eprintln!("{}", format!("Usage: {} fetch <ely://node_id/path>", bin).yellow());
                return Ok(());
            }
            let url = args[2].clone();
            fetch(url)?;
        }
        "name" => {
            if args.len() < 3 {
                eprintln!("{}", format!("Usage: {} name <register|resolve> [args]", bin).yellow());
                return Ok(());
            }
            let subcommand = &args[2];
            match subcommand.as_str() {
                "register" => {
                    if args.len() < 5 {
                        eprintln!(
                            "{}",
                            format!("Usage: {} name register <name> <node_id>", bin).yellow()
                        );
                        return Ok(());
                    }
                    let name = args[3].clone();
                    let node_id = args[4].clone();
                    name_register(name, node_id)?;
                }
                "resolve" => {
                    if args.len() < 4 {
                        eprintln!(
                            "{}",
                            format!("Usage: {} name resolve <name>", bin).yellow()
                        );
                        return Ok(());
                    }
                    let name = args[3].clone();
                    name_resolve(name)?;
                }
                _ => {
                    eprintln!("{} Unknown name subcommand: {}", "✗".red().bold(), subcommand.red());
                    eprintln!("  Available: register, resolve");
                }
            }
        }
        "bundle" => {
            if args.len() < 3 {
                eprintln!("{}", format!("Usage: {} bundle <export|import|info> [args]", bin).yellow());
                return Ok(());
            }
            let subcommand = &args[2];
            match subcommand.as_str() {
                "export" => {
                    if args.len() < 4 {
                        eprintln!(
                            "{}",
                            format!("Usage: {} bundle export <output_file>", bin).yellow()
                        );
                        return Ok(());
                    }
                    let output_file = args[3].clone();
                    bundle_export(output_file)?;
                }
                "import" => {
                    if args.len() < 4 {
                        eprintln!(
                            "{}",
                            format!("Usage: {} bundle import <input_file>", bin).yellow()
                        );
                        return Ok(());
                    }
                    let input_file = args[3].clone();
                    bundle_import(input_file)?;
                }
                "info" => {
                    if args.len() < 4 {
                        eprintln!(
                            "{}",
                            format!("Usage: {} bundle info <bundle_file>", bin).yellow()
                        );
                        return Ok(());
                    }
                    let bundle_file = args[3].clone();
                    bundle_info(bundle_file)?;
                }
                _ => {
                    eprintln!("{} Unknown bundle subcommand: {}", "✗".red().bold(), subcommand.red());
                    eprintln!("  Available: export, import, info");
                }
            }
        }
        "install-handler" => {
            crate::url_handler::install_url_handler()?;
        }
        "handle-url" => {
            if args.len() < 3 {
                eprintln!("{}", format!("Usage: {} handle-url <ely://...>", bin).yellow());
                return Ok(());
            }
            let url = args[2].clone();
            crate::url_handler::handle_url(url)?;
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
        "  {} <p2p_port> [peers...] [-d]  Start a node (P2P + discovery + local API)",
        "start".cyan()
    );
    println!("                                {} Use -d to run in background (daemon mode)", "→".dimmed());
    println!(
        "  {} <peer_id|broadcast>     Interactive chat (Ctrl+C to exit)",
        "chat".cyan()
    );
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
    println!(
        "  {}                        List all known peers",
        "peers".cyan()
    );
    println!(
        "  {}                        Show node status",
        "status".cyan()
    );
    println!(
        "  {} [n]                    Show last N messages from inbox (default 20)",
        "inbox".cyan()
    );
    println!(
        "  {}                        Live stream messages (Ctrl+C to exit)",
        "watch".cyan()
    );
    println!(
        "  {} <path> <content>         Publish content to mesh (use @file to read from file)",
        "publish".cyan()
    );
    println!(
        "  {} <url>                     Fetch content from mesh (ely://node_id/path)",
        "fetch".cyan()
    );
    println!(
        "  {}                            Install OS-level ely:// URL handler",
        "install-handler".cyan()
    );
    println!(
        "  {} <ely://url>               Open ely:// URL in browser via Web Gateway",
        "handle-url".cyan()
    );
    println!(
        "  {} register <name> <id>     Register a human-readable name",
        "name".cyan()
    );
    println!(
        "  {} resolve <name>           Resolve name to node_id",
        "name".cyan()
    );
    println!(
        "  {} export <file>            Export messages to bundle file",
        "bundle".cyan()
    );
    println!(
        "  {} import <file>            Import bundle from file",
        "bundle".cyan()
    );
    println!(
        "  {} info <file>              Show bundle info",
        "bundle".cyan()
    );
}

fn get_api_port() -> u16 {
    // Priority 1: Environment variable
    if let Ok(port) = std::env::var("MESHLINK_API_PORT") {
        if let Ok(p) = port.parse::<u16>() {
            return p;
        }
    }

    // Priority 2: Last used port file (from running node)
    if let Some(home) = std::env::var("HOME").ok() {
        let port_file = std::path::Path::new(&home).join(".elysium_api_port");
        if let Ok(content) = std::fs::read_to_string(&port_file) {
            if let Ok(port) = content.trim().parse::<u16>() {
                // Verify it's actually running
                if TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
                    eprintln!(
                        "{} Connected to API on port {} {}",
                        "✓".green(),
                        port.to_string().cyan(),
                        "(from ~/.elysium_api_port)".dimmed()
                    );
                    return port;
                }
            }
        }
    }

    // Priority 3: Default port (most common: 8080 → 17080)
    let default_port = 17080;
    if TcpStream::connect(format!("127.0.0.1:{}", default_port)).is_ok() {
        eprintln!(
            "{} Connected to API on port {} {}",
            "✓".green(),
            default_port.to_string().cyan(),
            "(default)".dimmed()
        );
        return default_port;
    }

    // Priority 4: Scan common ports (8080-8089 → 17080-17089)
    for port in 17080..=17089 {
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

    // Not found - show helpful message
    eprintln!(
        "{}",
        "✗ Error: Could not find Elysium API server".red().bold()
    );
    eprintln!("  Make sure a node is running:");
    eprintln!(
        "  {} {}",
        "→".dimmed(),
        "ely start 8080".yellow()
    );
    eprintln!();
    eprintln!("  Or specify the API port explicitly:");
    eprintln!(
        "  {} {}",
        "→".dimmed(),
        "MESHLINK_API_PORT=17080 ely status".yellow()
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

fn inbox(limit: usize) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = serde_json::json!({
        "command": "inbox",
        "since": 0,
        "limit": limit
    });

    writeln!(stream, "{}", request)?;
    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;
    if !resp["success"].as_bool().unwrap_or(false) {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        anyhow::bail!("API error: {}", error);
    }

    let messages = resp["data"]["messages"].as_array().cloned().unwrap_or_default();
    if messages.is_empty() {
        println!("{}", "Inbox is empty".dimmed());
        return Ok(());
    }

    for m in messages {
        print_inbox_message(&m);
        println!();
    }
    Ok(())
}

fn watch() -> anyhow::Result<()> {
    let mut since: u64 = 0;
    loop {
        let api_port = get_api_port();
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
        stream.set_read_timeout(Some(Duration::from_secs(35)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let request = serde_json::json!({
            "command": "watch",
            "since": since,
            "timeout_ms": 20000,
            "limit": 50
        });

        writeln!(stream, "{}", request)?;

        let mut response = String::new();
        use std::io::BufRead;
        std::io::BufReader::new(&stream).read_line(&mut response)?;

        let resp: serde_json::Value = serde_json::from_str(&response)?;
        if !resp["success"].as_bool().unwrap_or(false) {
            let error = resp["error"].as_str().unwrap_or("Unknown error");
            eprintln!("{} {}", "✗".red().bold(), error.red());
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }

        if let Some(next) = resp["data"]["next_since"].as_u64() {
            since = next;
        }
        if let Some(messages) = resp["data"]["messages"].as_array() {
            for m in messages {
                print_inbox_message(m);
            }
        }
    }
}

fn print_inbox_message(m: &serde_json::Value) {
    let ts = m["timestamp"].as_str().unwrap_or("");
    let direction = m["direction"].as_str().unwrap_or("?");
    let kind = m["kind"].as_str().unwrap_or("?");
    let from = m["from"].as_str().unwrap_or("?");
    let to = m["to"].as_str().unwrap_or("broadcast");
    let preview = m["preview"].as_str().unwrap_or("");
    let msg_id = m["message_id"]
        .as_str()
        .map(|s| s.chars().take(8).collect::<String>())
        .unwrap_or_default();

    let arrow = if direction == "out" { "→" } else { "←" };
    let header = format!(
        "{} {} {} {} {} {} {}",
        ts.dimmed(),
        kind.yellow(),
        arrow,
        from.bright_white(),
        "→".dimmed(),
        to.bright_white(),
        if msg_id.is_empty() {
            "".to_string()
        } else {
            format!("({})", msg_id).dimmed().to_string()
        }
    );
    println!("{}", header);
    println!("  {}", preview);
}

fn chat(target: String) -> anyhow::Result<()> {
    println!(
        "{} {}",
        "Chat target:".bright_white().bold(),
        target.cyan()
    );
    println!("{}", "Type messages and press Enter. Ctrl+C to exit.".dimmed());
    println!();

    // Background thread: stream inbox and print as it arrives.
    std::thread::spawn(move || {
        // Best-effort: filter locally by printing everything (simple MVP).
        // Users can run `ely chat <peer>` on the right node if they want a 1:1 experience.
        let _ = watch();
    });

    // Foreground: read stdin and send
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap_or_default();
        let msg = line.trim();
        if msg.is_empty() {
            continue;
        }
        if target == "broadcast" {
            let _ = send_message(None, msg.to_string());
        } else {
            let _ = send_message(Some(normalize_peer_id(&target).to_string()), msg.to_string());
        }
    }

    Ok(())
}

fn start_node(start_args: &[String], daemon: bool) -> anyhow::Result<()> {
    // start_args: ["<port>", ...]
    // Filter out daemon flags from args before parsing config
    let mut config_args: Vec<String> = start_args
        .iter()
        .filter(|a| *a != "-d" && *a != "--daemon")
        .cloned()
        .collect();
    
    config_args.insert(0, "core".to_string());

    let config = Config::from_args(&config_args)?;
    
    if daemon {
        // Run in background: spawn new process and detach
        let mut cmd = std::process::Command::new(std::env::current_exe()?);
        
        // Rebuild args: ["start", "<port>", ...] (without daemon flag and without "core"/"ely")
        let mut new_args: Vec<String> = vec!["start".to_string()];
        new_args.extend(config_args.iter().skip(1).cloned()); // Skip "core", keep rest (port, peers, etc)
        cmd.args(&new_args);
        
        // Redirect stdout/stderr to log file
        let log_dir = config.data_dir.as_ref()
            .map(|d| d.clone())
            .unwrap_or_else(|| std::path::PathBuf::from(".ely"));
        std::fs::create_dir_all(&log_dir)?;
        let log_file = log_dir.join(format!("node-{}.log", config.listen_addr.port()));
        
        let file = std::fs::File::create(&log_file)?;
        cmd.stdout(file.try_clone()?);
        cmd.stderr(file);
        
        // Note: Process will continue running after parent exits on Unix
        // We don't wait for it, so it becomes detached automatically
        
        // Spawn process in background
        let mut child = cmd.spawn()?;
        let pid = child.id();
        
        // Give it a moment to start and initialize
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        // Check if process is still running
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited immediately - read log file to show error
                let error_msg = if let Ok(log_content) = std::fs::read_to_string(&log_file) {
                    format!("Node exited. Last log entries:\n{}", 
                        log_content.lines().rev().take(10).collect::<Vec<_>>().join("\n"))
                } else {
                    format!("Node exited with status: {}", status)
                };
                return Err(anyhow::anyhow!("Failed to start node: {}", error_msg));
            }
            Ok(None) => {
                // Process is still running - success!
            }
            Err(e) => {
                // Can't check status - assume it's running
                tracing::debug!("Could not check child process status: {} (assuming running)", e);
            }
        }
        
        // Don't wait for child - let it run in background
        // The process will continue even after this function returns
        
        println!(
            "{} Node started in background (PID: {})",
            "✓".green().bold(),
            pid
        );
        println!(
            "{} Logs: {}",
            "→".cyan(),
            log_file.display()
        );
        println!(
            "{} Stop with: kill {}",
            "→".cyan(),
            pid
        );
        println!(
            "{} Wait a moment for node to initialize before running commands",
            "→".dimmed()
        );
        
        Ok(())
    } else {
        // Run in foreground (original behavior)
        // Initialize tracing (only for the long-running start command)
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
            )
            .try_init();

        let node = Node::new(config)?;

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async move { node.start().await })?;
        Ok(())
    }
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

fn publish(path: String, content_arg: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    // Read content from file if starts with @
    let content = if content_arg.starts_with('@') {
        let file_path = &content_arg[1..];
        std::fs::read(file_path)?
    } else {
        content_arg.into_bytes()
    };

    println!(
        "{} Publishing {} bytes to path: {}",
        "⤴".cyan().bold(),
        content.len(),
        path.yellow()
    );

    let request = serde_json::json!({
        "command": "publish",
        "path": path,
        "content": content
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(data) = resp["data"].as_object() {
            if let Some(url) = data["url"].as_str() {
                println!("{} Content published at: {}", "✓".green().bold(), url.green());
            }
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn fetch(url: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    println!("{} Fetching: {}", "⤵".cyan().bold(), url.yellow());

    let request = serde_json::json!({
        "command": "fetch",
        "url": url,
        "timeout_ms": 5000
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(data) = resp["data"].as_object() {
            if let Some(content) = data["content"].as_str() {
                if let Some(size) = data["size_bytes"].as_u64() {
                    println!(
                        "{} Content retrieved ({} bytes):",
                        "✓".green().bold(),
                        size
                    );
                    println!("{}", content);
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

fn name_register(name: String, node_id: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    println!(
        "{} Registering name: {} → {}",
        "⚡".cyan().bold(),
        name.yellow(),
        node_id.green()
    );

    let request = serde_json::json!({
        "command": "name_register",
        "name": name,
        "node_id": node_id
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        println!("{} Name registered successfully", "✓".green().bold());
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn name_resolve(name: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    println!("{} Resolving name: {}", "⚡".cyan().bold(), name.yellow());

    let request = serde_json::json!({
        "command": "name_resolve",
        "name": name
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(node_id) = resp["data"]["node_id"].as_str() {
            println!("{} {} → {}", "✓".green().bold(), name.yellow(), node_id.green());
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn bundle_export(output_file: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    println!("{} Exporting bundle to: {}", "📦".cyan().bold(), output_file.yellow());

    let request = serde_json::json!({
        "command": "bundle_export",
        "output_path": output_file
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(count) = resp["data"]["message_count"].as_u64() {
            println!("{} Bundle exported: {} messages", "✓".green().bold(), count);
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn bundle_import(input_file: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    println!("{} Importing bundle from: {}", "📦".cyan().bold(), input_file.yellow());

    let request = serde_json::json!({
        "command": "bundle_import",
        "input_path": input_file
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(data) = resp["data"].as_object() {
            let delivered = data["delivered"].as_u64().unwrap_or(0);
            let forwarded = data["forwarded"].as_u64().unwrap_or(0);
            println!(
                "{} Bundle imported: {} delivered, {} forwarded",
                "✓".green().bold(),
                delivered,
                forwarded
            );
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}

fn bundle_info(bundle_file: String) -> anyhow::Result<()> {
    let api_port = get_api_port();
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", api_port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;

    println!("{} Bundle info: {}", "📦".cyan().bold(), bundle_file.yellow());

    let request = serde_json::json!({
        "command": "bundle_info",
        "bundle_path": bundle_file
    });

    writeln!(stream, "{}", request)?;

    let mut response = String::new();
    use std::io::BufRead;
    std::io::BufReader::new(&stream).read_line(&mut response)?;

    let resp: serde_json::Value = serde_json::from_str(&response)?;

    if resp["success"].as_bool().unwrap_or(false) {
        if let Some(data) = resp["data"].as_object() {
            let count = data["message_count"].as_u64().unwrap_or(0);
            let created = data["created_at"].as_str().unwrap_or("?");
            let expires = data["expires_at"].as_str().unwrap_or("?");
            let expired = data["expired"].as_bool().unwrap_or(false);

            println!("{} Bundle Info:", "ℹ".cyan().bold());
            println!("  Messages: {}", count);
            println!("  Created:  {}", created);
            println!("  Expires:  {}", expires);
            println!("  Expired:  {}", if expired { "YES".red() } else { "NO".green() });
        }
    } else {
        let error = resp["error"].as_str().unwrap_or("Unknown error");
        eprintln!("{} Error: {}", "✗".red().bold(), error.red());
        std::process::exit(1);
    }

    Ok(())
}
