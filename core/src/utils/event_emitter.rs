/// Event emitter for visualization and monitoring
use tokio::net::UdpSocket;

pub struct EventEmitter {
    node_id: String,
}

impl EventEmitter {
    pub fn new(node_id: String) -> Self {
        Self { node_id }
    }
    
    pub async fn emit(&self, event: &str, peer: Option<&str>) {
        let payload = serde_json::json!({
            "node": self.node_id,
            "event": event,
            "peer": peer,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })
        .to_string();

        if let Ok(sock) = UdpSocket::bind("127.0.0.1:0").await {
            let _ = sock.send_to(payload.as_bytes(), "127.0.0.1:9999").await;
        }
    }
}

impl Clone for EventEmitter {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
        }
    }
}

