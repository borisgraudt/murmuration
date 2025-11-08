use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

#[derive(Debug, Clone)]
struct Node {
    name: String,
    x: usize,
    y: usize,
}

#[derive(Debug, Clone)]
struct Link {
    a: usize, // index in nodes
    b: usize,
    pulse: u8, // 0 = no pulse, >0 = frames left to pulse
    message_wave: Option<u8>, // Wave animation for message transmission (0-20)
}

#[derive(Debug, Clone)]
struct Graph {
    nodes: Vec<Node>,
    links: Vec<Link>,
    last_event: Option<String>,
    name_to_idx: std::collections::BTreeMap<String, usize>,
    pulse_nodes: std::collections::BTreeMap<usize, u8>, // node idx -> frames left to pulse
    animation_frame: u64,
    rng: StdRng,
    grid_w: usize,
    grid_h: usize,
    node_last_seen: std::collections::BTreeMap<String, std::time::Instant>, // Track when nodes were last seen
}

impl Graph {
    fn new() -> Self {
        Self {
            nodes: vec![],
            links: vec![],
            last_event: None,
            name_to_idx: std::collections::BTreeMap::new(),
            pulse_nodes: std::collections::BTreeMap::new(),
            animation_frame: 0,
            rng: StdRng::seed_from_u64(42),
            grid_w: 24,
            grid_h: 12,
            node_last_seen: std::collections::BTreeMap::new(),
        }
    }
    
    fn cleanup_stale_nodes(&mut self, timeout: std::time::Duration) {
        let now = std::time::Instant::now();
        let stale_nodes: Vec<String> = self.node_last_seen
            .iter()
            .filter(|(_, &last_seen)| now.duration_since(last_seen) > timeout)
            .map(|(name, _)| name.clone())
            .collect();
        
        for name in stale_nodes {
            if let Some(&idx) = self.name_to_idx.get(&name) {
                // Remove node
                self.nodes.remove(idx);
                // Remove links
                self.links.retain(|link| link.a != idx && link.b != idx);
                // Update indices
                self.name_to_idx.remove(&name);
                // Rebuild name_to_idx with correct indices
                self.name_to_idx.clear();
                for (i, node) in self.nodes.iter().enumerate() {
                    self.name_to_idx.insert(node.name.clone(), i);
                }
                // Update link indices
                for link in &mut self.links {
                    if link.a > idx {
                        link.a -= 1;
                    }
                    if link.b > idx {
                        link.b -= 1;
                    }
                }
            }
            self.node_last_seen.remove(&name);
        }
    }

    fn find_or_add_node(&mut self, name: &str) -> usize {
        if let Some(&idx) = self.name_to_idx.get(name) {
            idx
        } else {
            // Place randomly in grid
            let mut tries = 0;
            let (x, y) = loop {
                let x = self.rng.gen_range(1..self.grid_w-1);
                let y = self.rng.gen_range(1..self.grid_h-1);
                let occupied = self.nodes.iter().any(|n| n.x == x && n.y == y);
                if !occupied || tries > 10 {
                    break (x, y);
                }
                tries += 1;
            };
            let idx = self.nodes.len();
            self.nodes.push(Node { name: name.to_string(), x, y });
            self.name_to_idx.insert(name.to_string(), idx);
            idx
        }
    }

    fn find_link(&self, a: usize, b: usize) -> Option<usize> {
        self.links.iter().position(|l| (l.a == a && l.b == b) || (l.a == b && l.b == a))
    }

    fn add_link(&mut self, a: usize, b: usize) -> usize {
        if let Some(idx) = self.find_link(a, b) {
            idx
        } else {
            let idx = self.links.len();
            self.links.push(Link { a, b, pulse: 0, message_wave: None });
            idx
        }
    }

    fn apply(&mut self, msg: &str) {
        self.last_event = Some(msg.to_string());
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(msg) {
            let node = v.get("node").and_then(|x| x.as_str()).unwrap_or("unknown");
            let event = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
            let peer = v.get("peer").and_then(|x| x.as_str()).unwrap_or("");

            let node_idx = self.find_or_add_node(node);
            // Update last seen for this node
            self.node_last_seen.insert(node.to_string(), std::time::Instant::now());
            // Pulse this node
            self.pulse_nodes.insert(node_idx, 8);

            if !peer.is_empty() {
                let peer_idx = self.find_or_add_node(peer);
                // Update last seen for peer
                self.node_last_seen.insert(peer.to_string(), std::time::Instant::now());
                if event == "connected" {
                    let link_idx = self.add_link(node_idx, peer_idx);
                    self.links[link_idx].pulse = 10; // Longer pulse for connections
                } else if event == "disconnected" {
                    // Remove link when disconnected
                    if let Some(link_idx) = self.find_link(node_idx, peer_idx) {
                        self.links.remove(link_idx);
                    }
                } else if event == "message_sent" || event == "message_received" || event == "mesh_message_received" {
                    // Pulse link when message is sent/received
                    if let Some(link_idx) = self.find_link(node_idx, peer_idx) {
                        self.links[link_idx].pulse = 15; // Strong pulse for messages
                        self.links[link_idx].message_wave = Some(0); // Start wave animation
                    }
                    // Also pulse both nodes
                    self.pulse_nodes.insert(node_idx, 12);
                    self.pulse_nodes.insert(peer_idx, 12);
                }
            } else if event == "message_sent" || event == "message_received" || event == "mesh_message_received" {
                // Message event without peer (broadcast) - just pulse the node
                self.pulse_nodes.insert(node_idx, 10);
            } else if event == "heartbeat" {
                // Heartbeat events - just update last seen, no pulse
                // This prevents too many visual updates
            }
        }
    }

    fn tick(&mut self) {
        // Decrement pulse counters
        self.animation_frame = self.animation_frame.wrapping_add(1);
        self.pulse_nodes.retain(|_, v| {
            if *v > 0 {
                *v -= 1;
                *v > 0
            } else {
                false
            }
        });
        for l in self.links.iter_mut() {
            if l.pulse > 0 {
                l.pulse -= 1;
            }
            // Update message wave animation
            if let Some(wave) = &mut l.message_wave {
                *wave += 1;
                if *wave > 20 {
                    l.message_wave = None; // End wave animation
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // UDP listener task
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let sock = UdpSocket::bind("127.0.0.1:9999").await.expect("bind 9999");
        let mut buf = vec![0u8; 2048];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((n, _)) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        let _ = tx.send(s.to_string());
                    }
                }
                Err(_) => {}
            }
        }
    });

    // TUI setup
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let res = run_app(&mut terminal, &mut rx);
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("TUI error: {e}");
    }
    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    rx: &mut mpsc::UnboundedReceiver<String>,
) -> std::io::Result<()> {
    let mut g = Graph::new();
    let mut last_redraw = Instant::now();
    let mut last_cleanup = Instant::now();

    loop {
        // Drain incoming messages
        while let Ok(msg) = rx.try_recv() {
            g.apply(&msg);
        }
        
        // Cleanup stale nodes every 10 seconds
        if last_cleanup.elapsed() >= Duration::from_secs(10) {
            g.cleanup_stale_nodes(Duration::from_secs(30));
            last_cleanup = Instant::now();
        }

        // Handle keys
        if crossterm::event::poll(Duration::from_millis(5))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    _ => {}
                }
            }
        }

        // Redraw ~60 FPS cap
        if last_redraw.elapsed() >= Duration::from_millis(16) {
            terminal.draw(|f| {
                use ratatui::text::Span;
                use ratatui::style::{Color, Style};
                let grid_w = g.grid_w;
                let grid_h = g.grid_h;
                let mut grid: Vec<Vec<(char, Option<Style>)>> = vec![vec![(' ', None); grid_w]; grid_h];

                // Draw links with beautiful styling
                for link in &g.links {
                    let a = &g.nodes[link.a];
                    let b = &g.nodes[link.b];
                    let mut x0 = a.x as isize;
                    let mut y0 = a.y as isize;
                    let x1 = b.x as isize;
                    let y1 = b.y as isize;
                    let dx = (x1 - x0).abs();
                    let dy = (y1 - y0).abs();
                    let sx = if x0 < x1 { 1 } else { -1 };
                    let sy = if y0 < y1 { 1 } else { -1 };
                    let mut err = dx - dy;
                    let mut first = true;
                    let total_points = (dx + dy) as usize;
                    let mut point_idx = 0;
                    
                    // Calculate line direction for proper character selection
                    let is_vertical = dx == 0;
                    let is_horizontal = dy == 0;
                    
                    loop {
                        if !first && (x0 >= 0 && x0 < grid_w as isize && y0 >= 0 && y0 < grid_h as isize) {
                            // Choose character based on line direction - clean, straight lines
                            let ch = if is_vertical {
                                '│'  // Perfect vertical
                            } else if is_horizontal {
                                '─'  // Perfect horizontal
                            } else {
                                // Diagonal - use proper diagonal characters
                                if sx == sy {
                                    '╲'  // Top-left to bottom-right
                                } else {
                                    '╱'  // Top-right to bottom-left
                                }
                            };
                            
                            // Calculate position along the line (0.0 to 1.0)
                            let progress = if total_points > 0 {
                                point_idx as f32 / total_points as f32
                            } else {
                                0.0
                            };
                            
                            // Beautiful animation effects
                            let (ch, style) = if let Some(wave_pos) = link.message_wave {
                                // Message wave animation - flowing light effect
                                let wave_progress = wave_pos as f32 / 20.0; // 0.0 to 1.0
                                let distance_from_wave = (progress - wave_progress).abs();
                                
                                // Create glowing wave effect
                                let intensity = if distance_from_wave < 0.15 {
                                    // Near the wave - bright glow
                                    let fade = 1.0 - (distance_from_wave / 0.15);
                                    fade
                                } else {
                                    0.0
                                };
                                
                                // Color gradient for wave
                                let base_color = if intensity > 0.7 {
                                    Color::Cyan
                                } else if intensity > 0.4 {
                                    Color::LightCyan
                                } else {
                                    Color::Blue
                                };
                                
                                // Use bright, thick characters for wave
                                let wave_ch = match ch {
                                    '│' => '┃',
                                    '─' => '━',
                                    '╲' => '╲',
                                    '╱' => '╱',
                                    _ => ch,
                                };
                                
                                (wave_ch, Some(Style::default()
                                    .fg(base_color)
                                    .bg(Color::Black)
                                    .add_modifier(ratatui::style::Modifier::BOLD)))
                            } else if link.pulse > 0 {
                                // Pulsing link - subtle glow
                                let pulse_intensity = link.pulse as f32 / 15.0;
                                let pulse_phase = (g.animation_frame as f32 * 0.1 + progress * 2.0) % (std::f32::consts::PI * 2.0);
                                let brightness = (pulse_phase.sin() * 0.3 + 0.7) * pulse_intensity;
                                
                                let color = if brightness > 0.8 {
                                    Color::LightCyan
                                } else if brightness > 0.5 {
                                    Color::Cyan
                                } else {
                                    Color::Blue
                                };
                                
                                let active_ch = match ch {
                                    '│' => '┃',
                                    '─' => '━',
                                    '╲' => '╲',
                                    '╱' => '╱',
                                    _ => ch,
                                };
                                
                                (active_ch, Some(Style::default()
                                    .fg(color)
                                    .bg(Color::Black)
                                    .add_modifier(ratatui::style::Modifier::BOLD)))
                            } else {
                                // Inactive link - clean, subtle gray
                                (ch, Some(Style::default()
                                    .fg(Color::DarkGray)
                                    .bg(Color::Black)))
                            };
                            
                            // Only overwrite if cell is empty or has higher priority (wave > pulse > normal)
                            let current = &grid[y0 as usize][x0 as usize];
                            let priority = if link.message_wave.is_some() { 3 }
                                          else if link.pulse > 0 { 2 }
                                          else { 1 };
                            let current_priority = if current.0 == ' ' { 0 } else { 1 };
                            
                            if priority >= current_priority {
                                grid[y0 as usize][x0 as usize] = (ch, style);
                            }
                        }
                        if x0 == x1 && y0 == y1 { break; }
                        let e2 = 2*err;
                        if e2 > -dy { err -= dy; x0 += sx; }
                        if e2 < dx { err += dx; y0 += sy; }
                        first = false;
                        point_idx += 1;
                    }
                }

                // Draw nodes with beautiful styling and smooth animations
                for (idx, node) in g.nodes.iter().enumerate() {
                    let pulse = g.pulse_nodes.get(&idx).copied().unwrap_or(0);
                    let (ch, style) = if pulse > 0 {
                        // Pulsing node - create smooth glow effect
                        let pulse_progress = pulse as f32 / 12.0; // Normalize to 0-1
                        let animation_phase = (g.animation_frame as f32 * 0.15) % (std::f32::consts::PI * 2.0);
                        let glow_intensity = (animation_phase.sin() * 0.4 + 0.6) * pulse_progress;
                        
                        // Smooth color transition
                        let color = if glow_intensity > 0.8 {
                            Color::Yellow
                        } else if glow_intensity > 0.6 {
                            Color::LightYellow
                        } else if glow_intensity > 0.4 {
                            Color::Cyan
                        } else {
                            Color::LightCyan
                        };
                        
                        // Use glowing character
                        ('◉', Some(Style::default()
                            .fg(color)
                            .bg(Color::Black)
                            .add_modifier(ratatui::style::Modifier::BOLD)))
                    } else {
                        // Normal node - clean white dot
                        ('●', Some(Style::default()
                            .fg(Color::White)
                            .bg(Color::Black)))
                    };
                    grid[node.y][node.x] = (ch, style);
                }

                // Compose grid into lines
                let mut lines = Vec::new();
                for row in &grid {
                    let mut line = Vec::new();
                    for &(ch, style) in row {
                        line.push(Span::styled(
                            ch.to_string(),
                            style.unwrap_or_else(|| Style::default())
                        ));
                    }
                    lines.push(ratatui::text::Line::from(line));
                }
                // Overlay node names
                for node in &g.nodes {
                    // Only show name if enough space
                    if node.x + 1 < grid_w {
                        let y = node.y;
                        let x = node.x + 1;
                        if y < lines.len() && x < grid_w {
                            let name = &node.name;
                            let style = Style::default().fg(Color::Gray);
                            let mut s = name.clone();
                            if s.len() > (grid_w-x) { s.truncate(grid_w-x); }
                            let span = Span::styled(s, style);
                            lines[y].spans.push(span);
                        }
                    }
                }

                // Compose info area with beautiful styling
                let mut info_lines = vec![];
                info_lines.push(ratatui::text::Line::from(vec![
                    Span::styled(" ⚡ MeshLink Network Visualization ⚡ ", 
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(ratatui::style::Modifier::BOLD)),
                ]));
                info_lines.push(ratatui::text::Line::from(vec![
                    Span::styled("Nodes: ", Style::default().fg(Color::White)),
                    Span::styled(format!("{}", g.nodes.len()), Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD)),
                    Span::raw("   "),
                    Span::styled("Links: ", Style::default().fg(Color::White)),
                    Span::styled(format!("{}", g.links.len()), Style::default().fg(Color::Blue).add_modifier(ratatui::style::Modifier::BOLD)),
                    Span::raw("   "),
                    Span::styled("Frame: ", Style::default().fg(Color::White)),
                    Span::styled(format!("{}", g.animation_frame), Style::default().fg(Color::Magenta)),
                ]));
                if let Some(ev) = &g.last_event {
                    // Parse and format event nicely
                    let event_text = if let Ok(v) = serde_json::from_str::<serde_json::Value>(ev) {
                        let node = v.get("node").and_then(|x| x.as_str()).unwrap_or("?");
                        let event = v.get("event").and_then(|x| x.as_str()).unwrap_or("?");
                        let peer = v.get("peer").and_then(|x| x.as_str()).unwrap_or("");
                        
                        let icon = match event {
                            "connected" => "🔗",
                            "disconnected" => "❌",
                            "message_sent" => "📤",
                            "message_received" => "📥",
                            "mesh_message_received" => "🌐",
                            _ => "⚡",
                        };
                        
                        if !peer.is_empty() {
                            format!("{} {} → {} ({})", icon, node, peer, event)
                        } else {
                            format!("{} {} ({})", icon, node, event)
                        }
                    } else {
                        ev.clone()
                    };
                    
                    info_lines.push(ratatui::text::Line::from(vec![
                        Span::styled("Last Event: ", Style::default().fg(Color::Gray)),
                        Span::styled(event_text, 
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(ratatui::style::Modifier::BOLD)),
                    ]));
                }
                info_lines.push(ratatui::text::Line::from(vec![
                    Span::styled("Press ", Style::default().fg(Color::Gray)),
                    Span::styled("q", Style::default().fg(Color::Red).add_modifier(ratatui::style::Modifier::BOLD)),
                    Span::styled(" to quit", Style::default().fg(Color::Gray)),
                ]));

                // Layout: grid in most of the area, info at bottom
                let vchunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Constraint::Length(grid_h as u16 + 2),
                            Constraint::Min(3),
                        ]
                        .as_ref(),
                    )
                    .split(f.size());
                let grid_area = vchunks[0];
                let info_area = vchunks[1];

                // Draw grid as Paragraph with beautiful styling
                let para = Paragraph::new(lines)
                    .block(Block::default()
                        .title(" Network Topology ")
                        .title_style(Style::default().fg(Color::Cyan).add_modifier(ratatui::style::Modifier::BOLD))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)));
                f.render_widget(para, grid_area);

                // Draw info with beautiful styling
                let info_para = Paragraph::new(info_lines)
                    .block(Block::default()
                        .title(" Status ")
                        .title_style(Style::default().fg(Color::Green).add_modifier(ratatui::style::Modifier::BOLD))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::DarkGray)));
                f.render_widget(info_para, info_area);
            })?;
            last_redraw = Instant::now();
            g.tick();
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}


