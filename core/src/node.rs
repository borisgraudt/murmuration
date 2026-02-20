use crate::ai::router::{MeshMessage, Router};
use crate::ai::routing_logger::{
    MessageContext, PeerMetricsSnapshot, PeerSelection, RoutingLogEntry, RoutingLogger,
};
/// Main node implementation
use crate::config::Config;
use crate::content_store::ContentStore;
use crate::elysium::packet::ElysiumPacket;
use crate::error::{MeshError, Result};
use crate::identity;
use crate::message_store::MessageStore;
use crate::naming::NameRegistry;
use crate::p2p::discovery::DiscoveryManager;
use crate::p2p::encryption::{EncryptionManager, SessionKeyManager};
use crate::p2p::peer::{ConnectionState, PeerManager};
use crate::p2p::protocol::{Frame, Message};
use crate::peer_store;
use crate::utils::event_emitter::EventEmitter;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;
use tokio::sync::{mpsc, oneshot, Notify, RwLock, Semaphore};
use tokio::time::{interval, sleep, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Type alias for ping waiter (sent_at, responder)
type PingWaiter = (Instant, oneshot::Sender<Duration>);

const INBOX_MAX_MESSAGES: usize = 500;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct InboxMessage {
    pub seq: u64,
    pub timestamp: String,
    pub direction: String, // "in" | "out"
    pub kind: String,      // "data" | "mesh"
    pub peer: String,      // peer_id for transport (or best-effort)
    pub from: String,
    pub to: Option<String>,
    pub message_id: Option<String>,
    pub bytes: usize,
    pub preview: String,
    // --- Messenger extensions (serde(default) for backward compat with old sled records) ---
    /// "dm:{min_id}:{max_id}" for DMs, "broadcast" for broadcasts
    #[serde(default)]
    pub conversation_id: String,
    /// Full decoded message text (not truncated); None for binary payloads
    #[serde(default)]
    pub content: Option<String>,
    /// True once the recipient sent a MessageAck back to us
    #[serde(default)]
    pub delivered: bool,
}

/// Compute a stable, canonical conversation ID from sender and optional recipient.
/// Both sides of a DM will always compute the same ID.
pub fn compute_conversation_id(from: &str, to: Option<&str>) -> String {
    match to {
        None => "broadcast".to_string(),
        Some(peer) => {
            if from <= peer {
                format!("dm:{}:{}", from, peer)
            } else {
                format!("dm:{}:{}", peer, from)
            }
        }
    }
}

#[derive(Clone)]
struct PeerChannel {
    token: Uuid,
    tx: mpsc::UnboundedSender<Message>,
}

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

    /// Encryption manager
    encryption_manager: EncryptionManager,

    /// Session key manager
    session_keys: SessionKeyManager,

    /// Router for mesh messages
    router: Router,

    /// Routing logger for AI training data
    routing_logger: RoutingLogger,

    /// Content store for mesh sites
    content_store: ContentStore,

    /// Name registry (ely://name resolution)
    name_registry: NameRegistry,

    /// Message store (persistent history)
    message_store: MessageStore,

    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
    /// Programmatic shutdown notifier (used by tests and embedding)
    shutdown_notify: Arc<Notify>,

    /// Message senders for each peer connection
    message_senders: Arc<RwLock<HashMap<String, PeerChannel>>>,

    /// Pending pings for latency measurement (peer_id -> timestamp when ping was sent)
    pending_pings: Arc<RwLock<HashMap<String, Instant>>>,

    /// Manual ping waiters (peer_id -> (sent_at, responder))
    pending_manual_pings: Arc<RwLock<HashMap<String, PingWaiter>>>,

    /// Heartbeat missed pongs counter (peer_id -> consecutive missed count)
    missed_pongs: Arc<RwLock<HashMap<String, u32>>>,

    /// Pending content requests (request_id -> response channel)
    pending_content_requests: Arc<RwLock<HashMap<String, oneshot::Sender<Vec<u8>>>>>,

    /// Actual API address (may differ from default if port was taken and we had to fall back)
    api_addr: Arc<RwLock<SocketAddr>>,

    /// Discovery port used for UDP broadcast/listen
    discovery_port: u16,

    /// Connection storm protection: limit concurrent connect attempts
    connect_semaphore: Arc<Semaphore>,

    /// Connection storm protection: per-address cooldown
    last_connect_attempt: Arc<RwLock<HashMap<SocketAddr, Instant>>>,

    /// In-memory inbox for user-visible messages (used by CLI/watch)
    inbox: Arc<RwLock<VecDeque<InboxMessage>>>,
    inbox_next_seq: Arc<RwLock<u64>>,
    inbox_notify: Arc<Notify>,

    /// Contact book (persisted in sled)
    contact_store: crate::contact_store::ContactStore,

    /// Broadcast channel for Messenger SSE events (new messages, peer events, delivery acks)
    ws_event_tx: Arc<tokio::sync::broadcast::Sender<crate::messenger_types::MessengerEvent>>,
}

impl Node {
    /// Create a new node
    pub fn new(mut config: Config) -> Result<Self> {
        // Ensure each node has a unique data directory based on port
        let data_dir = config.data_dir.clone().unwrap_or_else(|| {
            std::path::PathBuf::from(format!(".ely/node-{}", config.listen_addr.port()))
        });
        
        // Warn if data_dir is explicitly set and might conflict with another node
        if config.data_dir.is_some() {
            warn!(
                "Using explicit data directory: {}. Make sure each node uses a unique directory!",
                data_dir.display()
            );
        }
        
        config.data_dir = Some(data_dir.clone());

        let ident = identity::load_or_create(&data_dir)?;
        let id = ident.node_id;
        
        // Log data directory for debugging
        info!("Node data directory: {}", data_dir.display());

        let peer_manager = PeerManager::new(id.clone(), config.listen_addr.port());
        let event_emitter = EventEmitter::new(id.clone());
        let encryption_manager = ident.encryption;
        let session_keys = SessionKeyManager::new();
        let router = Router::new(id.clone());
        let routing_logger = RoutingLogger::new();
        let content_store = ContentStore::new(&data_dir)?;
        let name_registry = NameRegistry::with_storage(&data_dir)?;
        let message_store = MessageStore::new(&data_dir)?;
        let contact_store = crate::contact_store::ContactStore::new(&data_dir)?;
        let (ws_event_tx, _) = tokio::sync::broadcast::channel::<crate::messenger_types::MessengerEvent>(256);
        let ws_event_tx = Arc::new(ws_event_tx);

        info!("Created new node with ID: {}", id);

        let default_api_port = 9000u16.checked_add(config.listen_addr.port()).unwrap_or(0);
        let initial_api_addr: SocketAddr = config.api_addr.unwrap_or_else(|| {
            format!("127.0.0.1:{}", default_api_port)
                .parse()
                .unwrap_or_else(|e| {
                    panic!("Invalid default API address {}: {}", default_api_port, e);
                })
        });
        let discovery_port = config.discovery_port;
        let max_connect_in_flight = config.max_connect_in_flight.max(1);

        Ok(Self {
            id,
            config,
            peer_manager,
            event_emitter,
            encryption_manager,
            session_keys,
            router,
            routing_logger,
            content_store,
            name_registry,
            message_store,
            shutdown: Arc::new(RwLock::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
            message_senders: Arc::new(RwLock::new(HashMap::new())),
            pending_pings: Arc::new(RwLock::new(HashMap::new())),
            pending_manual_pings: Arc::new(RwLock::new(HashMap::new())),
            missed_pongs: Arc::new(RwLock::new(HashMap::new())),
            pending_content_requests: Arc::new(RwLock::new(HashMap::new())),
            api_addr: Arc::new(RwLock::new(initial_api_addr)),
            discovery_port,
            connect_semaphore: Arc::new(Semaphore::new(max_connect_in_flight)),
            last_connect_attempt: Arc::new(RwLock::new(HashMap::new())),
            inbox: Arc::new(RwLock::new(VecDeque::new())),
            inbox_next_seq: Arc::new(RwLock::new(1)),
            inbox_notify: Arc::new(Notify::new()),
            contact_store,
            ws_event_tx,
        })
    }

    pub async fn get_api_addr(&self) -> SocketAddr {
        *self.api_addr.read().await
    }

    async fn push_inbox(&self, mut msg: InboxMessage) {
        // assign seq
        let seq = {
            let mut next = self.inbox_next_seq.write().await;
            let seq = *next;
            *next = next.saturating_add(1);
            seq
        };
        msg.seq = seq;

        let mut inbox = self.inbox.write().await;
        inbox.push_back(msg.clone());
        while inbox.len() > INBOX_MAX_MESSAGES {
            inbox.pop_front();
        }
        drop(inbox);

        // Persist to DB (best-effort)
        let _ = self.message_store.save(&msg);

        self.inbox_notify.notify_waiters();

        // Broadcast to Messenger SSE subscribers (best-effort: ignore if no subscribers)
        let _ = self.ws_event_tx.send(crate::messenger_types::MessengerEvent::NewMessage {
            message: msg,
        });
    }

    pub async fn list_inbox(&self, since: Option<u64>, limit: usize) -> (u64, Vec<InboxMessage>) {
        let since = since.unwrap_or(0);
        let limit = limit.clamp(1, INBOX_MAX_MESSAGES);

        let inbox = self.inbox.read().await;
        let mut out: Vec<InboxMessage> = inbox.iter().filter(|m| m.seq > since).cloned().collect();
        // keep only last `limit`
        if out.len() > limit {
            out = out.split_off(out.len() - limit);
        }
        let next_since = inbox.back().map(|m| m.seq).unwrap_or(since);
        (next_since, out)
    }

    pub async fn watch_inbox(
        &self,
        since: u64,
        timeout_dur: Duration,
        limit: usize,
    ) -> (u64, Vec<InboxMessage>) {
        // Fast path: already have newer messages
        let (next, msgs) = self.list_inbox(Some(since), limit).await;
        if !msgs.is_empty() {
            return (next, msgs);
        }

        // Wait for new messages or timeout
        let notified = self.inbox_notify.notified();
        let _ = tokio::time::timeout(timeout_dur, notified).await;
        self.list_inbox(Some(since), limit).await
    }

    pub fn get_discovery_port(&self) -> u16 {
        self.discovery_port
    }

    /// Export inbox as bundle (for offline transfer)
    pub async fn export_bundle(&self, limit: usize) -> Result<crate::bundle::MessageBundle> {
        let (_, messages) = self.list_inbox(None, limit).await;
        let bundle = crate::bundle::MessageBundle::new(messages, 7); // 7 days TTL
        Ok(bundle)
    }

    /// Import bundle and deliver messages
    pub async fn import_bundle(
        &self,
        bundle: crate::bundle::MessageBundle,
    ) -> Result<(usize, usize)> {
        let mut delivered = 0;
        let forwarded = 0;

        for msg in bundle.messages {
            // Simple: just push to inbox (future: check if for us, forward if not)
            self.push_inbox(msg).await;
            delivered += 1;
        }

        Ok((delivered, forwarded))
    }

    /// Register a human-readable name
    pub async fn register_name(&self, name: String, node_id: String) -> Result<()> {
        self.name_registry.register(name, node_id).await
    }

    /// Resolve name to node_id
    pub async fn resolve_name(&self, name: &str) -> Option<String> {
        self.name_registry.resolve(name).await
    }

    /// List all registered names
    pub async fn list_names(&self) -> Vec<crate::naming::NameRecord> {
        self.name_registry.list().await
    }

    // ─── Messenger API ────────────────────────────────────────────────────────

    /// Return one ConversationSummary per unique conversation (last message per thread)
    pub async fn get_conversations(&self) -> Vec<crate::messenger_types::ConversationSummary> {
        let inbox = self.inbox.read().await;
        let mut map: std::collections::HashMap<String, InboxMessage> = std::collections::HashMap::new();
        for msg in inbox.iter() {
            let key = msg.conversation_id.clone();
            let entry = map.entry(key).or_insert_with(|| msg.clone());
            if msg.seq > entry.seq {
                *entry = msg.clone();
            }
        }
        let mut result: Vec<crate::messenger_types::ConversationSummary> = map
            .into_values()
            .map(|msg| {
                let peer_id = if msg.direction == "out" {
                    msg.to.clone().unwrap_or_default()
                } else {
                    msg.from.clone()
                };
                crate::messenger_types::ConversationSummary {
                    conversation_id: msg.conversation_id.clone(),
                    peer_id,
                    last_preview: msg.preview.clone(),
                    last_timestamp: msg.timestamp.clone(),
                    last_seq: msg.seq,
                }
            })
            .collect();
        result.sort_by(|a, b| b.last_seq.cmp(&a.last_seq));
        result
    }

    /// Return paginated message history for a specific DM conversation
    pub async fn get_conversation_history(
        &self,
        peer_id: &str,
        since: Option<u64>,
        limit: usize,
    ) -> (u64, Vec<InboxMessage>) {
        let target = compute_conversation_id(&self.id, Some(peer_id));
        let since = since.unwrap_or(0);
        let limit = limit.clamp(1, INBOX_MAX_MESSAGES);
        let inbox = self.inbox.read().await;
        let mut msgs: Vec<InboxMessage> = inbox
            .iter()
            .filter(|m| m.conversation_id == target && m.seq > since)
            .cloned()
            .collect();
        if msgs.len() > limit {
            msgs = msgs.split_off(msgs.len() - limit);
        }
        let next = msgs.last().map(|m| m.seq).unwrap_or(since);
        (next, msgs)
    }

    /// Mark an outgoing message as delivered (called when MessageAck is received)
    pub async fn mark_message_delivered(&self, message_id: &str) {
        {
            let mut inbox = self.inbox.write().await;
            for msg in inbox.iter_mut() {
                if msg.message_id.as_deref() == Some(message_id) {
                    msg.delivered = true;
                }
            }
        }
        let _ = self.message_store.update_delivered(message_id);
        let _ = self.ws_event_tx.send(crate::messenger_types::MessengerEvent::MessageDelivered {
            message_id: message_id.to_string(),
        });
    }

    /// Publish own profile to the mesh content store
    pub async fn publish_profile(&self, display_name: String, bio: String) -> Result<()> {
        let profile = serde_json::json!({
            "node_id": self.id,
            "display_name": display_name,
            "bio": bio,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });
        let url = format!("ely://{}/messenger/profile", self.id);
        self.content_store.put(
            &url,
            serde_json::to_vec(&profile).map_err(MeshError::Serialization)?,
        )
    }

    /// Fetch another node's profile from the mesh (with timeout)
    pub async fn fetch_profile(
        &self,
        node_id: &str,
        timeout_dur: Duration,
    ) -> Result<Option<serde_json::Value>> {
        let url = format!("ely://{}/messenger/profile", node_id);
        match self.fetch_content(&url, timeout_dur).await? {
            Some(bytes) => {
                let v = serde_json::from_slice(&bytes).map_err(MeshError::Serialization)?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }

    /// Add or update a contact
    pub async fn add_contact(&self, c: crate::contact_store::Contact) -> Result<()> {
        self.contact_store.add_contact(&c)
    }

    /// List all contacts
    pub async fn get_contacts(&self) -> Result<Vec<crate::contact_store::Contact>> {
        self.contact_store.get_contacts()
    }

    /// Remove a contact by node_id; returns true if it existed
    pub async fn remove_contact(&self, node_id: &str) -> Result<bool> {
        self.contact_store.remove_contact(node_id)
    }

    /// Lookup a single contact
    pub async fn get_contact(&self, node_id: &str) -> Result<Option<crate::contact_store::Contact>> {
        self.contact_store.get_contact(node_id)
    }

    /// Clone the broadcast sender so messenger_api can subscribe
    pub fn ws_event_sender(
        &self,
    ) -> tokio::sync::broadcast::Sender<crate::messenger_types::MessengerEvent> {
        (*self.ws_event_tx).clone()
    }

    // ─── End Messenger API ────────────────────────────────────────────────────

    /// Start the node
    pub async fn start(&self) -> Result<()> {
        info!("Starting MeshLink node {}", self.id);
        info!("Listening on: {}", self.config.listen_addr);
        info!("Known peers: {:?}", self.config.known_peers);
        info!(
            "Discovery: {} (port {})",
            if self.config.enable_discovery {
                "enabled"
            } else {
                "disabled"
            },
            self.discovery_port
        );

        // Initialize known peers
        for peer_addr in &self.config.known_peers {
            if let Ok(addr) = peer_addr.parse::<SocketAddr>() {
                // Generate temporary ID, will be updated during handshake
                let temp_id = format!("peer-{}", addr);
                self.peer_manager.add_peer(temp_id, addr).await;
            }
        }

        // Bootstrap from cached discovery peers (user-friendly: works even if you don't pass peer args)
        if let Some(dir) = self.config.data_dir.as_ref() {
            match peer_store::load_cached_peers(dir) {
                Ok(addrs) => {
                    if !addrs.is_empty() {
                        info!(
                            "Bootstrap cache: loaded {} peer(s) from {}",
                            addrs.len(),
                            dir.display()
                        );
                    }
                    for addr in addrs {
                        let temp_id = format!("peer-{}", addr);
                        self.peer_manager.add_peer(temp_id, addr).await;
                    }
                }
                Err(e) => {
                    debug!("Bootstrap cache: failed to load peers: {}", e);
                }
            }
        }

        self.event_emitter.emit("started", None).await;

        // Initialize routing logger (logs to logs/ai_routing_logs.jsonl)
        self.routing_logger.init(None).await;
        info!("AI routing logs will be saved to: logs/ai_routing_logs.jsonl");

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

        // Start discovery
        let discovery_handle = {
            let node = self.clone();
            tokio::spawn(async move { node.run_discovery().await })
        };

        // Start API server for CLI (configurable; auto-falls-back if port is taken)
        let api_handle = {
            let node = self.clone();
            tokio::spawn(async move {
                let preferred = node.get_api_addr().await;
                let listener = match Node::bind_tcp_with_fallback(preferred, 20).await {
                    Ok((listener, actual)) => {
                        *node.api_addr.write().await = actual;
                        listener
                    }
                    Err(e) => {
                        error!("API server failed to bind: {}", e);
                        return;
                    }
                };

                if let Err(e) = crate::api::start_api_server_with_listener(node, listener).await {
                    error!("API server error: {}", e);
                }
            })
        };

        // Start Web Gateway for browser viewing (HTTP server on API port + 1)
        let web_gateway_handle = {
            let node = Arc::new(self.clone());
            tokio::spawn(async move {
                // Wait a bit for API server to bind and set api_addr
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

                // Determine gateway port: use config if set, otherwise API port + 1
                let web_port = if let Some(port) = node.config.gateway_port {
                    port
                } else {
                    let mut api_port = node.get_api_addr().await.port();

                    // If API port is still 0 or uninitialized, use default
                    if api_port == 0 {
                        api_port = 17080; // Default fallback
                        warn!(
                            "Web Gateway: API port not yet initialized, using default {}",
                            api_port
                        );
                    }

                    api_port + 1
                };

                info!("Starting Web Gateway on port {}", web_port);

                if let Err(e) = crate::web_gateway::start_web_gateway(node, web_port).await {
                    error!("Web Gateway error: {}", e);
                }
            })
        };

        // Start Messenger API (REST + SSE) on API port + 2
        let messenger_handle = {
            let node = self.clone();
            tokio::spawn(async move {
                // Wait for API server to bind and update api_addr
                tokio::time::sleep(tokio::time::Duration::from_millis(600)).await;

                let api_port = node.get_api_addr().await.port();
                let messenger_port = api_port.saturating_add(2);

                info!("Starting Messenger API on port {}", messenger_port);

                if let Err(e) = crate::messenger_api::start_messenger_api(node, messenger_port).await {
                    error!("Messenger API error: {}", e);
                }
            })
        };

        // Wait for shutdown signal
        self.wait_for_shutdown().await;

        // Signal shutdown
        *self.shutdown.write().await = true;
        info!("Shutdown signal received, stopping node...");

        // Give tasks a moment to see the shutdown flag
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Abort all tasks to ensure clean shutdown
        listener_handle.abort();
        connector_handle.abort();
        heartbeat_handle.abort();
        keepalive_handle.abort();
        discovery_handle.abort();
        api_handle.abort();
        web_gateway_handle.abort();
        messenger_handle.abort();

        // Wait a bit for tasks to clean up
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        info!("Node stopped");
        Ok(())
    }

    /// Request a graceful shutdown (primarily for tests and embedding).
    pub fn request_shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }

    /// Ping a connected peer and return RTT.
    pub async fn ping_peer(&self, peer_id: &str, timeout_dur: Duration) -> Result<Duration> {
        // Peer state can flip to Connected slightly before the message channel is inserted.
        // Also, we can transiently have a mismatch between PeerManager IDs and active channel IDs
        // (e.g. during simultaneous connect). Resolve via direct ID or by matching peer address.
        let max_wait = timeout_dur.min(Duration::from_secs(5));
        let sender_wait_deadline = Instant::now() + max_wait;
        let sender = loop {
            if let Some(sender) = self.resolve_sender_for_peer(peer_id).await {
                break sender;
            }
            if Instant::now() >= sender_wait_deadline {
                return Err(MeshError::Peer(format!("Peer not connected: {}", peer_id)));
            }
            sleep(Duration::from_millis(25)).await;
        };

        let (tx, rx) = oneshot::channel::<Duration>();
        {
            let mut pending = self.pending_manual_pings.write().await;
            if pending.contains_key(peer_id) {
                return Err(MeshError::Peer(format!(
                    "Ping already in progress for peer: {}",
                    peer_id
                )));
            }
            pending.insert(peer_id.to_string(), (Instant::now(), tx));
        }

        let ping = Message::Ping {
            timestamp: chrono::Utc::now().timestamp(),
        };

        if let Err(_e) = sender.send(ping) {
            let mut pending = self.pending_manual_pings.write().await;
            pending.remove(peer_id);
            return Err(MeshError::Peer(format!(
                "Failed to send ping to {}: channel closed",
                peer_id
            )));
        }

        match timeout(timeout_dur, rx).await {
            Ok(Ok(latency)) => Ok(latency),
            Ok(Err(_canceled)) => Err(MeshError::Peer(format!(
                "Ping canceled (peer disconnected): {}",
                peer_id
            ))),
            Err(_elapsed) => {
                let mut pending = self.pending_manual_pings.write().await;
                pending.remove(peer_id);
                Err(MeshError::Timeout(format!(
                    "Ping timeout after {:?} to peer {}",
                    timeout_dur, peer_id
                )))
            }
        }
    }

    async fn resolve_sender_for_peer(
        &self,
        peer_id: &str,
    ) -> Option<mpsc::UnboundedSender<Message>> {
        // 1) Direct match by peer_id
        if let Some(ch) = self.message_senders.read().await.get(peer_id) {
            return Some(ch.tx.clone());
        }

        // 2) Address-based match (peer_id known in PeerManager but channel keyed differently)
        let target_addr = self
            .peer_manager
            .get_peer(peer_id)
            .await
            .map(|p| p.address)?;

        // Snapshot sender IDs to avoid holding the lock across awaits
        let sender_ids: Vec<String> = {
            let senders = self.message_senders.read().await;
            senders.keys().cloned().collect()
        };

        if sender_ids.is_empty() {
            return None;
        }

        // Find any peer entry that maps to the same address and has an active sender
        let peers = self.peer_manager.get_all_peers().await;
        let channel_peer_id = peers
            .into_iter()
            .find(|p| p.address == target_addr && sender_ids.contains(&p.node_id))
            .map(|p| p.node_id)?;

        self.message_senders
            .read()
            .await
            .get(&channel_peer_id)
            .map(|ch| ch.tx.clone())
    }

    /// Publish content to local content store
    /// Path format: ely://<node_id>/<path> or just <path> (node_id is prepended)
    pub async fn publish_content(&self, path: &str, content: Vec<u8>) -> Result<String> {
        // Normalize path: ensure it starts with ely://<our_node_id>/
        let full_path = if path.starts_with("ely://") {
            path.to_string()
        } else {
            let clean_path = path.trim_start_matches('/');
            format!("ely://{}/{}", self.id, clean_path)
        };

        info!(
            "Publishing content: {} ({} bytes)",
            full_path,
            content.len()
        );
        self.content_store.put(&full_path, content)?;

        Ok(full_path)
    }

    /// Fetch content from mesh (local or remote)
    /// If URL is local (our node_id), fetch from local store
    /// Otherwise, send ContentRequest to network and wait for response
    pub async fn fetch_content(&self, url: &str, timeout_dur: Duration) -> Result<Option<Vec<u8>>> {
        // Parse URL: ely://<node_id>/<path>
        if !url.starts_with("ely://") {
            return Err(MeshError::Protocol(format!("Invalid content URL: {}", url)));
        }

        let url_parts: Vec<&str> = url.trim_start_matches("ely://").splitn(2, '/').collect();
        if url_parts.len() < 2 {
            return Err(MeshError::Protocol(format!(
                "Invalid content URL format: {}",
                url
            )));
        }

        let target_node_id = url_parts[0];

        // Local fetch (fast path)
        if target_node_id == self.id {
            debug!("Fetching local content: {}", url);
            return self.content_store.get(url);
        }

        // Remote fetch: send ContentRequest and wait for response
        info!(
            "Fetching remote content: {} from node {}",
            url, target_node_id
        );

        let request_id = Uuid::new_v4().to_string();
        let request = Message::ContentRequest {
            request_id: request_id.clone(),
            url: url.to_string(),
            from_node: self.id.clone(),
            ttl: 8, // Max 8 hops
            path: vec![self.id.clone()],
        };

        // Create response channel
        let (tx, rx) = oneshot::channel::<Vec<u8>>();

        // Store pending request
        {
            let mut pending = self.pending_content_requests.write().await;
            pending.insert(request_id.clone(), tx);
        }

        // Send request via mesh routing (broadcast or directed if we know a route to target)
        let request_bytes = request.to_bytes().map_err(MeshError::Serialization)?;
        let data_msg = Message::Data {
            payload: request_bytes,
            message_id: request_id.clone(),
        };

        // Broadcast to all connected peers (routing will handle forwarding)
        let senders = self.message_senders.read().await;
        for (peer_id, ch) in senders.iter() {
            if let Err(e) = ch.tx.send(data_msg.clone()) {
                warn!("Failed to send content request to {}: {}", peer_id, e);
            }
        }
        drop(senders);

        // Wait for response (with timeout)
        match timeout(timeout_dur, rx).await {
            Ok(Ok(content)) => Ok(Some(content)),
            Ok(Err(_)) => Err(MeshError::Protocol("Content request canceled".to_string())),
            Err(_) => {
                // Cleanup on timeout
                let mut pending = self.pending_content_requests.write().await;
                pending.remove(&request_id);
                Err(MeshError::Timeout(format!(
                    "Content fetch timeout after {:?}",
                    timeout_dur
                )))
            }
        }
    }

    /// Wait for shutdown signal (Ctrl+C)
    async fn wait_for_shutdown(&self) {
        let requested = async {
            self.shutdown_notify.notified().await;
            info!("Shutdown requested");
        };

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
            _ = requested => {},
        }
    }

    /// Run listener for incoming connections
    async fn run_listener(&self) -> Result<()> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(MeshError::Io)?;

        info!(
            "Listening for incoming connections on {}",
            self.config.listen_addr
        );

        loop {
            if *self.shutdown.read().await {
                break;
            }

            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, addr)) => {
                            info!("Accepted TCP connection from {}", addr);
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
    async fn handle_incoming_connection(
        &self,
        mut stream: TcpStream,
        addr: SocketAddr,
    ) -> Result<()> {
        info!(
            "Incoming connection from {} (source port: {})",
            addr,
            addr.port()
        );

        // Check if we're already connecting to this address - if so, reject incoming
        // Note: addr is the source address, which may be different from the peer's listen address
        let all_peers = self.peer_manager.get_all_peers().await;
        debug!(
            "Checking {} known peers for address {}",
            all_peers.len(),
            addr
        );
        if let Some(existing_peer) = all_peers.iter().find(|p| p.address == addr) {
            if existing_peer.state == ConnectionState::Connecting
                || existing_peer.state == ConnectionState::Handshaking
            {
                info!("Rejecting incoming connection from {} - we're already connecting to it (state: {:?})", addr, existing_peer.state);
                // Close stream gracefully before dropping
                use tokio::io::AsyncWriteExt;
                let _ = stream.shutdown().await;
                drop(stream);
                return Ok(()); // Silently close - we'll use our outgoing connection
            }
            if existing_peer.state == ConnectionState::Connected {
                info!(
                    "Rejecting incoming connection from {} - already connected",
                    addr
                );
                // Close stream gracefully before dropping
                use tokio::io::AsyncWriteExt;
                let _ = stream.shutdown().await;
                drop(stream);
                return Ok(()); // Already connected
            }
            debug!(
                "Found existing peer {} with state {:?}, accepting connection",
                existing_peer.node_id, existing_peer.state
            );
        } else {
            debug!(
                "No existing peer found for address {}, accepting new connection",
                addr
            );
        }

        info!(
            "Accepting incoming connection from {} (proceeding with handshake)",
            addr
        );

        // Set state to Handshaking before starting handshake
        // We need to find the peer by address first
        let temp_id = format!("peer-{}", addr);
        self.peer_manager.add_peer(temp_id.clone(), addr).await;
        self.peer_manager
            .update_peer_state(&temp_id, ConnectionState::Handshaking)
            .await;

        self.event_emitter
            .emit("incoming_connection", Some(&addr.to_string()))
            .await;

        // Perform handshake
        debug!("Starting handshake with {} (incoming)", addr);
        let (peer_id, protocol_version, _peer_public_key) = match self
            .peer_manager
            .perform_handshake(
                &mut stream,
                false, // false = incoming connection
                Some(&self.encryption_manager),
                Some(&self.session_keys),
            )
            .await
        {
            Ok(result) => result,
            Err(e) => {
                error!("Handshake failed with {}: {}", addr, e);
                // Reset state on failure
                self.peer_manager
                    .update_peer_state(&temp_id, ConnectionState::Disconnected)
                    .await;
                // Drop stream - it will close automatically
                drop(stream);
                return Err(e);
            }
        };

        info!(
            "Handshake successful with {} (ID: {}, protocol: {})",
            addr, peer_id, protocol_version
        );

        // Update peer info with actual ID from handshake
        self.peer_manager.add_peer(peer_id.clone(), addr).await;
        // Remove temp peer if different (add_peer will update the entry)
        self.peer_manager
            .update_peer_state(&peer_id, ConnectionState::Connected)
            .await;
        self.peer_manager.update_peer_last_seen(&peer_id).await;

        self.event_emitter.emit("connected", Some(&peer_id)).await;
        let _ = self.ws_event_tx.send(crate::messenger_types::MessengerEvent::PeerConnected {
            peer_id: peer_id.clone(),
        });

        // Handle connection (this will block until connection closes)
        if let Err(e) = self.handle_connection(stream, peer_id.clone(), addr).await {
            error!("Error in connection with {}: {}", peer_id, e);
        }

        // Cleanup - only mark as disconnected if we're not already connected via another connection
        // Check if peer is still connected (might have reconnected)
        let current_state = self
            .peer_manager
            .get_peer(&peer_id)
            .await
            .map(|p| p.state)
            .unwrap_or(ConnectionState::Disconnected);

        if current_state == ConnectionState::Connected {
            // Peer is still connected (probably via another connection), don't mark as disconnected
            debug!("Peer {} is still connected, skipping disconnect", peer_id);
        } else {
            // Mark as disconnected only if not already connected
            self.peer_manager
                .update_peer_state(&peer_id, ConnectionState::Disconnected)
                .await;
            self.event_emitter
                .emit("disconnected", Some(&peer_id))
                .await;
            let _ = self.ws_event_tx.send(crate::messenger_types::MessengerEvent::PeerDisconnected {
                peer_id: peer_id.clone(),
            });
        }

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
            if connected.len() >= self.config.max_connections {
                debug!(
                    "Connector: at max connections ({}/{})",
                    connected.len(),
                    self.config.max_connections
                );
                continue;
            }
            let connected_ids: std::collections::HashSet<_> =
                connected.iter().map(|p| p.node_id.clone()).collect();

            debug!(
                "Connector: checking {} peers, {} connected",
                peers.len(),
                connected.len()
            );

            let slots = self
                .config
                .max_connections
                .saturating_sub(connected.len())
                .max(1);
            let permits = self.connect_semaphore.available_permits().max(1);
            let budget = slots.min(permits);

            // Rank candidates: fewer attempts, newer peers, lower latency if known
            let mut candidates: Vec<_> = peers
                .into_iter()
                .filter(|peer| {
                    !connected_ids.contains(&peer.node_id)
                        && peer.state != ConnectionState::Connecting
                        && peer.state != ConnectionState::Handshaking
                        && peer.connection_attempts < self.config.max_connection_attempts
                        && peer.node_id != self.id
                        && !(peer.address.ip().is_loopback()
                            && peer.address.port() == self.config.listen_addr.port())
                })
                .collect();

            candidates.sort_by(|a, b| {
                let c1 = a.connection_attempts.cmp(&b.connection_attempts);
                if c1 != std::cmp::Ordering::Equal {
                    return c1;
                }
                let c2 = b.added_at.cmp(&a.added_at);
                if c2 != std::cmp::Ordering::Equal {
                    return c2;
                }
                a.metrics
                    .latency
                    .unwrap_or(Duration::from_secs(9999))
                    .cmp(&b.metrics.latency.unwrap_or(Duration::from_secs(9999)))
            });

            for peer in candidates.into_iter().take(budget) {
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
        if peer_id == self.id {
            return;
        }

        // Exponential backoff based on attempts (cap at connect_backoff_max)
        let attempts = self
            .peer_manager
            .get_peer(&peer_id)
            .await
            .map(|p| p.connection_attempts)
            .unwrap_or(0);
        let exp = attempts.min(6); // 2^6 = 64x
        let factor: u32 = 1u32.checked_shl(exp).unwrap_or(u32::MAX);
        let backoff = self.config.connect_cooldown.saturating_mul(factor);
        let backoff = backoff.min(self.config.connect_backoff_max);

        // Cooldown by address to avoid reconnect storms (especially with discovery bursts)
        {
            let now = Instant::now();
            let mut map = self.last_connect_attempt.write().await;
            if let Some(last) = map.get(&addr) {
                if now.duration_since(*last) < backoff {
                    debug!(
                        "Skipping connect to {} (backoff {:?}, attempts {})",
                        addr, backoff, attempts
                    );
                    return;
                }
            }
            map.insert(addr, now);
        }

        // Limit concurrent connects
        let _permit = match self.connect_semaphore.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };
        // Double-check we're not already connected or connecting
        // Also check by address to catch cases where peer_id might be different
        let all_peers = self.peer_manager.get_all_peers().await;
        if let Some(peer) = all_peers
            .iter()
            .find(|p| p.address == addr || p.node_id == peer_id)
        {
            if peer.state == ConnectionState::Connected
                || peer.state == ConnectionState::Connecting
                || peer.state == ConnectionState::Handshaking
            {
                debug!(
                    "Skipping connection to {} - already {} state",
                    addr,
                    format!("{:?}", peer.state)
                );
                return;
            }
        }

        // Update state atomically
        self.peer_manager
            .update_peer_state(&peer_id, ConnectionState::Connecting)
            .await;
        self.peer_manager
            .increment_connection_attempts(&peer_id)
            .await;

        match timeout(self.config.connection_timeout, TcpStream::connect(&addr)).await {
            Ok(Ok(mut stream)) => {
                debug!("TCP connection established to {}", addr);

                // Update state to Handshaking
                self.peer_manager
                    .update_peer_state(&peer_id, ConnectionState::Handshaking)
                    .await;

                // Perform handshake
                debug!("Starting handshake with {} (outgoing)", addr);
                match self
                    .peer_manager
                    .perform_handshake(
                        &mut stream,
                        true, // true = outgoing connection
                        Some(&self.encryption_manager),
                        Some(&self.session_keys),
                    )
                    .await
                {
                    Ok((actual_peer_id, protocol_version, _peer_public_key)) => {
                        info!(
                            "Connected to peer {} (ID: {}, protocol: {})",
                            addr, actual_peer_id, protocol_version
                        );

                        // Update peer info with actual ID from handshake
                        self.peer_manager
                            .add_peer(actual_peer_id.clone(), addr)
                            .await;
                        self.peer_manager
                            .update_peer_state(&actual_peer_id, ConnectionState::Connected)
                            .await;
                        self.peer_manager
                            .update_peer_last_seen(&actual_peer_id)
                            .await;

                        self.event_emitter
                            .emit("connected", Some(&actual_peer_id))
                            .await;

                        // Handle connection (this will block until connection closes)
                        if let Err(e) = self
                            .handle_connection(stream, actual_peer_id.clone(), addr)
                            .await
                        {
                            error!("Error in connection with {}: {}", actual_peer_id, e);
                        }

                        // Cleanup - only mark as disconnected if we're not already connected via another connection
                        // Check if peer is still connected (might have reconnected)
                        let current_state = self
                            .peer_manager
                            .get_peer(&actual_peer_id)
                            .await
                            .map(|p| p.state)
                            .unwrap_or(ConnectionState::Disconnected);

                        if current_state == ConnectionState::Connected {
                            // Peer is still connected (probably via another connection), don't mark as disconnected
                            debug!(
                                "Peer {} is still connected, skipping disconnect",
                                actual_peer_id
                            );
                        } else {
                            // Mark as disconnected only if not already connected
                            self.peer_manager
                                .update_peer_state(&actual_peer_id, ConnectionState::Disconnected)
                                .await;
                            self.event_emitter
                                .emit("disconnected", Some(&actual_peer_id))
                                .await;
                        }
                    }
                    Err(e) => {
                        error!("Handshake failed with {}: {}", addr, e);
                        self.peer_manager
                            .update_peer_state(&peer_id, ConnectionState::Disconnected)
                            .await;
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to connect to {}: {}", addr, e);
                self.peer_manager
                    .update_peer_state(&peer_id, ConnectionState::Disconnected)
                    .await;
            }
            Err(_) => {
                warn!("Connection timeout to {}", addr);
                self.peer_manager
                    .update_peer_state(&peer_id, ConnectionState::Disconnected)
                    .await;
            }
        }
    }

    /// Handle established connection
    async fn handle_connection(
        &self,
        stream: TcpStream,
        peer_id: String,
        _addr: SocketAddr,
    ) -> Result<()> {
        use tokio::io::AsyncReadExt;

        // Create message channel for this connection
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let channel_token = Uuid::new_v4();
        {
            let mut senders = self.message_senders.write().await;
            senders.insert(
                peer_id.clone(),
                PeerChannel {
                    token: channel_token,
                    tx,
                },
            );
            info!(
                "Created message channel for peer {} (total channels: {})",
                peer_id,
                senders.len()
            );
        }

        // Split stream for reading and writing (owned halves)
        let (mut reader, mut writer) = tokio::io::split(stream);
        let mut len_buf = [0u8; 4];

        // Spawn task to handle outgoing messages
        let node_clone = self.clone();
        let peer_id_clone = peer_id.clone();
        let shutdown_clone = self.shutdown.clone();
        let channel_token_clone = channel_token;
        let writer_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(message) => {
                                if let Err(e) = node_clone.send_message_to_stream(&mut writer, &peer_id_clone, &message).await {
                                    warn!("Failed to send message to {}: {}", peer_id_clone, e);
                                    break;
                                }
                            }
                            None => {
                                debug!("Message channel closed for {}", peer_id_clone);
                                break;
                            }
                        }
                    }
                    _ = sleep(Duration::from_millis(100)) => {
                        // Check shutdown periodically
                        if *shutdown_clone.read().await {
                            break;
                        }
                    }
                }
            }

            // Cleanup: remove channel only if we're still the active one (prevents stomping newer connections)
            let mut senders = node_clone.message_senders.write().await;
            if let Some(ch) = senders.get(&peer_id_clone) {
                if ch.token == channel_token_clone {
                    senders.remove(&peer_id_clone);
                    node_clone
                        .peer_manager
                        .update_peer_state(&peer_id_clone, ConnectionState::Disconnected)
                        .await;
                }
            }
        });

        loop {
            if *self.shutdown.read().await {
                break;
            }

            // Read frame length
            match timeout(Duration::from_secs(30), reader.read_exact(&mut len_buf)).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    debug!("Connection closed by peer: {}", peer_id);
                    break;
                }
                Ok(Err(e)) => {
                    return Err(MeshError::Io(e));
                }
                Err(_) => {
                    // If a manual ping is in-flight, skip heartbeat ping to avoid clobbering
                    if self
                        .pending_manual_pings
                        .read()
                        .await
                        .contains_key(&peer_id)
                    {
                        continue;
                    }

                    // Check missed pongs before sending new ping
                    let missed = {
                        let mut missed_map = self.missed_pongs.write().await;
                        let count = missed_map.entry(peer_id.clone()).or_insert(0);
                        *count += 1;
                        *count
                    };

                    // If 3+ pongs missed, disconnect peer
                    if missed >= 3 {
                        warn!(
                            "Heartbeat: {} missed {} pongs, disconnecting",
                            peer_id, missed
                        );
                        // Clean up and break to disconnect
                        self.missed_pongs.write().await.remove(&peer_id);
                        self.pending_pings.write().await.remove(&peer_id);
                        break;
                    }

                    // Timeout - send ping and record timestamp for latency measurement
                    let ping_timestamp = Instant::now();
                    {
                        let mut pending = self.pending_pings.write().await;
                        pending.insert(peer_id.clone(), ping_timestamp);
                    }
                    let ping = Message::Ping {
                        timestamp: chrono::Utc::now().timestamp(),
                    };
                    if let Some(ch) = self.message_senders.read().await.get(&peer_id) {
                        if let Err(_e) = ch.tx.send(ping) {
                            warn!("Failed to send ping to {}: channel closed", peer_id);
                            // Remove from pending if send failed
                            let mut pending = self.pending_pings.write().await;
                            pending.remove(&peer_id);
                        }
                    }
                    continue;
                }
            }

            let length = u32::from_be_bytes(len_buf) as usize;
            let mut payload = vec![0u8; length];

            timeout(Duration::from_secs(30), reader.read_exact(&mut payload))
                .await
                .map_err(|_| MeshError::Timeout("Read timeout".to_string()))?
                .map_err(MeshError::Io)?;

            // Try to decrypt if we have a session key and payload looks encrypted (>= 12 bytes)
            let decrypted_payload =
                if let Some(session_key) = self.session_keys.get_session_key(&peer_id).await {
                    if payload.len() >= 12 {
                        // Try to decrypt (nonce is first 12 bytes)
                        let (nonce, encrypted_data) = payload.split_at(12);
                        match crate::p2p::encryption::EncryptionManager::decrypt_aes(
                            encrypted_data,
                            &session_key.key,
                            nonce,
                        ) {
                            Ok(decrypted) => {
                                debug!("Decrypted message from {}", peer_id);
                                decrypted
                            }
                            Err(e) => {
                                // If decryption fails, try parsing as plain message
                                debug!("Decryption failed for {}: {}, trying plain", peer_id, e);
                                payload
                            }
                        }
                    } else {
                        payload
                    }
                } else {
                    payload
                };

            // Parse message
            let message = Message::from_bytes(&decrypted_payload)
                .map_err(|e| MeshError::Protocol(format!("Invalid message: {}", e)))?;

            // Update last seen
            self.peer_manager.update_peer_last_seen(&peer_id).await;

            // Handle message
            match message {
                Message::Ping { timestamp } => {
                    debug!("Received ping from {}", peer_id);
                    let pong = Message::Pong { timestamp };
                    if let Some(ch) = self.message_senders.read().await.get(&peer_id) {
                        if let Err(_e) = ch.tx.send(pong) {
                            warn!("Failed to send pong to {}: channel closed", peer_id);
                        }
                    }
                }
                Message::Pong { timestamp: _ } => {
                    debug!("Received pong from {}", peer_id);

                    // Reset missed pongs counter (heartbeat is alive)
                    {
                        let mut missed = self.missed_pongs.write().await;
                        missed.remove(&peer_id);
                    }

                    // Prefer completing a manual ping if one is pending
                    let manual = {
                        let mut pending = self.pending_manual_pings.write().await;
                        pending.remove(&peer_id)
                    };
                    if let Some((ping_time, tx)) = manual {
                        let latency = ping_time.elapsed();
                        let _ = tx.send(latency);
                        self.peer_manager
                            .update_peer_latency(&peer_id, latency)
                            .await;
                        continue;
                    }
                    // Calculate latency from pending ping
                    let mut pending = self.pending_pings.write().await;
                    if let Some(ping_time) = pending.remove(&peer_id) {
                        let latency = ping_time.elapsed();
                        debug!("Latency to {}: {:?}", peer_id, latency);
                        self.peer_manager
                            .update_peer_latency(&peer_id, latency)
                            .await;
                    } else {
                        // Pong received but no pending ping (might be from another connection)
                        debug!("Received pong from {} but no pending ping found", peer_id);
                    }
                }
                Message::ContentRequest {
                    request_id,
                    url,
                    from_node,
                    ttl,
                    path,
                } => {
                    debug!(
                        "Received content request: {} for {} from {}",
                        request_id, url, from_node
                    );

                    // Check if this request is for us
                    if url.starts_with(&format!("ely://{}/", self.id)) {
                        // We are the content owner, check if we have it
                        match self.content_store.get(&url) {
                            Ok(Some(content)) => {
                                info!("✓ Content found locally: {} ({} bytes)", url, content.len());
                                // Send response back to requester
                                let response = Message::ContentResponse {
                                    request_id: request_id.clone(),
                                    url: url.clone(),
                                    content: Some(content),
                                    found: true,
                                    from_node: self.id.clone(),
                                };

                                // Send back via the peer who forwarded this request
                                if let Some(ch) = self.message_senders.read().await.get(&peer_id) {
                                    if let Err(e) = ch.tx.send(response) {
                                        warn!("Failed to send content response: {}", e);
                                    }
                                }
                            }
                            Ok(None) => {
                                info!("✗ Content not found: {}", url);
                                // Send not-found response
                                let response = Message::ContentResponse {
                                    request_id,
                                    url,
                                    content: None,
                                    found: false,
                                    from_node: self.id.clone(),
                                };

                                if let Some(ch) = self.message_senders.read().await.get(&peer_id) {
                                    if let Err(e) = ch.tx.send(response) {
                                        warn!("Failed to send not-found response: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Error fetching content {}: {}", url, e);
                            }
                        }
                    } else if ttl > 0 {
                        // Forward request to other peers (if we're not in the path yet)
                        if !path.contains(&self.id) {
                            debug!("Forwarding content request {} (ttl: {})", request_id, ttl);
                            let mut forward_path = path.clone();
                            forward_path.push(self.id.clone());

                            let forward_request = Message::ContentRequest {
                                request_id,
                                url,
                                from_node,
                                ttl: ttl - 1,
                                path: forward_path,
                            };

                            // Forward to all connected peers except sender
                            let senders = self.message_senders.read().await;
                            for (other_peer_id, ch) in senders.iter() {
                                if other_peer_id != &peer_id {
                                    let _ = ch.tx.send(forward_request.clone());
                                }
                            }
                        }
                    }
                }
                Message::ContentResponse {
                    request_id,
                    url,
                    content,
                    found,
                    from_node,
                } => {
                    if found {
                        info!(
                            "✓ Content response: {} from {} ({} bytes)",
                            url,
                            from_node,
                            content.as_ref().map(|c| c.len()).unwrap_or(0)
                        );
                    } else {
                        info!("✗ Content not found: {} (from {})", url, from_node);
                    }

                    // Match with pending request and deliver
                    if let Some(tx) = self
                        .pending_content_requests
                        .write()
                        .await
                        .remove(&request_id)
                    {
                        if let Some(data) = content {
                            let _ = tx.send(data);
                        }
                    }
                }
                Message::Data {
                    payload,
                    message_id,
                } => {
                    // Try to parse as MeshMessage for routing
                    let is_mesh_message = if let Ok(json_value) =
                        serde_json::from_slice::<serde_json::Value>(&payload)
                    {
                        // Check if it has MeshMessage structure (from, to, data, message_id, ttl, path)
                        json_value.get("from").is_some()
                            && json_value.get("message_id").is_some()
                            && json_value.get("ttl").is_some()
                    } else {
                        false
                    };

                    if is_mesh_message {
                        // Try to parse as MeshMessage and handle routing
                        if let Ok(protocol_msg) = serde_json::from_slice::<Message>(&payload) {
                            if let Message::MeshMessage { .. } = protocol_msg {
                                if let Some(mesh_msg) =
                                    MeshMessage::from_protocol_message(&protocol_msg)
                                {
                                    // Handle as mesh message for routing
                                    if let Err(e) =
                                        self.handle_mesh_message(mesh_msg, &peer_id).await
                                    {
                                        warn!("Error handling mesh message from Data: {}", e);
                                    }
                                    // Continue to also show content
                                }
                            }
                        }
                    }

                    // Try to parse as MeshMessage JSON to extract content
                    let content_str = if let Ok(json_value) =
                        serde_json::from_slice::<serde_json::Value>(&payload)
                    {
                        // Check if it's a MeshMessage structure
                        if let Some(data_field) = json_value.get("data") {
                            if let Some(data_array) = data_field.as_array() {
                                // Convert JSON array of numbers to bytes
                                let bytes: Vec<u8> = data_array
                                    .iter()
                                    .filter_map(|v| v.as_u64().map(|n| n as u8))
                                    .collect();
                                let bytes_len = bytes.len();
                                // Try to decode as UTF-8 string
                                String::from_utf8(bytes)
                                    .unwrap_or_else(|_| format!("{} bytes (binary)", bytes_len))
                            } else if let Some(data_str) = data_field.as_str() {
                                data_str.to_string()
                            } else {
                                format!("{} bytes", payload.len())
                            }
                        } else {
                            // Try to decode entire payload as UTF-8
                            String::from_utf8(payload.clone())
                                .unwrap_or_else(|_| format!("{} bytes (binary)", payload.len()))
                        }
                    } else {
                        // Try to decode as UTF-8 string directly
                        String::from_utf8(payload.clone())
                            .unwrap_or_else(|_| format!("{} bytes (binary)", payload.len()))
                    };

                    let display_content = if content_str.len() > 150 {
                        format!("{}...", &content_str[..150])
                    } else {
                        content_str.clone()
                    };

                    info!(
                        "📨 Received data message {} from {}: {} bytes\n   Content: \"{}\"",
                        message_id,
                        peer_id,
                        payload.len(),
                        display_content
                    );
                    // Store in inbox so CLI can display it
                    self.push_inbox(InboxMessage {
                        seq: 0,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        direction: "in".to_string(),
                        kind: "data".to_string(),
                        peer: peer_id.clone(),
                        from: peer_id.clone(),
                        to: Some(self.id.clone()),
                        message_id: Some(message_id.clone()),
                        bytes: payload.len(),
                        preview: display_content.clone(),
                        conversation_id: compute_conversation_id(&peer_id, Some(&self.id)),
                        content: Some(content_str),
                        delivered: false,
                    })
                    .await;
                    self.event_emitter
                        .emit("message_received", Some(&peer_id))
                        .await;
                }
                Message::MeshMessage { .. } => {
                    // Handle mesh message routing
                    if let Some(mesh_msg) = MeshMessage::from_protocol_message(&message) {
                        if let Err(e) = self.handle_mesh_message(mesh_msg, &peer_id).await {
                            warn!("Error handling mesh message: {}", e);
                        }
                    }
                }
                Message::Close { reason } => {
                    info!("Peer {} closed connection: {}", peer_id, reason);
                    break;
                }
                Message::MessageAck { message_id, from } => {
                    debug!("MessageAck received for {} from {}", message_id, from);
                    self.mark_message_delivered(&message_id).await;
                }
                _ => {
                    debug!(
                        "Received unhandled message type from {}: {}",
                        peer_id,
                        message.message_type()
                    );
                }
            }
        }

        // Cleanup: remove message sender and close writer task
        {
            let mut senders = self.message_senders.write().await;
            senders.remove(&peer_id);
            debug!(
                "Removed message channel for peer {} (remaining channels: {})",
                peer_id,
                senders.len()
            );
        }
        writer_handle.abort();

        Ok(())
    }

    /// Send a message to a stream (encrypts if session key is available)
    async fn send_message_to_stream(
        &self,
        stream: &mut (impl tokio::io::AsyncWrite + Unpin),
        peer_id: &str,
        message: &Message,
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        // Serialize message
        let plain_payload = message
            .to_bytes()
            .map_err(|e| MeshError::Protocol(format!("Failed to serialize message: {}", e)))?;

        // Encrypt if we have a session key
        let frame = if let Some(session_key) = self.session_keys.get_session_key(peer_id).await {
            // Generate new nonce for this message
            use aes_gcm::aead::{AeadCore, OsRng};
            let nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut OsRng);
            #[allow(deprecated)] // GenericArray::as_slice is deprecated
            let nonce_bytes = nonce.as_slice().to_vec();

            // Encrypt payload
            match crate::p2p::encryption::EncryptionManager::encrypt_aes(
                &plain_payload,
                &session_key.key,
                &nonce_bytes,
            ) {
                Ok(encrypted_data) => Frame::from_encrypted(&nonce_bytes, &encrypted_data),
                Err(e) => {
                    warn!(
                        "Failed to encrypt message to {}: {}, sending plain",
                        peer_id, e
                    );
                    Frame::from_message(message).map_err(|e| {
                        MeshError::Protocol(format!("Failed to create frame: {}", e))
                    })?
                }
            }
        } else {
            // No session key, send plain
            Frame::from_message(message)
                .map_err(|e| MeshError::Protocol(format!("Failed to create frame: {}", e)))?
        };

        // Send frame
        stream
            .write_all(&frame.to_bytes())
            .await
            .map_err(MeshError::Io)?;

        Ok(())
    }

    /// Send a message to a peer (encrypts if session key is available)
    /// This is a convenience method that uses the message channel
    #[allow(dead_code)]
    async fn send_message(
        &self,
        stream: &mut TcpStream,
        peer_id: &str,
        message: &Message,
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;

        // Serialize message
        let plain_payload = message
            .to_bytes()
            .map_err(|e| MeshError::Protocol(format!("Failed to serialize message: {}", e)))?;

        // Encrypt if we have a session key
        let frame = if let Some(session_key) = self.session_keys.get_session_key(peer_id).await {
            // Generate new nonce for this message
            use aes_gcm::aead::{AeadCore, OsRng};
            let nonce = aes_gcm::Aes256Gcm::generate_nonce(&mut OsRng);
            #[allow(deprecated)] // GenericArray::as_slice is deprecated
            let nonce_bytes = nonce.as_slice().to_vec();

            // Encrypt payload
            match crate::p2p::encryption::EncryptionManager::encrypt_aes(
                &plain_payload,
                &session_key.key,
                &nonce_bytes,
            ) {
                Ok(encrypted_data) => Frame::from_encrypted(&nonce_bytes, &encrypted_data),
                Err(e) => {
                    warn!(
                        "Failed to encrypt message to {}: {}, sending plain",
                        peer_id, e
                    );
                    Frame::from_message(message).map_err(|e| {
                        MeshError::Protocol(format!("Failed to create frame: {}", e))
                    })?
                }
            }
        } else {
            // No session key, send plain
            Frame::from_message(message)
                .map_err(|e| MeshError::Protocol(format!("Failed to create frame: {}", e)))?
        };

        // Send frame
        stream
            .write_all(&frame.to_bytes())
            .await
            .map_err(MeshError::Io)?;

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

            // Update uptime metrics for all connected peers
            // This is done by calling update_peer_last_seen which updates uptime
            let connected = self.peer_manager.get_connected_peers().await;
            for peer in &connected {
                // This will update uptime as a side effect
                self.peer_manager.update_peer_last_seen(&peer.node_id).await;
            }

            let connected = self.peer_manager.get_connected_peers().await;
            let all = self.peer_manager.get_all_peers().await;

            info!(
                "Heartbeat - Connected: {}/{} peers",
                connected.len(),
                all.len()
            );
            self.event_emitter.emit("heartbeat", None).await;

            // Remove stale peers
            let removed = self
                .peer_manager
                .remove_stale_peers(self.config.peer_stale_timeout)
                .await;
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
            encryption_manager: self.encryption_manager.clone(),
            session_keys: self.session_keys.clone(),
            router: self.router.clone(),
            routing_logger: self.routing_logger.clone(),
            content_store: self.content_store.clone(),
            name_registry: self.name_registry.clone(),
            message_store: self.message_store.clone(),
            shutdown: self.shutdown.clone(),
            shutdown_notify: self.shutdown_notify.clone(),
            message_senders: self.message_senders.clone(),
            pending_pings: self.pending_pings.clone(),
            pending_manual_pings: self.pending_manual_pings.clone(),
            missed_pongs: self.missed_pongs.clone(),
            pending_content_requests: self.pending_content_requests.clone(),
            api_addr: self.api_addr.clone(),
            discovery_port: self.discovery_port,
            connect_semaphore: self.connect_semaphore.clone(),
            last_connect_attempt: self.last_connect_attempt.clone(),
            inbox: self.inbox.clone(),
            inbox_next_seq: self.inbox_next_seq.clone(),
            inbox_notify: self.inbox_notify.clone(),
            contact_store: self.contact_store.clone(),
            ws_event_tx: self.ws_event_tx.clone(),
        }
    }
}

impl Node {
    /// Run discovery task
    async fn run_discovery(&self) -> Result<()> {
        if !self.config.enable_discovery {
            return Ok(());
        }

        let (tx, mut rx) = mpsc::unbounded_channel();

        let public_key = self.encryption_manager.get_public_key_string()?;
        let discovery_port = self.discovery_port;

        let discovery_manager = DiscoveryManager::new(
            self.id.clone(),
            self.config.listen_addr.port(),
            public_key,
            discovery_port,
            tx,
        );

        // Start discovery in background
        let discovery_handle = tokio::spawn(async move {
            if let Err(e) = discovery_manager.start().await {
                error!("Discovery error: {}", e);
            }
        });

        // Process discovered peers
        while let Some((node_id, addr, _public_key)) = rx.recv().await {
            if *self.shutdown.read().await {
                break;
            }

            info!("Discovered peer {} at {}", node_id, addr);
            self.peer_manager.add_peer(node_id.clone(), addr).await;

            // Persist for future boots (best-effort)
            if let Some(dir) = self.config.data_dir.as_ref() {
                if let Err(e) = peer_store::record_peer(dir, addr) {
                    debug!("Failed to record discovered peer {}: {}", addr, e);
                }
            }

            // Try to connect if not already connected
            let connected = self.peer_manager.get_connected_peers().await;
            if connected.len() >= self.config.max_connections {
                continue;
            }
            if !connected.iter().any(|p| p.node_id == node_id) {
                let node = self.clone();
                tokio::spawn(async move {
                    node.connect_to_peer(node_id, addr).await;
                });
            }
        }

        discovery_handle.abort();
        Ok(())
    }

    async fn bind_tcp_with_fallback(
        preferred: SocketAddr,
        tries: u16,
    ) -> Result<(TcpListener, SocketAddr)> {
        // 1) Preferred
        if let Ok(l) = TcpListener::bind(preferred).await {
            return Ok((l, preferred));
        }

        // 2) Increment a few ports
        for i in 1..=tries {
            if let Some(port) = preferred.port().checked_add(i) {
                let candidate = SocketAddr::new(preferred.ip(), port);
                if let Ok(l) = TcpListener::bind(candidate).await {
                    return Ok((l, candidate));
                }
            }
        }

        // 3) Ephemeral port (still local)
        let ephemeral = SocketAddr::new(preferred.ip(), 0);
        let l = TcpListener::bind(ephemeral).await.map_err(MeshError::Io)?;
        let actual = l.local_addr().map_err(MeshError::Io)?;
        Ok((l, actual))
    }

    /// Handle incoming mesh message
    async fn handle_mesh_message(&self, message: MeshMessage, from_peer: &str) -> Result<()> {
        // Check if we should process this message
        if !self.router.should_process(&message).await {
            return Ok(());
        }

        // Mark as seen
        let message_id = message.message_id.clone();
        self.router.mark_seen(&message_id).await;

        // Check if message is for us (broadcast or directed to us)
        let is_broadcast = message.to.is_none();
        let is_for_us = self.router.is_for_us(&message);

        // Decode ElysiumPacket if present (backward compatible: ignore errors)
        let mut decoded_packet: Option<ElysiumPacket> = None;
        if let Ok(packet) = ElysiumPacket::from_bytes(&message.data) {
            let dst = packet
                .dst
                .clone()
                .unwrap_or_else(|| "broadcast".to_string());
            let preview = String::from_utf8(packet.payload.clone())
                .unwrap_or_else(|_| format!("{} bytes (binary)", packet.payload.len()));
            info!(
                "📦 ElysiumPacket {} -> {} (via {}) Payload: {}",
                packet.src,
                dst,
                from_peer,
                if preview.len() > 80 {
                    format!("{}...", &preview[..80])
                } else {
                    preview
                }
            );
            decoded_packet = Some(packet);
        }

        if is_for_us {
            info!(
                "Received mesh message {} for us from {}",
                message_id, from_peer
            );
            self.event_emitter
                .emit("mesh_message_received", Some(&message_id))
                .await;
            // Deliver to application layer (inbox for CLI)
            if let Some(packet) = decoded_packet {
                let full_content = String::from_utf8(packet.payload.clone())
                    .unwrap_or_else(|_| format!("{} bytes (binary)", packet.payload.len()));
                let preview = if full_content.len() > 200 {
                    format!("{}...", &full_content[..200])
                } else {
                    full_content.clone()
                };
                let msg_from = packet.src.clone();
                let msg_to = packet.dst.clone();
                self.push_inbox(InboxMessage {
                    seq: 0,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    direction: "in".to_string(),
                    kind: "mesh".to_string(),
                    peer: from_peer.to_string(),
                    from: msg_from.clone(),
                    to: msg_to.clone(),
                    message_id: Some(message_id.clone()),
                    bytes: packet.payload.len(),
                    preview,
                    conversation_id: compute_conversation_id(&msg_from, msg_to.as_deref()),
                    content: Some(full_content),
                    delivered: false,
                })
                .await;
            } else {
                let full_content = String::from_utf8(message.data.clone())
                    .unwrap_or_else(|_| format!("{} bytes (binary)", message.data.len()));
                let preview = if full_content.len() > 200 {
                    format!("{}...", &full_content[..200])
                } else {
                    full_content.clone()
                };
                let msg_from = from_peer.to_string();
                let msg_to = message.to.clone();
                self.push_inbox(InboxMessage {
                    seq: 0,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    direction: "in".to_string(),
                    kind: "mesh".to_string(),
                    peer: from_peer.to_string(),
                    from: msg_from.clone(),
                    to: msg_to.clone(),
                    message_id: Some(message_id.clone()),
                    bytes: message.data.len(),
                    preview,
                    conversation_id: compute_conversation_id(&msg_from, msg_to.as_deref()),
                    content: Some(full_content),
                    delivered: false,
                })
                .await;
            }

            // Send MessageAck back to the direct sender if this was a directed message
            if !is_broadcast {
                let senders = self.message_senders.read().await;
                if let Some(ch) = senders.get(from_peer) {
                    let ack = Message::MessageAck {
                        message_id: message_id.clone(),
                        from: self.id.clone(),
                    };
                    if let Err(e) = ch.tx.send(ack) {
                        debug!("Could not send MessageAck to {}: {}", from_peer, e);
                    }
                }
            }

            // If it's a directed message (not broadcast), don't forward
            if !is_broadcast {
                return Ok(());
            }
            // For broadcast messages, continue to forward below
        }

        // Forward to other peers using AI-routing (select best peers based on metrics)
        let all_peers = self.peer_manager.get_all_peers().await;

        // Use AI-routing to select best peers (top 3 by default, or all if less than 3)
        let max_forward_peers = 3;
        let forward_peers = self
            .router
            .get_best_forward_peers(&message, &all_peers, max_forward_peers)
            .await;

        if !forward_peers.is_empty() {
            let forward_msg = self.router.prepare_for_forwarding(&message);
            let protocol_msg = forward_msg.to_protocol_message();

            // Log selected peers with their scores
            let peer_scores: Vec<(String, f64)> = forward_peers
                .iter()
                .filter_map(|peer_id| {
                    all_peers
                        .iter()
                        .find(|p| p.node_id == *peer_id)
                        .map(|peer| {
                            let score = Router::calculate_peer_score(&peer.metrics, None);
                            (peer_id.clone(), score)
                        })
                })
                .collect();

            if self.config.ai_debug {
                // Debug output: show all peer scores
                info!(
                    "🧠 AI-Routing Debug: All peer scores for message {}:",
                    forward_msg.message_id
                );
                // Note: route_history is private, so we can't access it directly
                // This is a simplified debug output
                for peer in &all_peers {
                    if peer.is_connected() && peer.node_id != message.from {
                        let score = Router::calculate_peer_score(&peer.metrics, None);
                        info!(
                            "  Peer {}: score={:.3}, latency={}, uptime={:.1}s, reliability={:.2}",
                            peer.node_id,
                            score,
                            peer.metrics
                                .latency
                                .map(|d| format!("{:.0}ms", d.as_millis()))
                                .unwrap_or_else(|| "N/A".to_string()),
                            peer.metrics.uptime.as_secs_f64(),
                            peer.metrics.reliability_score()
                        );
                    }
                }
            }

            info!(
                "🎯 AI-Routing: Forwarding mesh message {} to {} peer(s): {:?}",
                forward_msg.message_id,
                forward_peers.len(),
                peer_scores
                    .iter()
                    .map(|(id, score)| format!("{} (score: {:.2})", id, score))
                    .collect::<Vec<_>>()
            );

            // Log routing decision for AI training
            let selected_peers_log: Vec<PeerSelection> = peer_scores
                .iter()
                .filter_map(|(peer_id, score)| {
                    all_peers
                        .iter()
                        .find(|p| p.node_id == *peer_id)
                        .map(|peer| PeerSelection {
                            peer_id: peer_id.clone(),
                            score: *score,
                            metrics: PeerMetricsSnapshot::from(peer),
                        })
                })
                .collect();

            let available_peers_log: Vec<PeerMetricsSnapshot> =
                all_peers.iter().map(PeerMetricsSnapshot::from).collect();

            let log_entry = RoutingLogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                message_id: forward_msg.message_id.clone(),
                node_id: self.id.clone(),
                from_peer: Some(from_peer.to_string()),
                selected_peers: selected_peers_log,
                available_peers: available_peers_log,
                message_context: MessageContext {
                    ttl: forward_msg.ttl,
                    path_length: forward_msg.path.len(),
                    is_broadcast: message.to.is_none(),
                    target_peer: message.to.clone(),
                },
            };
            self.routing_logger.log_routing_decision(log_entry).await;

            // Forward to each selected peer
            for peer_id in forward_peers {
                if let Some(peer) = all_peers.iter().find(|p| p.node_id == peer_id) {
                    if peer.is_connected() {
                        // Emit event for visualization
                        self.event_emitter
                            .emit("message_sent", Some(&peer_id))
                            .await;

                        // Send via existing connection channel
                        let data_msg = Message::Data {
                            payload: serde_json::to_vec(&protocol_msg).unwrap_or_default(),
                            message_id: forward_msg.message_id.clone(),
                        };

                        let senders = self.message_senders.read().await;
                        if let Some(ch) = senders.get(&peer_id) {
                            if let Err(e) = ch.tx.send(data_msg) {
                                warn!(
                                    "Failed to forward message to {}: channel closed ({})",
                                    peer_id, e
                                );
                            } else {
                                debug!(
                                    "Forwarded mesh message {} to {} (score: {:.2})",
                                    forward_msg.message_id,
                                    peer_id,
                                    Router::calculate_peer_score(&peer.metrics, None)
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Send a mesh message to a specific peer or broadcast
    pub async fn send_mesh_message(&self, to: Option<String>, data: Vec<u8>) -> Result<String> {
        // Show message content for logging (before moving data)
        let full_content = String::from_utf8(data.clone())
            .unwrap_or_else(|_| format!("{} bytes (binary)", data.len()));
        let content_bytes_len = full_content.len();
        let content_display = if full_content.len() > 50 {
            format!("{}...", &full_content[..50])
        } else {
            full_content.clone()
        };

        // Wrap into ElysiumPacket (future-proof for signatures/TOFU). Routing target remains MeshMessage.to.
        let packet = ElysiumPacket::new(self.id.clone(), to.clone(), data);
        let packet_bytes = packet.to_bytes()?;

        let mesh_msg = MeshMessage::new(self.id.clone(), to.clone(), packet_bytes);
        let message_id = mesh_msg.message_id.clone();
        let protocol_msg = mesh_msg.to_protocol_message();

        // Check active connections via message_senders (more reliable than peer state)
        let senders = self.message_senders.read().await;
        let active_peer_ids: Vec<String> = senders.keys().cloned().collect();

        info!(
            "send_mesh_message: active peer channels: {:?} (total: {})",
            active_peer_ids,
            active_peer_ids.len()
        );

        if active_peer_ids.is_empty() {
            return Err(MeshError::Protocol("No connected peers".to_string()));
        }

        // Send to specific peer or broadcast to all
        let target_peers: Vec<String> = if let Some(target_id) = &to {
            if active_peer_ids.iter().any(|id| id == target_id) {
                vec![target_id.clone()]
            } else {
                return Err(MeshError::Protocol(format!(
                    "Peer {} not connected (active peers: {:?})",
                    target_id, active_peer_ids
                )));
            }
        } else {
            active_peer_ids
        };

        info!(
            "📤 Sending mesh message {} to {} peer(s)\n   Content: {}",
            message_id,
            target_peers.len(),
            content_display
        );

        // Store in local outbox so CLI watch shows our sends too
        self.push_inbox(InboxMessage {
            seq: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            direction: "out".to_string(),
            kind: "mesh".to_string(),
            peer: to.clone().unwrap_or_else(|| "broadcast".to_string()),
            from: self.id.clone(),
            to: to.clone(),
            message_id: Some(message_id.clone()),
            bytes: content_bytes_len,
            preview: content_display.clone(),
            conversation_id: compute_conversation_id(&self.id, to.as_deref()),
            content: Some(full_content),
            delivered: false,
        })
        .await;

        // Send as Data message to connected peers using existing connections
        for peer_id in &target_peers {
            // Emit event for visualization with correct peer_id
            self.event_emitter.emit("message_sent", Some(peer_id)).await;

            // Send message through existing connection channel
            let data_msg = Message::Data {
                payload: serde_json::to_vec(&protocol_msg).unwrap_or_default(),
                message_id: message_id.clone(),
            };

            if let Some(ch) = senders.get(peer_id) {
                if let Err(e) = ch.tx.send(data_msg) {
                    warn!(
                        "Failed to send message to {}: channel closed ({})",
                        peer_id, e
                    );
                } else {
                    info!("Message {} queued for sending to {}", message_id, peer_id);
                }
            } else {
                warn!(
                    "No message channel found for peer {} (available: {:?})",
                    peer_id,
                    senders.keys().collect::<Vec<_>>()
                );
            }
        }

        Ok(message_id)
    }

    /// Get list of all peers with their status
    pub async fn get_peers(&self) -> Vec<(String, SocketAddr, ConnectionState)> {
        let peers = self.peer_manager.get_all_peers().await;
        peers
            .into_iter()
            .map(|p| (p.node_id, p.address, p.state))
            .collect()
    }

    /// Get IDs of peers that currently have an active connection channel.
    pub async fn get_active_peer_ids(&self) -> Vec<String> {
        let senders = self.message_senders.read().await;
        senders.keys().cloned().collect()
    }

    /// Get node status
    pub async fn get_status(&self) -> (String, usize, usize) {
        let all_peers = self.peer_manager.get_all_peers().await;
        let connected = self.peer_manager.get_connected_peers().await;
        (self.id.clone(), connected.len(), all_peers.len())
    }
}
