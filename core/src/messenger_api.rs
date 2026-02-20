/// Messenger REST API + SSE — HTTP server for Swift frontend
///
/// Port: api_port + 2  (e.g. node 8080 → TCP API 17080 → Messenger API 17082)
///
/// Endpoints:
///   GET  /api/status
///   GET  /api/peers
///   GET  /api/conversations
///   GET  /api/conversations/:peer_id   ?since=N&limit=N
///   POST /api/send                     body: {"to":"<id>|null","message":"..."}
///   GET  /api/contacts
///   POST /api/contacts                 body: {"node_id":"...","display_name":"..."}
///   DELETE /api/contacts/:node_id
///   GET  /api/profile                  own profile
///   PUT  /api/profile                  body: {"display_name":"...","bio":"..."}
///   GET  /api/profile/:node_id         ?timeout_ms=5000
///   GET  /events                       SSE stream of MessengerEvent JSON
use crate::contact_store::Contact;
use crate::error::{MeshError, Result};
use crate::messenger_types::MessengerEvent;
use crate::node::Node;
use futures_util::stream::{unfold, StreamExt};
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::Frame;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::{error, info};

// ─── Type alias ──────────────────────────────────────────────────────────────

type BoxBody = http_body_util::combinators::BoxBody<bytes::Bytes, Infallible>;
type Resp = Response<BoxBody>;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn cors_headers(builder: hyper::http::response::Builder) -> hyper::http::response::Builder {
    builder
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type")
}

fn json_resp(status: StatusCode, body: Vec<u8>) -> Resp {
    cors_headers(Response::builder())
        .status(status)
        .header("Content-Type", "application/json")
        .body(Full::new(bytes::Bytes::from(body)).boxed())
        .unwrap_or_else(|_| Response::new(Full::new(bytes::Bytes::new()).boxed()))
}

fn json_ok(value: serde_json::Value) -> Resp {
    json_resp(StatusCode::OK, serde_json::to_vec(&value).unwrap_or_default())
}

fn json_err(status: StatusCode, msg: &str) -> Resp {
    json_resp(
        status,
        serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default(),
    )
}

fn sse_resp(rx: tokio::sync::broadcast::Receiver<MessengerEvent>) -> Resp {
    // Keepalive comment sent immediately so the client knows the connection is live
    let initial = bytes::Bytes::from(": connected\n\n");
    let first = futures_util::stream::once(async move {
        Ok::<Frame<bytes::Bytes>, Infallible>(Frame::data(initial))
    });

    let events = unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let json = serde_json::to_string(&event).unwrap_or_default();
                    let data = format!("data: {}\n\n", json);
                    let frame = Frame::data(bytes::Bytes::from(data));
                    return Some((Ok::<_, Infallible>(frame), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    // Client is too slow — skip lagged events and continue
                    tracing::warn!("SSE client lagged {} events", n);
                    continue;
                }
                Err(_) => return None, // channel closed
            }
        }
    });

    let stream = first.chain(events);
    cors_headers(Response::builder())
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream; charset=utf-8")
        .header("Cache-Control", "no-cache")
        .header("X-Accel-Buffering", "no") // disable nginx buffering
        .body(StreamBody::new(stream).boxed())
        .unwrap_or_else(|_| Response::new(Full::new(bytes::Bytes::new()).boxed()))
}

// ─── Entry point ─────────────────────────────────────────────────────────────

pub async fn start_messenger_api(node: Node, port: u16) -> Result<()> {
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().map_err(|e| {
        MeshError::Io(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            format!("Invalid messenger API address: {}", e),
        ))
    })?;

    // Persist port for Swift to discover
    if let Ok(home) = std::env::var("HOME") {
        let path = std::path::Path::new(&home).join(".elysium_messenger_port");
        let _ = std::fs::write(&path, port.to_string());
    }

    let listener = TcpListener::bind(addr).await.map_err(MeshError::Io)?;
    info!("Messenger API started on http://{}", addr);

    let node = Arc::new(node);
    loop {
        match listener.accept().await {
            Ok((stream, _peer)) => {
                let io = TokioIo::new(stream);
                let node = node.clone();
                tokio::spawn(async move {
                    let svc = service_fn(move |req| {
                        let node = node.clone();
                        async move { Ok::<_, Infallible>(handle(req, node).await) }
                    });
                    if let Err(e) = http1::Builder::new().serve_connection(io, svc).await {
                        // Ignore client-disconnect errors (normal for SSE)
                        if !e.is_incomplete_message() {
                            error!("Messenger API connection error: {:?}", e);
                        }
                    }
                });
            }
            Err(e) => error!("Messenger API accept error: {}", e),
        }
    }
}

// ─── Router ──────────────────────────────────────────────────────────────────

async fn handle(req: Request<hyper::body::Incoming>, node: Arc<Node>) -> Resp {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();

    // CORS preflight
    if method == Method::OPTIONS {
        return cors_headers(Response::builder())
            .status(StatusCode::NO_CONTENT)
            .body(Full::new(bytes::Bytes::new()).boxed())
            .unwrap_or_else(|_| Response::new(Full::new(bytes::Bytes::new()).boxed()));
    }

    match (method.clone(), path.as_str()) {
        (Method::GET, "/api/status") => get_status(&node).await,
        (Method::GET, "/api/peers") => get_peers(&node).await,
        (Method::GET, "/api/conversations") => get_conversations(&node).await,
        (Method::POST, "/api/send") => post_send(req, &node).await,
        (Method::GET, "/api/contacts") => get_contacts(&node).await,
        (Method::POST, "/api/contacts") => post_add_contact(req, &node).await,
        (Method::GET, "/api/profile") => get_own_profile(&node).await,
        (Method::PUT, "/api/profile") => put_profile(req, &node).await,
        (Method::GET, "/events") => get_sse(&node),
        _ => {
            // Dynamic segments
            if method == Method::GET && path.starts_with("/api/conversations/") {
                let peer_id = path.trim_start_matches("/api/conversations/").to_string();
                return get_conversation_history(&peer_id, &query, &node).await;
            }
            if method == Method::DELETE && path.starts_with("/api/contacts/") {
                let nid = path.trim_start_matches("/api/contacts/").to_string();
                return delete_contact(&nid, &node).await;
            }
            if method == Method::GET && path.starts_with("/api/profile/") {
                let nid = path.trim_start_matches("/api/profile/").to_string();
                return get_profile(&nid, &query, &node).await;
            }
            json_err(StatusCode::NOT_FOUND, "not found")
        }
    }
}

// ─── Handlers ────────────────────────────────────────────────────────────────

async fn get_status(node: &Node) -> Resp {
    let (node_id, connected, total) = node.get_status().await;
    let api_addr = node.get_api_addr().await;
    let messenger_port = api_addr.port() + 2;
    json_ok(serde_json::json!({
        "node_id": node_id,
        "connected_peers": connected,
        "total_peers": total,
        "api_port": api_addr.port(),
        "messenger_port": messenger_port,
    }))
}

async fn get_peers(node: &Node) -> Resp {
    let peers = node.get_peers().await;
    let list: Vec<_> = peers
        .into_iter()
        .map(|(id, addr, state)| {
            serde_json::json!({
                "id": id,
                "address": addr.to_string(),
                "state": format!("{:?}", state),
            })
        })
        .collect();
    json_ok(serde_json::json!({ "peers": list }))
}

async fn get_conversations(node: &Node) -> Resp {
    let convs = node.get_conversations().await;
    json_ok(serde_json::json!({ "conversations": convs }))
}

async fn get_conversation_history(peer_id: &str, query: &str, node: &Node) -> Resp {
    let since = parse_query_u64(query, "since");
    let limit = parse_query_usize(query, "limit").unwrap_or(50).clamp(1, 500);
    let (next_since, messages) = node.get_conversation_history(peer_id, since, limit).await;
    json_ok(serde_json::json!({
        "next_since": next_since,
        "messages": messages,
    }))
}

#[derive(Deserialize)]
struct SendRequest {
    to: Option<String>,
    message: String,
}

async fn post_send(req: Request<hyper::body::Incoming>, node: &Node) -> Resp {
    let body = match read_body(req).await {
        Ok(b) => b,
        Err(e) => return json_err(StatusCode::BAD_REQUEST, &format!("body read error: {}", e)),
    };
    let req: SendRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return json_err(StatusCode::BAD_REQUEST, &format!("invalid JSON: {}", e)),
    };
    match node.send_mesh_message(req.to, req.message.into_bytes()).await {
        Ok(id) => json_ok(serde_json::json!({ "message_id": id })),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_contacts(node: &Node) -> Resp {
    match node.get_contacts().await {
        Ok(contacts) => json_ok(serde_json::json!({ "contacts": contacts })),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Deserialize)]
struct AddContactRequest {
    node_id: String,
    display_name: String,
    alias: Option<String>,
}

async fn post_add_contact(req: Request<hyper::body::Incoming>, node: &Node) -> Resp {
    let body = match read_body(req).await {
        Ok(b) => b,
        Err(e) => return json_err(StatusCode::BAD_REQUEST, &format!("body read error: {}", e)),
    };
    let r: AddContactRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return json_err(StatusCode::BAD_REQUEST, &format!("invalid JSON: {}", e)),
    };
    let contact = Contact {
        node_id: r.node_id,
        display_name: r.display_name,
        alias: r.alias,
        added_at: chrono::Utc::now().to_rfc3339(),
    };
    match node.add_contact(contact.clone()).await {
        Ok(()) => json_ok(serde_json::json!({ "contact": contact })),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn delete_contact(node_id: &str, node: &Node) -> Resp {
    match node.remove_contact(node_id).await {
        Ok(removed) => json_ok(serde_json::json!({ "removed": removed })),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_own_profile(node: &Node) -> Resp {
    let url = format!("ely://{}/messenger/profile", node.id);
    match node.fetch_content(&url, Duration::from_millis(500)).await {
        Ok(Some(bytes)) => {
            let v: serde_json::Value =
                serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
            json_ok(v)
        }
        Ok(None) => json_err(StatusCode::NOT_FOUND, "profile not set"),
        Err(_) => json_err(StatusCode::NOT_FOUND, "profile not set"),
    }
}

#[derive(Deserialize)]
struct UpdateProfileRequest {
    display_name: String,
    bio: Option<String>,
}

async fn put_profile(req: Request<hyper::body::Incoming>, node: &Node) -> Resp {
    let body = match read_body(req).await {
        Ok(b) => b,
        Err(e) => return json_err(StatusCode::BAD_REQUEST, &format!("body read error: {}", e)),
    };
    let r: UpdateProfileRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return json_err(StatusCode::BAD_REQUEST, &format!("invalid JSON: {}", e)),
    };
    match node
        .publish_profile(r.display_name, r.bio.unwrap_or_default())
        .await
    {
        Ok(()) => json_ok(serde_json::json!({ "success": true })),
        Err(e) => json_err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

async fn get_profile(node_id: &str, query: &str, node: &Node) -> Resp {
    let timeout_ms = parse_query_u64(query, "timeout_ms").unwrap_or(5000);
    let timeout = Duration::from_millis(timeout_ms);
    match node.fetch_profile(node_id, timeout).await {
        Ok(Some(v)) => json_ok(v),
        Ok(None) => json_err(StatusCode::NOT_FOUND, "profile not found"),
        Err(e) => json_err(StatusCode::GATEWAY_TIMEOUT, &e.to_string()),
    }
}

fn get_sse(node: &Node) -> Resp {
    let rx = node.ws_event_sender().subscribe();
    sse_resp(rx)
}

// ─── Utilities ────────────────────────────────────────────────────────────────

async fn read_body(req: Request<hyper::body::Incoming>) -> std::result::Result<bytes::Bytes, String> {
    req.collect()
        .await
        .map(|c| c.to_bytes())
        .map_err(|e| e.to_string())
}

fn parse_query_u64(query: &str, key: &str) -> Option<u64> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return v.parse().ok();
            }
        }
    }
    None
}

fn parse_query_usize(query: &str, key: &str) -> Option<usize> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return v.parse().ok();
            }
        }
    }
    None
}
