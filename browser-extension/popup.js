const STORAGE_KEY = 'elysium_web_port';
const DEFAULT_PORT = 17081;

document.addEventListener('DOMContentLoaded', async () => {
  const portInput = document.getElementById('web-port');
  const saveBtn = document.getElementById('save');
  const status = document.getElementById('status');
  
  // Load saved port
  const result = await chrome.storage.sync.get([STORAGE_KEY]);
  portInput.value = result[STORAGE_KEY] || DEFAULT_PORT;
  
  saveBtn.addEventListener('click', async () => {
    const port = parseInt(portInput.value);
    
    if (port < 1024 || port > 65535) {
      showStatus('Invalid port (must be 1024-65535)', 'error');
      return;
    }
    
    await chrome.storage.sync.set({ [STORAGE_KEY]: port });
    showStatus('Settings saved!', 'success');
  });
  
  function showStatus(message, type) {
    status.textContent = message;
    status.className = `status ${type}`;
    status.style.display = 'block';
    
    setTimeout(() => {
      status.style.display = 'none';
    }, 2000);
  }
});

