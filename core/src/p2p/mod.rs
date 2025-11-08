/// P2P networking modules
pub mod protocol;
pub mod peer;
pub mod encryption;
pub mod discovery;

pub use protocol::{Frame, Message, PROTOCOL_VERSION};
pub use peer::{ConnectionState, PeerInfo, PeerManager};
pub use encryption::{EncryptionManager, EncryptedMessage};
pub use discovery::{DiscoveryManager, DiscoveryMessage};

