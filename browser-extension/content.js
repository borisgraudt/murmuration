// Intercept clicks on ely:// links
document.addEventListener('click', (e) => {
  let target = e.target;
  
  // Find anchor element
  while (target && target.tagName !== 'A') {
    target = target.parentElement;
  }
  
  if (target && target.href && target.href.startsWith('ely://')) {
    e.preventDefault();
    e.stopPropagation();
    
    // Send to background script
    chrome.runtime.sendMessage({
      type: 'ely-link-clicked',
      url: target.href
    });
    
    return false;
  }
}, true);

// Listen for ely:// URL updates from background script
window.addEventListener('elysium-url', (event) => {
  const { elyUrl, gatewayUrl } = event.detail;
  
  // Update page title
  document.title = elyUrl;
  
  // Try to update URL bar display (limited - browser security)
  try {
    const state = { ely: elyUrl, gateway: gatewayUrl };
    history.replaceState(state, elyUrl, gatewayUrl);
  } catch (e) {
    console.warn('Could not update history:', e);
  }
});

// Rewrite ely:// links to use gateway
function rewriteLinks() {
  const links = document.querySelectorAll('a[href^="ely://"]');
  links.forEach(link => {
    const href = link.getAttribute('href');
    if (href) {
      // Keep original href for click interception
      // Just add data attribute
      link.setAttribute('data-ely-original', href);
    }
  });
}

// Run immediately and on DOM changes
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', rewriteLinks);
} else {
  rewriteLinks();
}

// Watch for dynamically added links
const observer = new MutationObserver(rewriteLinks);
if (document.body || document.documentElement) {
  observer.observe(document.body || document.documentElement, {
    childList: true,
    subtree: true
  });
}

