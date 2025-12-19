// Elysium Web Frontend - Dashboard JavaScript (Claude Code style)
// For GitHub Pages, use relative API path or configure CORS
const API_BASE = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1'
    ? 'http://localhost:8000/api'
    : '/api'; // For GitHub Pages, you'll need to configure this

let updateInterval = null;

// Update header status
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

async function fetchAPI(endpoint) {
    try {
        const response = await fetch(`${API_BASE}${endpoint}`, {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
            },
        });
        
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
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
    
    if (peers.length === 0) {
        peersDiv.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">🔌</div>
                <div class="empty-state-text">No peers connected</div>
                <div class="empty-state-text" style="margin-top: 8px; font-size: 12px; color: var(--text-dim);">
                    Start additional nodes to see them here
                </div>
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
                <strong>${peer.node_id || 'unknown'}</strong>
                <small>${peer.address || 'unknown'}</small>
                <div class="peer-status ${stateClass}">
                    <span class="peer-status-dot"></span>
                    <span>${stateText}</span>
                </div>
            </div>
        `;
    }).join('');
}

async function updateSites() {
    const response = await fetchAPI('/sites');
    const sitesDiv = document.getElementById('sites-list');
    
    if (response.error) {
        sitesDiv.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">⚠️</div>
                <div class="empty-state-text">Error: ${response.error}</div>
            </div>
        `;
        return;
    }
    
    const sites = response.sites || [];
    
    if (sites.length === 0) {
        sitesDiv.innerHTML = `
            <div class="empty-state">
                <div class="empty-state-icon">🌐</div>
                <div class="empty-state-text">No sites available</div>
            </div>
        `;
        return;
    }
    
    sitesDiv.innerHTML = sites.map(site => `
        <div class="site-item">
            <a href="${site.path || '#'}" target="_blank">
                <span>🌐</span>
                <span>${site.site_id || 'unknown'}</span>
            </a>
        </div>
    `).join('');
}

async function sendMessage(message, to = null) {
    try {
        const response = await fetch(`${API_BASE}/send`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                message: message,
                to: to
            })
        });
        
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        
        const result = await response.json();
        if (result.error) {
            showError(`Error: ${result.error}`);
            return false;
        }
        
        // Add message to chat
        addChatMessage(message, 'sent');
        return true;
    } catch (error) {
        showError(`Failed to send message: ${error.message}`);
        return false;
    }
}

function addChatMessage(message, type) {
    const messagesDiv = document.getElementById('chat-messages');
    const messageDiv = document.createElement('div');
    messageDiv.className = `message ${type}`;
    
    const time = new Date().toLocaleTimeString('en-US', { 
        hour: '2-digit', 
        minute: '2-digit' 
    });
    
    messageDiv.innerHTML = `
        <div>${escapeHtml(message)}</div>
        <div class="message-time">${time}</div>
    `;
    
    messagesDiv.appendChild(messageDiv);
    messagesDiv.scrollTop = messagesDiv.scrollHeight;
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

function showError(message) {
    // You could use a toast notification library here
    console.error(message);
    // For now, just log to console
    // In production, you might want to show a toast notification
}

// Event listeners
document.addEventListener('DOMContentLoaded', () => {
    const sendButton = document.getElementById('send-button');
    const messageInput = document.getElementById('message-input');
    
    if (sendButton) {
        sendButton.addEventListener('click', async () => {
            const message = messageInput.value.trim();
            if (message) {
                sendButton.disabled = true;
                await sendMessage(message);
                messageInput.value = '';
                sendButton.disabled = false;
                messageInput.focus();
            }
        });
    }
    
    if (messageInput) {
        messageInput.addEventListener('keypress', async (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                const message = messageInput.value.trim();
                if (message) {
                    sendButton.disabled = true;
                    await sendMessage(message);
                    messageInput.value = '';
                    sendButton.disabled = false;
                }
            }
        });
    }
    
    // Initial load
    updateStatus();
    updatePeers();
    updateSites();
    
    // Update dashboard every 5 seconds
    updateInterval = setInterval(() => {
        updateStatus();
        updatePeers();
        updateSites();
    }, 5000);
});

// Cleanup on page unload
window.addEventListener('beforeunload', () => {
    if (updateInterval) {
        clearInterval(updateInterval);
    }
});
