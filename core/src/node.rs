/// Main node implementation
use crate::config::Config;
use crate::error::{MeshError, Result};
use crate::p2p::peer::{ConnectionState, PeerManager};
use crate::p2p::protocol::{Frame, Message};
use crate::utils::event_emitter::EventEmitter;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::{interval, sleep, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Main P2P node
pub struct Node {
    /// Unique node identifier
    pub id: String,
    
    /// Node configuration
    config: Config,
    
    /// Peer manager
    peer_manager: PeerManager,
    
    /// Event emitter for visualization
    event_emitter: EventEmitter,
    
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl Node {
    /// Create a new node
    pub fn new(config: Config) -> Self {
        let id = Uuid::new_v4().to_string();
        let peer_manager = PeerManager::new(id.clone(), config.listen_addr.port());
        let event_emitter = EventEmitter::new(id.clone());
        
        info!("Created new node with ID: {}", id);
        
        Self {
            id,
            config,
            peer_manager,
            event_emitter,
            shutdown: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Start the node
    pub async fn start(&self) -> Result<()> {
        info!("Starting MeshLink node {}", self.id);
        info!("Listening on: {}", self.config.listen_addr);
        info!("Known peers: {:?}", self.config.known_peers);
        
        // Initialize known peers
        for peer_addr in &self.config.known_peers {
            if let Ok(addr) = peer_addr.parse::<SocketAddr>() {
                // Generate temporary ID, will be updated during handshake
                let temp_id = format!("peer-{}", addr);
                self.peer_manager.add_peer(temp_id, addr).await;
            }
        }
        
        self.event_emitter.emit("started", None).await;
        
        // Spawn all tasks
        let listener_handle = {
            let node = self.clone();
            tokio::spawn(async move { node.run_listener().await })
        };
        
        let connector_handle = {
            let node = self.clone();
            tokio::spawn(async move { node.run_connector().await })
        };
        
        let heartbeat_handle = {
            let node = self.clone();
            tokio::spawn(async move { node.run_heartbeat().await })
        };
        
        let keepalive_handle = {
            let node = self.clone();
            tokio::spawn(async move { node.run_keepalive().await })
        };
        
        // Wait for shutdown signal
        self.wait_for_shutdown().await;
        
        // Signal shutdown
        *self.shutdown.write().await = true;
        info!("Shutdown signal received, stopping node...");
        
        // Wait for tasks to complete
        let _ = tokio::join!(
            listener_handle,
            connector_handle,
            heartbeat_handle,
            keepalive_handle,
        );
        
        info!("Node stopped");
        Ok(())
    }
    
    /// Wait for shutdown signal (Ctrl+C)
    async fn wait_for_shutdown(&self) {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
            info!("Ctrl+C received");
        };
        
        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
            info!("SIGTERM received");
        };
        
        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();
        
        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }
    
    /// Run listener for incoming connections
    async fn run_listener(&self) -> Result<()> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| MeshError::Io(e))?;
        
        info!("Listening for incoming connections on {}", self.config.listen_addr);
        
        loop {
            if *self.shutdown.read().await {
                break;
            }
            
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            let node = self.clone();
                            tokio::spawn(async move {
                                if let Err(e) = node.handle_incoming_connection(stream, addr).await {
                                    error!("Error handling incoming connection from {}: {}", addr, e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Error accepting connection: {}", e);
                            sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
                _ = sleep(Duration::from_millis(100)) => {
                    // Check shutdown periodically
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle incoming connection
    async fn handle_incoming_connection(&self, mut stream: TcpStream, addr: SocketAddr) -> Result<()> {
        debug!("Incoming connection from {}", addr);
        self.event_emitter.emit("incoming_connection", Some(&addr.to_string())).await;
        
        // Perform handshake
        let (peer_id, protocol_version) = match self.peer_manager
            .perform_handshake(&mut stream, true)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Handshake failed with {}: {}", addr, e);
                return Err(e);
            }
        };
        
        info!("Handshake successful with {} (ID: {}, protocol: {})", addr, peer_id, protocol_version);
        
        // Update peer info
        self.peer_manager.add_peer(peer_id.clone(), addr).await;
        self.peer_manager.update_peer_state(&peer_id, ConnectionState::Connected).await;
        self.peer_manager.update_peer_last_seen(&peer_id).await;
        
        self.event_emitter.emit("connected", Some(&peer_id)).await;
        
        // Handle connection
        if let Err(e) = self.handle_connection(stream, peer_id.clone(), addr).await {
            error!("Error in connection with {}: {}", peer_id, e);
        }
        
        // Cleanup
        self.peer_manager.update_peer_state(&peer_id, ConnectionState::Disconnected).await;
        self.event_emitter.emit("disconnected", Some(&peer_id)).await;
        
        Ok(())
    }
    
    /// Run connector to establish outbound connections
    async fn run_connector(&self) -> Result<()> {
        // Initial delay to let listener start
        sleep(Duration::from_secs(1)).await;
        
        let mut retry_interval = interval(self.config.retry_interval);
        retry_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            if *self.shutdown.read().await {
                break;
            }
            
            retry_interval.tick().await;
            
            let peers = self.peer_manager.get_all_peers().await;
            let connected = self.peer_manager.get_connected_peers().await;
            let connected_ids: std::collections::HashSet<_> = connected.iter()
                .map(|p| p.node_id.clone())
                .collect();
            
            debug!("Connector: checking {} peers, {} connected", peers.len(), connected.len());
            
            for peer in peers {
                // Skip if already connected
                if connected_ids.contains(&peer.node_id) {
                    continue;
                }
                
                // Skip if trying to connect to ourselves
                if peer.address.port() == self.config.listen_addr.port() {
                    continue;
                }
                
                // Skip if too many attempts
                if peer.connection_attempts >= self.config.max_connection_attempts {
                    continue;
                }
                
                let node = self.clone();
                let peer_addr = peer.address;
                let peer_id = peer.node_id.clone();
                
                tokio::spawn(async move {
                    node.connect_to_peer(peer_id, peer_addr).await;
                });
            }
        }
        
        Ok(())
    }
    
    /// Connect to a specific peer
    async fn connect_to_peer(&self, peer_id: String, addr: SocketAddr) {
        self.peer_manager.update_peer_state(&peer_id, ConnectionState::Connecting).await;
        self.peer_manager.increment_connection_attempts(&peer_id).await;
        
        match timeout(self.config.connection_timeout, TcpStream::connect(&addr)).await {
            Ok(Ok(mut stream)) => {
                debug!("TCP connection established to {}", addr);
                
                // Perform handshake
                match self.peer_manager.perform_handshake(&mut stream, false).await {
                    Ok((actual_peer_id, protocol_version)) => {
                        info!("Connected to peer {} (ID: {}, protocol: {})", addr, actual_peer_id, protocol_version);
                        
                        // Update peer info with actual ID from handshake
                        self.peer_manager.add_peer(actual_peer_id.clone(), addr).await;
                        self.peer_manager.update_peer_state(&actual_peer_id, ConnectionState::Connected).await;
                        self.peer_manager.update_peer_last_seen(&actual_peer_id).await;
                        
                        self.event_emitter.emit("connected", Some(&actual_peer_id)).await;
                        
                        // Handle connection
                        if let Err(e) = self.handle_connection(stream, actual_peer_id.clone(), addr).await {
                            error!("Error in connection with {}: {}", actual_peer_id, e);
                        }
                        
                        // Cleanup
                        self.peer_manager.update_peer_state(&actual_peer_id, ConnectionState::Disconnected).await;
                        self.event_emitter.emit("disconnected", Some(&actual_peer_id)).await;
                    }
                    Err(e) => {
                        error!("Handshake failed with {}: {}", addr, e);
                        self.peer_manager.update_peer_state(&peer_id, ConnectionState::Disconnected).await;
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to connect to {}: {}", addr, e);
                self.peer_manager.update_peer_state(&peer_id, ConnectionState::Disconnected).await;
            }
            Err(_) => {
                warn!("Connection timeout to {}", addr);
                self.peer_manager.update_peer_state(&peer_id, ConnectionState::Disconnected).await;
            }
        }
    }
    
    /// Handle established connection
    async fn handle_connection(
        &self,
        mut stream: TcpStream,
        peer_id: String,
        _addr: SocketAddr,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let mut len_buf = [0u8; 4];
        
        loop {
            if *self.shutdown.read().await {
                break;
            }
            
            // Read frame length
            match timeout(Duration::from_secs(30), stream.read_exact(&mut len_buf)).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    debug!("Connection closed by peer: {}", peer_id);
                    break;
                }
                Ok(Err(e)) => {
                    return Err(MeshError::Io(e));
                }
                Err(_) => {
                    // Timeout - send ping
                    let ping = Message::Ping {
                        timestamp: chrono::Utc::now().timestamp(),
                    };
                    let frame = Frame::from_message(&ping)
                        .map_err(|e| MeshError::Protocol(format!("Failed to serialize ping: {}", e)))?;
                    stream.write_all(&frame.to_bytes())
                        .await
                        .map_err(|e| MeshError::Io(e))?;
                    continue;
                }
            }
            
            let length = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; length];
            
            timeout(Duration::from_secs(30), stream.read_exact(&mut payload))
                .await
                .map_err(|_| MeshError::Timeout("Read timeout".to_string()))?
                .map_err(|e| MeshError::Io(e))?;
            
            // Parse message
            let message = Message::from_bytes(&payload)
                .map_err(|e| MeshError::Protocol(format!("Invalid message: {}", e)))?;
            
            // Update last seen
            self.peer_manager.update_peer_last_seen(&peer_id).await;
            
            // Handle message
            match message {
                Message::Ping { timestamp } => {
                    debug!("Received ping from {}", peer_id);
                    let pong = Message::Pong { timestamp };
                    let frame = Frame::from_message(&pong)
                        .map_err(|e| MeshError::Protocol(format!("Failed to serialize pong: {}", e)))?;
                    stream.write_all(&frame.to_bytes())
                        .await
                        .map_err(|e| MeshError::Io(e))?;
                }
                Message::Pong { .. } => {
                    debug!("Received pong from {}", peer_id);
                }
                Message::Data { payload, message_id } => {
                    info!("Received data message {} from {}: {} bytes", message_id, peer_id, payload.len());
                    self.event_emitter.emit("message_received", Some(&peer_id)).await;
                }
                Message::Close { reason } => {
                    info!("Peer {} closed connection: {}", peer_id, reason);
                    break;
                }
                _ => {
                    debug!("Received unhandled message type from {}: {}", peer_id, message.message_type());
                }
            }
        }
        
        Ok(())
    }
    
    /// Run heartbeat task
    async fn run_heartbeat(&self) -> Result<()> {
        let mut interval = interval(self.config.heartbeat_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            if *self.shutdown.read().await {
                break;
            }
            
            interval.tick().await;
            
            let connected = self.peer_manager.get_connected_peers().await;
            let all = self.peer_manager.get_all_peers().await;
            
            info!("Heartbeat - Connected: {}/{} peers", connected.len(), all.len());
            self.event_emitter.emit("heartbeat", None).await;
            
            // Remove stale peers
            let removed = self.peer_manager.remove_stale_peers(self.config.peer_stale_timeout).await;
            if removed > 0 {
                info!("Removed {} stale peers", removed);
            }
        }
        
        Ok(())
    }
    
    /// Run keepalive task (send pings to connected peers)
    async fn run_keepalive(&self) -> Result<()> {
        let mut interval = interval(self.config.keepalive_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            if *self.shutdown.read().await {
                break;
            }
            
            interval.tick().await;
            
            // Keepalive is handled in handle_connection via timeout
            // This task can be extended for additional keepalive logic
        }
        
        Ok(())
    }
}

impl Clone for Node {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            peer_manager: self.peer_manager.clone(),
            event_emitter: self.event_emitter.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

