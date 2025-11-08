/// Tron-style Mesh Visualization
/// Futuristic neon-style network visualization inspired by Tron
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
struct Node {
    id: String,
    x: f32,
    y: f32,
    vx: f32, // velocity for smooth movement
    vy: f32,
    pulse: u8,
    last_seen: Instant,
}

#[derive(Debug, Clone)]
struct Link {
    from: usize,
    to: usize,
    pulse: u8,
    energy: f32, // 0.0 to 1.0 for energy flow animation
}

#[derive(Debug, Clone)]
struct TronGraph {
    nodes: Vec<Node>,
    links: Vec<Link>,
    node_map: BTreeMap<String, usize>,
    last_event: Option<String>,
    animation_frame: u64,
    grid_w: usize,
    grid_h: usize,
}

impl TronGraph {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            node_map: BTreeMap::new(),
            last_event: None,
            animation_frame: 0,
            grid_w: 80,
            grid_h: 24,
        }
    }

    fn find_or_add_node(&mut self, id: &str) -> usize {
        if let Some(&idx) = self.node_map.get(id) {
            idx
        } else {
            let idx = self.nodes.len();
            // Place nodes in a circle initially
            let angle = (idx as f32) * 2.0 * std::f32::consts::PI / 8.0;
            let radius = (self.grid_w.min(self.grid_h) as f32) * 0.3;
            let x = (self.grid_w as f32) / 2.0 + radius * angle.cos();
            let y = (self.grid_h as f32) / 2.0 + radius * angle.sin();
            
            self.nodes.push(Node {
                id: id.to_string(),
                x,
                y,
                vx: 0.0,
                vy: 0.0,
                pulse: 10,
                last_seen: Instant::now(),
            });
            self.node_map.insert(id.to_string(), idx);
            idx
        }
    }

    fn apply(&mut self, msg: &str) {
        self.last_event = Some(msg.to_string());
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(msg) {
            let node_id = v.get("node").and_then(|x| x.as_str()).unwrap_or("unknown");
            let event = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
            let peer_id = v.get("peer").and_then(|x| x.as_str());

            let node_idx = self.find_or_add_node(node_id);
            self.nodes[node_idx].pulse = 15;
            self.nodes[node_idx].last_seen = Instant::now();

            if let Some(peer) = peer_id {
                if !peer.is_empty() {
                    let peer_idx = self.find_or_add_node(peer);
                    self.nodes[peer_idx].pulse = 15;
                    self.nodes[peer_idx].last_seen = Instant::now();

                    if event == "connected" {
                        // Add or update link
                        if let Some(link) = self.links.iter_mut()
                            .find(|l| (l.from == node_idx && l.to == peer_idx) || 
                                     (l.from == peer_idx && l.to == node_idx)) {
                            link.pulse = 20;
                            link.energy = 1.0;
                        } else {
                            self.links.push(Link {
                                from: node_idx,
                                to: peer_idx,
                                pulse: 20,
                                energy: 1.0,
                            });
                        }
                    } else if event == "disconnected" {
                        // Remove link
                        self.links.retain(|l| !((l.from == node_idx && l.to == peer_idx) || 
                                               (l.from == peer_idx && l.to == node_idx)));
                    }
                }
            }
        }
    }

    fn tick(&mut self) {
        self.animation_frame += 1;

        // Update node pulses
        for node in &mut self.nodes {
            if node.pulse > 0 {
                node.pulse -= 1;
            }
        }

        // Update link pulses and energy
        for link in &mut self.links {
            if link.pulse > 0 {
                link.pulse -= 1;
            }
            // Animate energy flow
            link.energy = ((self.animation_frame as f32 * 0.1) % (std::f32::consts::PI * 2.0)).sin() * 0.5 + 0.5;
        }

        // Remove stale nodes (not seen for 60 seconds)
        let now = Instant::now();
        let stale: Vec<String> = self.nodes.iter()
            .enumerate()
            .filter(|(_, n)| now.duration_since(n.last_seen) > Duration::from_secs(60))
            .map(|(_, n)| n.id.clone())
            .collect();
        
        for id in stale {
            if let Some(&idx) = self.node_map.get(&id) {
                self.nodes.remove(idx);
                self.links.retain(|l| l.from != idx && l.to != idx);
                self.node_map.remove(&id);
                // Rebuild map
                self.node_map.clear();
                for (i, node) in self.nodes.iter().enumerate() {
                    self.node_map.insert(node.id.clone(), i);
                }
                // Update link indices
                for link in &mut self.links {
                    if link.from > idx { link.from -= 1; }
                    if link.to > idx { link.to -= 1; }
                }
            }
        }
    }

    fn draw_tron_style(&self, area: Rect) -> Vec<ratatui::text::Line<'_>> {
        let mut grid = vec![vec![' '; area.width as usize]; area.height as usize];
        let mut colors = vec![vec![Color::Black; area.width as usize]; area.height as usize];

        // Draw links first (behind nodes) - Tron style neon lines
        for link in &self.links {
            let from = &self.nodes[link.from];
            let to = &self.nodes[link.to];
            
            let x0 = (from.x * (area.width as f32) / self.grid_w as f32) as usize;
            let y0 = (from.y * (area.height as f32) / self.grid_h as f32) as usize;
            let x1 = (to.x * (area.width as f32) / self.grid_w as f32) as usize;
            let y1 = (to.y * (area.height as f32) / self.grid_h as f32) as usize;

            // Draw line with Bresenham
            let mut x = x0 as isize;
            let mut y = y0 as isize;
            let dx = (x1 as isize - x).abs();
            let dy = (y1 as isize - y).abs();
            let sx = if x0 < x1 { 1 } else { -1 };
            let sy = if y0 < y1 { 1 } else { -1 };
            let mut err = dx - dy;
            let mut pos = 0;
            let total = (dx + dy).max(1) as usize;

            loop {
                if x >= 0 && x < area.width as isize && y >= 0 && y < area.height as isize {
                    let ratio = pos as f32 / total as f32;
                    let energy_pos = (ratio + link.energy) % 1.0;
                    
                    // Tron-style neon color
                    let color = if link.pulse > 0 {
                        // Active link - bright cyan/blue
                        if energy_pos < 0.3 {
                            Color::Cyan
                        } else if energy_pos < 0.6 {
                            Color::LightCyan
                        } else {
                            Color::Blue
                        }
                    } else {
                        // Inactive - dim blue
                        Color::DarkGray
                    };

                    // Use box-drawing or dot for line
                    let ch = if dx == 0 {
                        '│'
                    } else if dy == 0 {
                        '─'
                    } else if sx == sy {
                        '╲'
                    } else {
                        '╱'
                    };

                    grid[y as usize][x as usize] = ch;
                    colors[y as usize][x as usize] = color;
                }

                if x == x1 as isize && y == y1 as isize { break; }
                let e2 = 2 * err;
                if e2 > -dy { err -= dy; x += sx; }
                if e2 < dx { err += dx; y += sy; }
                pos += 1;
            }
        }

        // Draw nodes - Tron style glowing orbs
        for node in &self.nodes {
            let x = (node.x * (area.width as f32) / self.grid_w as f32) as usize;
            let y = (node.y * (area.height as f32) / self.grid_h as f32) as usize;

            if x < area.width as usize && y < area.height as usize {
                // Glowing node
                let pulse_intensity = (node.pulse as f32 / 15.0).min(1.0);
                let color = if pulse_intensity > 0.5 {
                    Color::Yellow
                } else if pulse_intensity > 0.2 {
                    Color::LightYellow
                } else {
                    Color::White
                };

                grid[y][x] = '●';
                colors[y][x] = color;
            }
        }

        // Convert to lines
        let mut lines = Vec::new();
        for (y, row) in grid.iter().enumerate() {
            let mut spans = Vec::new();
            for (x, &ch) in row.iter().enumerate() {
                let color = colors[y][x];
                let style = Style::default()
                    .fg(color)
                    .bg(Color::Black)
                    .add_modifier(if color != Color::Black && color != Color::DarkGray {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    });
                spans.push(ratatui::text::Span::styled(ch.to_string(), style));
            }
            lines.push(ratatui::text::Line::from(spans));
        }

        lines
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // UDP listener
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let sock = UdpSocket::bind("127.0.0.1:9999").await.expect("bind 9999");
        let mut buf = vec![0u8; 2048];
        loop {
            if let Ok((n, _)) = sock.recv_from(&mut buf).await {
                if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                    let _ = tx.send(s.to_string());
                }
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
    let mut graph = TronGraph::new();
    let mut last_redraw = Instant::now();
    let mut last_tick = Instant::now();

    loop {
        // Process messages
        while let Ok(msg) = rx.try_recv() {
            graph.apply(&msg);
        }

        // Update animation
        if last_tick.elapsed() >= Duration::from_millis(50) {
            graph.tick();
            last_tick = Instant::now();
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

        // Redraw
        if last_redraw.elapsed() >= Duration::from_millis(16) {
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                    .split(f.size());

                // Main visualization area
                let lines = graph.draw_tron_style(chunks[0]);
                let para = Paragraph::new(lines)
                    .block(Block::default()
                        .title(" TRON MESH NETWORK ")
                        .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)));
                f.render_widget(para, chunks[0]);

                // Info panel
                let info = format!(
                    " Nodes: {} | Links: {} | Frame: {} | Press 'q' to quit ",
                    graph.nodes.len(),
                    graph.links.len(),
                    graph.animation_frame
                );
                let info_para = Paragraph::new(info)
                    .style(Style::default().fg(Color::Cyan).bg(Color::Black))
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)));
                f.render_widget(info_para, chunks[1]);
            })?;
            last_redraw = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(8));
    }
}

