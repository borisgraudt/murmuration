/// Core P2P tests
/// Integration tests for P2P protocol, encryption, and routing

use core::p2p::encryption::{EncryptionManager, SessionKeyManager};
use core::p2p::peer::PeerManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn test_handshake_outgoing_connection() {
    // Create two peer managers
    let manager1 = Arc::new(PeerManager::new("node1".to_string(), 8080));
    let manager2 = Arc::new(PeerManager::new("node2".to_string(), 8081));

    // Create encryption managers
    let enc_mgr1 = EncryptionManager::new().unwrap();
    let enc_mgr2 = EncryptionManager::new().unwrap();

    // Create session key managers
    let sess_keys1 = SessionKeyManager::new();
    let sess_keys2 = SessionKeyManager::new();

    // Start listener on manager2
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn task to accept incoming connection
    let manager2_clone = manager2.clone();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        manager2_clone
            .perform_handshake(
                &mut stream,
                false, // incoming
                Some(&enc_mgr2),
                Some(&sess_keys2),
            )
            .await
    });

    // Connect from manager1 (outgoing)
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let result = manager1
        .perform_handshake(
            &mut stream,
            true, // outgoing
            Some(&enc_mgr1),
            Some(&sess_keys1),
        )
        .await;

    // Wait for both sides to complete
    let (peer_id, protocol_version, _) = result.unwrap();
    let (peer_id2, protocol_version2, _) = handle.await.unwrap().unwrap();

    // Verify handshake results
    assert_eq!(peer_id, "node2");
    assert_eq!(peer_id2, "node1");
    assert_eq!(protocol_version, 1);
    assert_eq!(protocol_version2, 1);
}

#[tokio::test]
async fn test_handshake_incoming_connection() {
    // Create two peer managers
    let manager1 = Arc::new(PeerManager::new("node1".to_string(), 8080));
    let manager2 = Arc::new(PeerManager::new("node2".to_string(), 8081));

    // Create encryption managers
    let enc_mgr1 = EncryptionManager::new().unwrap();
    let enc_mgr2 = EncryptionManager::new().unwrap();

    // Create session key managers
    let sess_keys1 = SessionKeyManager::new();
    let sess_keys2 = SessionKeyManager::new();

    // Start listener on manager1
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn task to accept incoming connection
    let manager1_clone = manager1.clone();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        manager1_clone
            .perform_handshake(
                &mut stream,
                false, // incoming
                Some(&enc_mgr1),
                Some(&sess_keys1),
            )
            .await
    });

    // Connect from manager2 (outgoing)
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let result = manager2
        .perform_handshake(
            &mut stream,
            true, // outgoing
            Some(&enc_mgr2),
            Some(&sess_keys2),
        )
        .await;

    // Wait for both sides to complete
    let (peer_id, protocol_version, _) = result.unwrap();
    let (peer_id2, protocol_version2, _) = handle.await.unwrap().unwrap();

    // Verify handshake results
    assert_eq!(peer_id, "node1");
    assert_eq!(peer_id2, "node2");
    assert_eq!(protocol_version, 1);
    assert_eq!(protocol_version2, 1);
}

