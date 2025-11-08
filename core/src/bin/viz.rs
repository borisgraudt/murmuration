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
            self.links.push(Link { a, b, pulse: 0 });
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
            self.pulse_nodes.insert(node_idx, 5);

            if !peer.is_empty() {
                let peer_idx = self.find_or_add_node(peer);
                // Update last seen for peer
                self.node_last_seen.insert(peer.to_string(), std::time::Instant::now());
                if event == "connected" {
                    let link_idx = self.add_link(node_idx, peer_idx);
                    self.links[link_idx].pulse = 5;
                } else if event == "disconnected" {
                    // Remove link when disconnected
                    if let Some(link_idx) = self.find_link(node_idx, peer_idx) {
                        self.links.remove(link_idx);
                    }
                }
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
                    let mut point_idx = 0;
                    let total_points = (dx + dy) as usize;
                    
                    loop {
                        if !first && (x0 >= 0 && x0 < grid_w as isize && y0 >= 0 && y0 < grid_h as isize) {
                            // Pick char for direction - use clean, professional box-drawing
                            // For neat appearance, use proper unicode box-drawing characters
                            let ch = if dx == 0 {
                                // Perfect vertical line
                                '│'
                            } else if dy == 0 {
                                // Perfect horizontal line
                                '─'
                            } else {
                                // For diagonal lines, determine which direction to emphasize
                                // Use cleaner diagonal characters that look more professional
                                let ratio = dy as f32 / dx as f32;
                                if ratio > 1.0 {
                                    // More vertical - use vertical with slight angle
                                    '│'
                                } else if ratio < 0.5 {
                                    // More horizontal - use horizontal with slight angle
                                    '─'
                                } else {
                                    // True diagonal - use proper box-drawing diagonal
                                    if sx == sy {
                                        '╲'  // Top-left to bottom-right
                                    } else {
                                        '╱'  // Top-right to bottom-left
                                    }
                                }
                            };
                            
                            // Beautiful gradient effect based on position and pulse
                            let (ch, style) = if link.pulse > 0 {
                                // Active link with pulsing animation
                                let pulse_phase = (g.animation_frame / 3) % 4;
                                let pos_phase = (point_idx * 2 + pulse_phase as usize) % 8;
                                
                                // Create flowing gradient effect
                                let color = match pos_phase {
                                    0 | 7 => Color::Cyan,
                                    1 | 6 => Color::LightCyan,
                                    2 | 5 => Color::Blue,
                                    3 | 4 => Color::LightBlue,
                                    _ => Color::Cyan,
                                };
                                
                                // Use thicker, brighter characters for active links
                                let active_ch = match ch {
                                    '│' => '┃',  // Thick vertical
                                    '─' => '━',  // Thick horizontal
                                    '╲' => '╲',  // Diagonal (already smooth)
                                    '╱' => '╱',  // Diagonal (already smooth)
                                    _ => ch,
                                };
                                
                                (active_ch, Some(Style::default()
                                    .fg(color)
                                    .bg(Color::Black)
                                    .add_modifier(ratatui::style::Modifier::BOLD)))
                            } else {
                                // Inactive link - use subtle, clean gray
                                // Make it look professional and neat
                                (ch, Some(Style::default()
                                    .fg(Color::DarkGray)
                                    .bg(Color::Black)))
                            };
                            
                            // Only overwrite if cell is empty or has lower priority
                            let current = &grid[y0 as usize][x0 as usize];
                            if current.0 == ' ' || link.pulse > 0 {
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

                // Draw nodes with beautiful styling
                for (idx, node) in g.nodes.iter().enumerate() {
                    let pulse = g.pulse_nodes.get(&idx).copied().unwrap_or(0);
                    let (ch, style) = if pulse > 0 {
                        // Pulsing node - create glow effect
                        let pulse_phase = (g.animation_frame / 2) % 4;
                        let color = match pulse_phase {
                            0 => Color::Yellow,
                            1 => Color::LightYellow,
                            2 => Color::Yellow,
                            _ => Color::White,
                        };
                        ('◉', Some(Style::default()
                            .fg(color)
                            .bg(Color::Black)
                            .add_modifier(ratatui::style::Modifier::BOLD)))
                    } else {
                        // Normal node
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

                // Compose info area
                let mut info_lines = vec![];
                info_lines.push(ratatui::text::Line::from(vec![
                    Span::styled(" Sci-Fi Mesh Visualization ", Style::default().fg(Color::Magenta)),
                ]));
                info_lines.push(ratatui::text::Line::from(vec![
                    Span::raw(format!("Nodes: {}", g.nodes.len())),
                    Span::raw("   "),
                    Span::raw(format!("Links: {}", g.links.len())),
                    Span::raw("   "),
                    Span::raw("Press q to quit"),
                ]));
                if let Some(ev) = &g.last_event {
                    info_lines.push(ratatui::text::Line::from(vec![
                        Span::styled(ev.clone(), Style::default().fg(Color::Yellow)),
                    ]));
                }

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

                // Draw grid as Paragraph
                let para = Paragraph::new(lines)
                    .block(Block::default().title("Mesh Grid").borders(Borders::ALL));
                f.render_widget(para, grid_area);

                // Draw info
                let info_para = Paragraph::new(info_lines)
                    .block(Block::default().title("Info").borders(Borders::ALL));
                f.render_widget(info_para, info_area);
            })?;
            last_redraw = Instant::now();
            g.tick();
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}


