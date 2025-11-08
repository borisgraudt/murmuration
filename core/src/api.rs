/// API server for CLI and external clients
use crate::error::{MeshError, Result};
use crate::node::Node;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

/// API request
#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
enum ApiRequest {
    #[serde(rename = "send")]
    Send {
        peer_id: Option<String>,
        message: String,
    },
    #[serde(rename = "broadcast")]
    Broadcast { message: String },
    #[serde(rename = "peers")]
    Peers,
    #[serde(rename = "status")]
    Status,
}

/// API response
#[derive(Debug, Serialize)]
struct ApiResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ApiResponse {
    fn success(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(msg: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg),
        }
    }
}

/// Start API server for CLI
pub async fn start_api_server(node: Node, api_addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(&api_addr)
        .await
        .map_err(|e| MeshError::Io(e))?;

    info!("API server listening on {}", api_addr);

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                debug!("API client connected from {}", addr);
                let node_clone = node.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_api_client(stream, node_clone).await {
                        error!("Error handling API client: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept API connection: {}", e);
            }
        }
    }
}

/// Handle API client connection
async fn handle_api_client(mut stream: TcpStream, node: Node) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("API client disconnected");
                break;
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let response = match handle_request(trimmed, &node).await {
                    Ok(resp) => resp,
                    Err(e) => ApiResponse::error(format!("{}", e)),
                };

                let json = serde_json::to_string(&response).map_err(|e| {
                    MeshError::Protocol(format!("Failed to serialize response: {}", e))
                })?;

                writer
                    .write_all(json.as_bytes())
                    .await
                    .map_err(|e| MeshError::Io(e))?;
                writer
                    .write_all(b"\n")
                    .await
                    .map_err(|e| MeshError::Io(e))?;
            }
            Err(e) => {
                error!("Error reading from API client: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Handle API request
async fn handle_request(request: &str, node: &Node) -> Result<ApiResponse> {
    let req: ApiRequest = serde_json::from_str(request)
        .map_err(|e| MeshError::Protocol(format!("Invalid request: {}", e)))?;

    match req {
        ApiRequest::Send { peer_id, message } => {
            let data = message.into_bytes();
            match node.send_mesh_message(peer_id, data).await {
                Ok(message_id) => Ok(ApiResponse::success(serde_json::json!({
                    "message_id": message_id
                }))),
                Err(e) => Ok(ApiResponse::error(format!("{}", e))),
            }
        }
        ApiRequest::Broadcast { message } => {
            let data = message.into_bytes();
            match node.send_mesh_message(None, data).await {
                Ok(message_id) => Ok(ApiResponse::success(serde_json::json!({
                    "message_id": message_id
                }))),
                Err(e) => Ok(ApiResponse::error(format!("{}", e))),
            }
        }
        ApiRequest::Peers => {
            let peers = node.get_peers().await;
            let peers_json: Vec<_> = peers
                .into_iter()
                .map(|(id, addr, state)| {
                    serde_json::json!({
                        "id": id,
                        "address": addr.to_string(),
                        "state": format!("{:?}", state)
                    })
                })
                .collect();
            Ok(ApiResponse::success(serde_json::json!({
                "peers": peers_json
            })))
        }
        ApiRequest::Status => {
            let (node_id, connected, total) = node.get_status().await;
            Ok(ApiResponse::success(serde_json::json!({
                "node_id": node_id,
                "connected_peers": connected,
                "total_peers": total
            })))
        }
    }
}
