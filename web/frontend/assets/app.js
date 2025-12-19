// Elysium Web - Network Visualization
// Shows nodes and message flow between them

const API_BASE = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1'
    ? 'http://localhost:8000/api'
    : '/api';

// Canvas setup
const canvas = document.getElementById('network-canvas');
const ctx = canvas.getContext('2d');

// Network state
let nodes = new Map(); // node_id -> {x, y, id, color, lastSeen}
let connections = new Map(); // "node1-node2" -> {from, to, messages: []}
let messages = []; // Active message animations
let selectedNode = null;
let isDragging = false;
let dragNode = null;
let offsetX = 0;
let offsetY = 0;
let viewOffsetX = 0;
let viewOffsetY = 0;
let scale = 1;

// Colors
const NODE_COLORS = [
    '#58a6ff', '#3fb950', '#f0883e', '#d29922', 
    '#db61a2', '#58a6ff', '#79c0ff', '#a5a5a5'
];
const MESSAGE_COLOR = '#d29922';
const CONNECTION_COLOR = '#30363d';

// Initialize canvas
function resizeCanvas() {
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width;
    canvas.height = rect.height;
    draw();
}

window.addEventListener('resize', resizeCanvas);
resizeCanvas();

// Node management
function addOrUpdateNode(nodeId, address, state) {
    if (!nodes.has(nodeId)) {
        // New node - place it in a circle
        const angle = (nodes.size * 2 * Math.PI) / Math.max(1, nodes.size + 1);
        const radius = Math.min(canvas.width, canvas.height) * 0.3;
        const x = canvas.width / 2 + radius * Math.cos(angle) + viewOffsetX;
        const y = canvas.height / 2 + radius * Math.sin(angle) + viewOffsetY;
        
        nodes.set(nodeId, {
            id: nodeId,
            address: address,
            x: x,
            y: y,
            color: NODE_COLORS[nodes.size % NODE_COLORS.length],
            state: state,
            lastSeen: Date.now(),
            pulse: 0
        });
    } else {
        // Update existing node
        const node = nodes.get(nodeId);
        node.state = state;
        node.lastSeen = Date.now();
        node.address = address;
    }
}

function removeNode(nodeId) {
    nodes.delete(nodeId);
    // Remove connections
    for (const [key, conn] of connections.entries()) {
        if (conn.from === nodeId || conn.to === nodeId) {
            connections.delete(key);
        }
    }
}

// Connection management
function addConnection(fromId, toId) {
    const key = [fromId, toId].sort().join('-');
    if (!connections.has(key)) {
        connections.set(key, {
            from: fromId,
            to: toId,
            messages: [],
            lastMessage: 0
        });
    }
    return connections.get(key);
}

// Message animation
function sendMessage(fromId, toId, messageText) {
    const fromNode = nodes.get(fromId);
    const toNode = nodes.get(toId);
    
    if (!fromNode || !toNode) return;
    
    const conn = addConnection(fromId, toId);
    const message = {
        from: fromId,
        to: toId,
        progress: 0,
        text: messageText || '',
        startTime: Date.now(),
        connection: conn
    };
    
    messages.push(message);
    conn.messages.push(message);
    conn.lastMessage = Date.now();
    
    // Pulse nodes
    if (fromNode) fromNode.pulse = 1;
    if (toNode) toNode.pulse = 1;
    
    // Log message
    addMessageLog(fromId, toId, messageText, 'sent');
}

function addMessageLog(fromId, toId, text, type) {
    const log = document.getElementById('messages-log');
    const entry = document.createElement('div');
    entry.className = `message-log-entry ${type}`;
    
    const time = new Date().toLocaleTimeString('en-US', { 
        hour: '2-digit', 
        minute: '2-digit',
        second: '2-digit'
    });
    
    const fromShort = fromId.substring(0, 8);
    const toShort = toId ? toId.substring(0, 8) : 'all';
    
    entry.innerHTML = `
        <div class="message-log-time">${time}</div>
        <div><strong>${fromShort}</strong> → <strong>${toShort}</strong></div>
        <div style="color: var(--text-dim); margin-top: 4px;">${text || 'Message'}</div>
    `;
    
    log.insertBefore(entry, log.firstChild);
    
    // Keep only last 50 messages
    while (log.children.length > 50) {
        log.removeChild(log.lastChild);
    }
}

// Drawing functions
function drawNode(node) {
    const { x, y, color, state, pulse } = node;
    const radius = 25;
    
    // Connection glow
    if (pulse > 0) {
        ctx.save();
        ctx.globalAlpha = pulse * 0.3;
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.arc(x, y, radius + 10, 0, Math.PI * 2);
        ctx.fill();
        ctx.restore();
    }
    
    // Node circle
    ctx.save();
    ctx.fillStyle = color;
    ctx.strokeStyle = state === 'Connected' ? '#3fb950' : '#d29922';
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.arc(x, y, radius, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
    ctx.restore();
    
    // Node ID (short)
    ctx.save();
    ctx.fillStyle = '#ffffff';
    ctx.font = 'bold 12px Inter, sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    const shortId = node.id.substring(0, 6);
    ctx.fillText(shortId, x, y);
    ctx.restore();
    
    // Address below
    if (node.address) {
        ctx.save();
        ctx.fillStyle = '#8b949e';
        ctx.font = '10px Inter, sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'top';
        ctx.fillText(node.address, x, y + radius + 5);
        ctx.restore();
    }
}

function drawConnection(conn) {
    const fromNode = nodes.get(conn.from);
    const toNode = nodes.get(conn.to);
    
    if (!fromNode || !toNode) return;
    
    const { x: x1, y: y1 } = fromNode;
    const { x: x2, y: y2 } = toNode;
    
    // Connection line
    ctx.save();
    ctx.strokeStyle = CONNECTION_COLOR;
    ctx.lineWidth = 2;
    ctx.setLineDash([5, 5]);
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();
    ctx.restore();
    
    // Draw messages on connection
    conn.messages.forEach(msg => {
        if (msg.progress >= 1) return;
        
        const t = msg.progress;
        const x = x1 + (x2 - x1) * t;
        const y = y1 + (y2 - y1) * t;
        
        // Message dot
        ctx.save();
        ctx.fillStyle = MESSAGE_COLOR;
        ctx.beginPath();
        ctx.arc(x, y, 6, 0, Math.PI * 2);
        ctx.fill();
        
        // Glow
        ctx.shadowBlur = 10;
        ctx.shadowColor = MESSAGE_COLOR;
        ctx.fill();
        ctx.restore();
    });
}

function draw() {
    // Clear canvas
    ctx.fillStyle = '#0d1117';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    
    // Draw connections
    for (const conn of connections.values()) {
        drawConnection(conn);
    }
    
    // Draw nodes
    for (const node of nodes.values()) {
        drawNode(node);
    }
    
    // Update message animations
    const now = Date.now();
    messages = messages.filter(msg => {
        msg.progress += 0.02;
        if (msg.progress >= 1) {
            // Message arrived
            if (msg.connection) {
                const idx = msg.connection.messages.indexOf(msg);
                if (idx > -1) msg.connection.messages.splice(idx, 1);
            }
            addMessageLog(msg.to, msg.from, msg.text, 'received');
            return false;
        }
        return true;
    });
    
    // Update node pulses
    for (const node of nodes.values()) {
        if (node.pulse > 0) {
            node.pulse -= 0.05;
            if (node.pulse < 0) node.pulse = 0;
        }
    }
    
    requestAnimationFrame(draw);
}

// Canvas interaction
canvas.addEventListener('mousedown', (e) => {
    const rect = canvas.getBoundingClientRect();
    const x = (e.clientX - rect.left) / scale;
    const y = (e.clientY - rect.top) / scale;
    
    // Check if clicking on a node
    for (const node of nodes.values()) {
        const dx = x - node.x;
        const dy = y - node.y;
        const dist = Math.sqrt(dx * dx + dy * dy);
        
        if (dist < 25) {
            isDragging = true;
            dragNode = node;
            offsetX = dx;
            offsetY = dy;
            selectedNode = node;
            canvas.style.cursor = 'grabbing';
            return;
        }
    }
    
    // Pan view
    isDragging = true;
    offsetX = x;
    offsetY = y;
});

canvas.addEventListener('mousemove', (e) => {
    const rect = canvas.getBoundingClientRect();
    const x = (e.clientX - rect.left) / scale;
    const y = (e.clientY - rect.top) / scale;
    
    if (isDragging) {
        if (dragNode) {
            dragNode.x = x - offsetX;
            dragNode.y = y - offsetY;
        } else {
            viewOffsetX += (x - offsetX) * 0.1;
            viewOffsetY += (y - offsetY) * 0.1;
        }
    }
});

canvas.addEventListener('mouseup', () => {
    isDragging = false;
    dragNode = null;
    canvas.style.cursor = 'default';
});

canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const delta = e.deltaY > 0 ? 0.9 : 1.1;
    scale *= delta;
    scale = Math.max(0.5, Math.min(2, scale));
});

// Reset view button
document.getElementById('reset-view').addEventListener('click', () => {
    viewOffsetX = 0;
    viewOffsetY = 0;
    scale = 1;
    // Rearrange nodes in circle
    const nodeArray = Array.from(nodes.values());
    const radius = Math.min(canvas.width, canvas.height) * 0.3;
    nodeArray.forEach((node, i) => {
        const angle = (i * 2 * Math.PI) / Math.max(1, nodeArray.length);
        node.x = canvas.width / 2 + radius * Math.cos(angle);
        node.y = canvas.height / 2 + radius * Math.sin(angle);
    });
});

// API functions
async function fetchAPI(endpoint) {
    try {
        const response = await fetch(`${API_BASE}${endpoint}`, {
            method: 'GET',
            headers: { 'Content-Type': 'application/json' },
        });
        
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        
        const data = await response.json();
        updateHeaderStatus(true);
        return data;
    } catch (error) {
        console.error('API Error:', error);
        updateHeaderStatus(false);
        return { error: error.message };
    }
}

function updateHeaderStatus(connected) {
    const statusDot = document.querySelector('.status-dot');
    const statusText = document.querySelector('.status-text');
    
    if (connected) {
        statusDot.className = 'status-dot connected';
        statusText.textContent = 'Connected';
    } else {
        statusDot.className = 'status-dot error';
        statusText.textContent = 'Disconnected';
    }
}

async function updateStatus() {
    const status = await fetchAPI('/status');
    const statusDiv = document.getElementById('node-status');
    
    if (status.error) {
        statusDiv.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">⚠️</div>
                <div class="empty-state-text">Error: ${status.error}</div>
            </div>
        `;
        return;
    }
    
    // Add/update current node
    if (status.node_id) {
        addOrUpdateNode(status.node_id, `localhost:${status.api_port || 'unknown'}`, 'Connected');
    }
    
    statusDiv.innerHTML = `
        <div class="status-item">
            <span class="status-label">Node ID</span>
            <div class="status-value">${status.node_id || 'unknown'}</div>
        </div>
        <div class="status-item">
            <span class="status-label">Connected Peers</span>
            <div class="status-value">
                <span style="color: var(--success);">${status.connected_peers || 0}</span>
                <span style="color: var(--text-dim);">/</span>
                <span style="color: var(--text-secondary);">${status.total_peers || 0}</span>
            </div>
        </div>
    `;
}

async function updatePeers() {
    const response = await fetchAPI('/peers');
    const peersDiv = document.getElementById('peers-list');
    const peersCount = document.getElementById('peers-count');
    
    if (response.error) {
        peersDiv.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">⚠️</div>
                <div class="empty-state-text">Error: ${response.error}</div>
            </div>
        `;
        peersCount.textContent = '0';
        return;
    }
    
    const peers = response.peers || [];
    peersCount.textContent = peers.length.toString();
    
    // Update nodes on canvas
    const currentStatus = await fetchAPI('/status');
    const currentNodeId = currentStatus.node_id;
    
    peers.forEach(peer => {
        const state = peer.state || 'unknown';
        addOrUpdateNode(peer.id, peer.address, state);
        
        // Add connection to current node
        if (currentNodeId && peer.id !== currentNodeId) {
            addConnection(currentNodeId, peer.id);
        }
    });
    
    // Remove nodes that are no longer in peers list
    const peerIds = new Set(peers.map(p => p.id));
    if (currentNodeId) peerIds.add(currentNodeId);
    
    for (const nodeId of nodes.keys()) {
        if (!peerIds.has(nodeId)) {
            removeNode(nodeId);
        }
    }
    
    if (peers.length === 0) {
        peersDiv.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">🔌</div>
                <div class="empty-state-text">No peers connected</div>
            </div>
        `;
        return;
    }
    
    peersDiv.innerHTML = peers.map(peer => {
        const state = peer.state || 'unknown';
        let stateClass = 'disconnected';
        let stateText = 'Disconnected';
        
        if (state.includes('Connected')) {
            stateClass = 'connected';
            stateText = 'Connected';
        } else if (state.includes('Handshaking') || state.includes('Connecting')) {
            stateClass = 'connecting';
            stateText = 'Connecting';
        }
        
        return `
            <div class="peer-item">
                <strong>${peer.id.substring(0, 32)}...</strong>
                <small>${peer.address || 'unknown'}</small>
                <div class="peer-status ${stateClass}">
                    <span class="peer-status-dot"></span>
                    <span>${stateText}</span>
                </div>
            </div>
        `;
    }).join('');
}

// Simulate message flow (for demo - replace with real API events)
function simulateMessage() {
    const nodeArray = Array.from(nodes.values());
    if (nodeArray.length < 2) return;
    
    const fromIdx = Math.floor(Math.random() * nodeArray.length);
    let toIdx = Math.floor(Math.random() * nodeArray.length);
    while (toIdx === fromIdx && nodeArray.length > 1) {
        toIdx = Math.floor(Math.random() * nodeArray.length);
    }
    
    const from = nodeArray[fromIdx];
    const to = nodeArray[toIdx];
    
    sendMessage(from.id, to.id, `Message ${Date.now() % 1000}`);
}

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    updateStatus();
    updatePeers();
    
    // Update every 2 seconds
    setInterval(() => {
        updateStatus();
        updatePeers();
    }, 2000);
    
    // Start drawing loop
    draw();
    
    // Simulate messages every 5 seconds (for demo)
    // setInterval(simulateMessage, 5000);
});

// Start drawing
draw();
