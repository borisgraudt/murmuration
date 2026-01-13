/// Web Gateway - HTTP server for viewing ely:// content in browser
use crate::error::Result;
use crate::node::Node;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{error, info};
use base64::{Engine as _, engine::general_purpose};

pub async fn start_web_gateway(node: Arc<Node>, port: u16) -> Result<()> {
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()
        .map_err(|e| crate::error::MeshError::Io(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            format!("Invalid address: {}", e)
        )))?;
    
    let listener = TcpListener::bind(addr).await
        .map_err(crate::error::MeshError::Io)?;
    
    info!("Web Gateway started on http://{}", addr);
    
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let io = TokioIo::new(stream);
                let node_clone = node.clone();
                
                tokio::spawn(async move {
                    let service = service_fn(move |req| handle_request(req, node_clone.clone()));
                    
                    if let Err(err) = http1::Builder::new()
                        .serve_connection(io, service)
                        .await
                    {
                        error!("Error serving connection: {:?}", err);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    node: Arc<Node>,
) -> std::result::Result<Response<Full<bytes::Bytes>>, hyper::Error> {
    let path = req.uri().path();
    
    // Support both old /view?url=... and new /e/... format
    if path.starts_with("/e/") {
        // New format: /e/<encoded_ely_url>
        let encoded = path.strip_prefix("/e/").unwrap_or("");
        let url = general_purpose::URL_SAFE_NO_PAD
            .decode(encoded)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_else(|| format!("ely://{}", encoded.replace("%2F", "/")));
        
        return handle_content_request(&url, node).await;
    }
    
    match (req.method(), path) {
        (&Method::GET, "/view") => {
            // Legacy format: ?url=ely://...
            let query = req.uri().query().unwrap_or("");
            let url = extract_url_from_query(query);
            
            if url.is_empty() {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::new(bytes::Bytes::from("Missing ?url parameter")))
                    .unwrap());
            }
            
            handle_content_request(&url, node).await
        }
        (&Method::GET, "/") => {
            // Show simple homepage with instructions
            let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Elysium Web Gateway</title>
    <style>
        body { font-family: system-ui, sans-serif; max-width: 800px; margin: 50px auto; padding: 20px; }
        h1 { color: #2563eb; }
        code { background: #f3f4f6; padding: 2px 6px; border-radius: 3px; }
        a { color: #2563eb; text-decoration: none; }
        a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <h1>🌐 Elysium Web Gateway</h1>
    <p>View <code>ely://</code> content in your browser.</p>
    <h2>Usage</h2>
    <p>Open ely:// URLs like:</p>
    <p><code><a href="/view?url=ely://node_id/path">/view?url=ely://node_id/path</a></code></p>
    <h2>Example</h2>
    <p>If you published content at <code>ely://Qm7xRJ.../site/index.html</code>, visit:</p>
    <p><code><a href="/view?url=ely://Qm7xRJ.../site/index.html">/view?url=ely://Qm7xRJ.../site/index.html</a></code></p>
    <hr>
    <p><small>Gateway is running. Your node must be online to fetch content from the mesh network.</small></p>
</body>
</html>"#;
            Ok(Response::builder()
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Full::new(bytes::Bytes::from(html)))
                .unwrap())
        }
        _ => {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/plain")
                .body(Full::new(bytes::Bytes::from("Not found")))
                .unwrap())
        }
    }
}

fn extract_url_from_query(query: &str) -> String {
    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            if key == "url" {
                return urlencoding::decode(value)
                    .unwrap_or_else(|_| std::borrow::Cow::Borrowed(value))
                    .to_string();
            }
        }
    }
    String::new()
}

/// Handle content request - fetch and serve content with URL rewriting for HTML
async fn handle_content_request(
    url: &str,
    node: Arc<Node>,
) -> std::result::Result<Response<Full<bytes::Bytes>>, hyper::Error> {
    match node.fetch_content(url, Duration::from_secs(10)).await {
        Ok(Some(content)) => {
            let content_type = detect_content_type(url, &content);
            
            // For HTML content, inject JavaScript to rewrite URL in address bar
            let body_bytes = if content_type.starts_with("text/html") {
                inject_url_rewriter(&content, url)
            } else {
                content
            };
            
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", content_type)
                .header("X-Elysium-URL", url) // Custom header with original URL
                .body(Full::new(bytes::Bytes::from(body_bytes)))
                .unwrap())
        }
        Ok(None) => {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "text/plain")
                .body(Full::new(bytes::Bytes::from(format!(
                    "Content not found: {}",
                    url
                ))))
                .unwrap())
        }
        Err(e) => {
            error!("Error fetching content {}: {}", url, e);
            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "text/plain")
                .body(Full::new(bytes::Bytes::from(format!(
                    "Error fetching content: {}",
                    e
                ))))
                .unwrap())
        }
    }
}

/// Inject JavaScript to rewrite URL in address bar and handle navigation
fn inject_url_rewriter(html_content: &[u8], ely_url: &str) -> Vec<u8> {
    let html_str = String::from_utf8_lossy(html_content);
    
    // Encode URL for use in JavaScript
    let encoded_url = ely_url.replace('\\', "\\\\").replace('"', "\\\"");
    let encoded_base64 = general_purpose::URL_SAFE_NO_PAD.encode(ely_url.as_bytes());
    
    // JavaScript to rewrite URL and handle navigation
    let script = format!(
        r#"
<script>
(function() {{
    'use strict';
    
    const elyUrl = "{}";
    const gatewayUrl = "/e/{}";
    
    // Rewrite URL in address bar using History API
    try {{
        // Replace current history entry with ely:// URL
        const newState = {{ ely: elyUrl }};
        history.replaceState(newState, document.title || elyUrl, gatewayUrl);
        
        // Update document title if needed
        if (!document.title || document.title === 'Elysium Web Gateway') {{
            document.title = elyUrl;
        }}
        
        // Override browser back/forward to maintain ely:// URL
        window.addEventListener('popstate', function(e) {{
            if (e.state && e.state.ely) {{
                history.replaceState(e.state, document.title, gatewayUrl);
            }}
        }});
    }} catch (e) {{
        console.warn('Could not rewrite URL:', e);
    }}
    
    // Rewrite all ely:// links to use gateway
    document.addEventListener('DOMContentLoaded', function() {{
        // Rewrite existing links
        const links = document.querySelectorAll('a[href^="ely://"]');
        links.forEach(function(link) {{
            const href = link.getAttribute('href');
            if (href) {{
                const encoded = btoa(unescape(encodeURIComponent(href))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
                link.href = '/e/' + encoded;
            }}
        }});
        
        // Intercept clicks on ely:// links
        document.addEventListener('click', function(e) {{
            let target = e.target;
            while (target && target.tagName !== 'A') {{
                target = target.parentElement;
            }}
            
            if (target && target.href && target.href.startsWith('ely://')) {{
                e.preventDefault();
                const encoded = btoa(unescape(encodeURIComponent(target.href))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
                window.location.href = '/e/' + encoded;
            }}
        }}, true);
    }});
    
    // Rewrite links immediately if DOM is already loaded
    if (document.readyState === 'loading') {{
        // Wait for DOMContentLoaded
    }} else {{
        const links = document.querySelectorAll('a[href^="ely://"]');
        links.forEach(function(link) {{
            const href = link.getAttribute('href');
            if (href) {{
                const encoded = btoa(unescape(encodeURIComponent(href))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
                link.href = '/e/' + encoded;
            }}
        }});
    }}
}})();
</script>
"#,
        encoded_url, encoded_base64
    );
    
    // Inject script before </head> or at the beginning of <body>
    let mut result = html_str.to_string();
    
    if let Some(idx) = result.find("</head>") {
        result.insert_str(idx, &script);
    } else if let Some(idx) = result.find("<body>") {
        result.insert_str(idx + 6, &script);
    } else {
        // No head or body, prepend
        result = format!("{}{}", script, result);
    }
    
    result.into_bytes()
}

fn detect_content_type(url: &str, content: &[u8]) -> &'static str {
    // Simple detection based on URL extension and content
    if url.ends_with(".html") || url.ends_with("/") {
        "text/html; charset=utf-8"
    } else if url.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if url.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if url.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if url.ends_with(".png") {
        "image/png"
    } else if url.ends_with(".jpg") || url.ends_with(".jpeg") {
        "image/jpeg"
    } else if url.ends_with(".svg") {
        "image/svg+xml"
    } else if content.starts_with(b"<html") || content.starts_with(b"<!DOCTYPE") {
        "text/html; charset=utf-8"
    } else if content.starts_with(b"{") || content.starts_with(b"[") {
        "application/json; charset=utf-8"
    } else {
        "text/plain; charset=utf-8"
    }
}

