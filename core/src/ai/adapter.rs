//! Bridge between the routing crate's storage-agnostic [`MeshMessage`] and the
//! node's wire [`Message`]. These conversions used to be methods on `MeshMessage`;
//! they live here so the `murmuration-routing` crate stays free of the protocol
//! type.

use crate::p2p::protocol::Message;
use murmuration_routing::MeshMessage;

/// Wrap a `MeshMessage` into the wire `Message` for transmission.
pub fn mesh_to_protocol(m: &MeshMessage) -> Message {
    Message::MeshMessage {
        from: m.from.clone(),
        to: m.to.clone(),
        data: m.data.clone(),
        message_id: m.message_id.clone(),
        ttl: m.ttl,
        path: m.path.clone(),
    }
}

/// Extract a `MeshMessage` from a wire `Message`, if it is one.
pub fn mesh_from_protocol(msg: &Message) -> Option<MeshMessage> {
    if let Message::MeshMessage {
        from,
        to,
        data,
        message_id,
        ttl,
        path,
    } = msg
    {
        Some(MeshMessage {
            from: from.clone(),
            to: to.clone(),
            data: data.clone(),
            message_id: message_id.clone(),
            ttl: *ttl,
            path: path.clone(),
        })
    } else {
        None
    }
}
