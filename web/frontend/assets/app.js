// Elysium Web Frontend - Dashboard JavaScript
const API_BASE = 'http://localhost:8000/api';

async function fetchAPI(endpoint) {
    try {
        const response = await fetch(`${API_BASE}${endpoint}`);
        return await response.json();
    } catch (error) {
        console.error('API Error:', error);
        return { error: error.message };
    }
}

async function updateStatus() {
    const status = await fetchAPI('/status');
    if (status.error) {
        document.getElementById('node-status').innerHTML = `<p>Error: ${status.error}</p>`;
        return;
    }
    
    document.getElementById('node-status').innerHTML = `
        <div class="status-item">
            <span class="status-label">Node ID:</span> ${status.node_id || 'unknown'}
        </div>
        <div class="status-item">
            <span class="status-label">Connected:</span> ${status.connected_peers || 0}/${status.total_peers || 0} peers
        </div>
    `;
}

async function updatePeers() {
    const response = await fetchAPI('/peers');
    if (response.error) {
        document.getElementById('peers-list').innerHTML = `<p>Error: ${response.error}</p>`;
        return;
    }
    
    const peers = response.peers || [];
    if (peers.length === 0) {
        document.getElementById('peers-list').innerHTML = '<p>No peers connected</p>';
        return;
    }
    
    document.getElementById('peers-list').innerHTML = peers.map(peer => `
        <div class="peer-item">
            <strong>${peer.node_id || 'unknown'}</strong><br>
            <small>${peer.address || 'unknown'} - ${peer.state || 'unknown'}</small>
        </div>
    `).join('');
}

async function updateSites() {
    const response = await fetchAPI('/sites');
    if (response.error) {
        document.getElementById('sites-list').innerHTML = `<p>Error: ${response.error}</p>`;
        return;
    }
    
    const sites = response.sites || [];
    if (sites.length === 0) {
        document.getElementById('sites-list').innerHTML = '<p>No sites available</p>';
        return;
    }
    
    document.getElementById('sites-list').innerHTML = sites.map(site => `
        <div class="site-item">
            <a href="${site.path}" target="_blank">${site.site_id}</a>
        </div>
    `).join('');
}

async function sendMessage(message, to = null) {
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
    
    const result = await response.json();
    if (result.error) {
        alert(`Error: ${result.error}`);
        return false;
    }
    
    // Add message to chat
    addChatMessage(message, 'sent');
    return true;
}

function addChatMessage(message, type) {
    const messagesDiv = document.getElementById('chat-messages');
    const messageDiv = document.createElement('div');
    messageDiv.className = `message ${type}`;
    messageDiv.textContent = message;
    messagesDiv.appendChild(messageDiv);
    messagesDiv.scrollTop = messagesDiv.scrollHeight;
}

// Event listeners
document.getElementById('send-button').addEventListener('click', async () => {
    const input = document.getElementById('message-input');
    const message = input.value.trim();
    if (message) {
        await sendMessage(message);
        input.value = '';
    }
});

document.getElementById('message-input').addEventListener('keypress', async (e) => {
    if (e.key === 'Enter') {
        const input = document.getElementById('message-input');
        const message = input.value.trim();
        if (message) {
            await sendMessage(message);
            input.value = '';
        }
    }
});

// Update dashboard every 5 seconds
setInterval(() => {
    updateStatus();
    updatePeers();
    updateSites();
}, 5000);

// Initial load
updateStatus();
updatePeers();
updateSites();

