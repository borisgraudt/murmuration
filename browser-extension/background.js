// Default Web Gateway port (API port + 1, default API is 17080)
const DEFAULT_WEB_PORT = 17081;

// Storage key for Web Gateway port
const STORAGE_KEY = 'elysium_web_port';

// Get Web Gateway port from storage or use default
async function getWebPort() {
  const result = await chrome.storage.sync.get([STORAGE_KEY]);
  return result[STORAGE_KEY] || DEFAULT_WEB_PORT;
}

// Encode ely:// URL to base64 for gateway
function encodeElyUrl(url) {
  // Use URL-safe base64 encoding (matches Rust implementation)
  const bytes = new TextEncoder().encode(url);
  return btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');
}

// Decode base64 to ely:// URL
function decodeElyUrl(encoded) {
  const base64 = encoded.replace(/-/g, '+').replace(/_/g, '/');
  const bytes = Uint8Array.from(atob(base64), c => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

// Convert ely:// URL to Web Gateway URL
async function elyToGatewayUrl(elyUrl) {
  const port = await getWebPort();
  const encoded = encodeElyUrl(elyUrl);
  return `http://localhost:${port}/e/${encoded}`;
}

// Convert Web Gateway URL back to ely://
function gatewayToElyUrl(gatewayUrl) {
  try {
    const url = new URL(gatewayUrl);
    if (url.pathname.startsWith('/e/')) {
      const encoded = url.pathname.substring(3);
      return decodeElyUrl(encoded);
    }
  } catch (e) {
    console.error('Error parsing gateway URL:', e);
  }
  return null;
}

// Intercept navigation to ely:// URLs
chrome.webNavigation.onBeforeNavigate.addListener(
  async (details) => {
    if (details.frameId !== 0) return; // Only main frame
    
    const url = details.url;
    if (url.startsWith('ely://')) {
      console.log('Intercepting ely:// URL:', url);
      
      // Redirect to Web Gateway
      const gatewayUrl = await elyToGatewayUrl(url);
      
      chrome.tabs.update(details.tabId, {
        url: gatewayUrl
      });
    }
  },
  { url: [{ urlPrefix: 'ely://' }] }
);

// Update address bar to show ely:// URL when viewing gateway pages
chrome.webNavigation.onCompleted.addListener(
  async (details) => {
    if (details.frameId !== 0) return;
    
    const url = details.url;
    const elyUrl = gatewayToElyUrl(url);
    
    if (elyUrl) {
      console.log('Detected gateway URL, original ely://:', elyUrl);
      
      // Update tab title to show ely:// URL
      try {
        await chrome.tabs.executeScript(details.tabId, {
          code: `
            // Update document title immediately
            document.title = '${elyUrl}';
            
            // Update when DOM is ready
            if (document.readyState === 'loading') {
              document.addEventListener('DOMContentLoaded', function() {
                document.title = '${elyUrl}';
              });
            } else {
              document.title = '${elyUrl}';
            }
            
            // Try to use History API (limited - browser security)
            try {
              const state = { ely: '${elyUrl}', gateway: '${url}' };
              history.replaceState(state, '${elyUrl}', '${url}');
              
              // Dispatch event for content script
              window.dispatchEvent(new CustomEvent('elysium-url', { 
                detail: { elyUrl: '${elyUrl}', gatewayUrl: '${url}' } 
              }));
            } catch (e) {
              console.warn('Could not update history:', e);
            }
          `
        });
      } catch (e) {
        // Fallback for Manifest V3
        try {
          await chrome.scripting.executeScript({
            target: { tabId: details.tabId },
            func: (elyUrl, gatewayUrl) => {
              document.title = elyUrl;
              const state = { ely: elyUrl, gateway: gatewayUrl };
              history.replaceState(state, elyUrl, gatewayUrl);
              window.dispatchEvent(new CustomEvent('elysium-url', { 
                detail: { elyUrl, gatewayUrl } 
              }));
            },
            args: [elyUrl, url]
          });
        } catch (err) {
          console.warn('Could not inject script:', err);
        }
      }
    }
  },
  { url: [{ hostEquals: 'localhost' }] }
);

// Handle clicks on ely:// links
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === 'ely-link-clicked') {
    const elyUrl = message.url;
    console.log('Ely link clicked:', elyUrl);
    
    elyToGatewayUrl(elyUrl).then(gatewayUrl => {
      chrome.tabs.update(sender.tab.id, {
        url: gatewayUrl
      });
    });
    
    sendResponse({ success: true });
    return true;
  }
});

