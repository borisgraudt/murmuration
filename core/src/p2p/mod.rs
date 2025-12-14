pub mod discovery;
pub mod encryption;
pub mod encryption_pqc;
pub mod peer;
/// P2P networking modules
pub mod protocol;

pub use discovery::{DiscoveryManager, DiscoveryMessage};
pub use encryption::{EncryptedMessage, EncryptionManager};
pub use encryption_pqc::{is_pqc_available, PqcEncryptionManager};
pub use peer::{ConnectionState, PeerInfo, PeerManager};
pub use protocol::{Frame, Message, PROTOCOL_VERSION};
