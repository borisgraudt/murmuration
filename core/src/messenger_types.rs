/// Shared types for the Messenger layer
use crate::node::InboxMessage;
use serde::{Deserialize, Serialize};

/// Summary of one conversation thread (for list view in Swift UI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    /// Canonical ID: "dm:{min_id}:{max_id}" or "broadcast"
    pub conversation_id: String,
    /// The other party's node_id (empty for broadcast)
    pub peer_id: String,
    /// Preview text of the last message
    pub last_preview: String,
    /// RFC3339 timestamp of the last message
    pub last_timestamp: String,
    /// Sequence number of the last message (for pagination)
    pub last_seq: u64,
}

/// Real-time events streamed over SSE (/events endpoint)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessengerEvent {
    /// A new message arrived or was sent
    NewMessage { message: InboxMessage },
    /// A peer connected to our node
    PeerConnected { peer_id: String },
    /// A peer disconnected from our node
    PeerDisconnected { peer_id: String },
    /// A message we sent was acknowledged by the recipient
    MessageDelivered { message_id: String },
}
