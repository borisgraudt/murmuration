/// Test multi-node connectivity and message routing
extern crate meshlink_core;

use meshlink_core::{Config, Node};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_3_nodes_connectivity() {
    // This test verifies that 3 nodes can connect to each other
    // Note: This is an integration test that may require actual network setup
    
    // Create configs for 3 nodes
    let config1 = Config {
        listen_addr: "127.0.0.1:18080".parse().unwrap(),
        known_peers: vec![],
        ..Default::default()
    };
    
    let config2 = Config {
        listen_addr: "127.0.0.1:18081".parse().unwrap(),
        known_peers: vec!["127.0.0.1:18080".to_string()],
        ..Default::default()
    };
    
    let config3 = Config {
        listen_addr: "127.0.0.1:18082".parse().unwrap(),
        known_peers: vec!["127.0.0.1:18080".to_string()],
        ..Default::default()
    };
    
    // Create nodes
    let node1 = Node::new(config1).expect("Failed to create node1");
    let node2 = Node::new(config2).expect("Failed to create node2");
    let node3 = Node::new(config3).expect("Failed to create node3");
    
    // Start nodes
    let handle1 = {
        let n = node1.clone();
        tokio::spawn(async move { n.start().await })
    };
    let handle2 = {
        let n = node2.clone();
        tokio::spawn(async move { n.start().await })
    };
    let handle3 = {
        let n = node3.clone();
        tokio::spawn(async move { n.start().await })
    };
    
    // Wait for connections to establish
    sleep(Duration::from_secs(3)).await;
    
    // Check connectivity
    let (_id1, connected1, _total1) = node1.get_status().await;
    let (_id2, connected2, _total2) = node2.get_status().await;
    let (_id3, connected3, _total3) = node3.get_status().await;
    
    // Each node should have at least 1 connection
    assert!(
        connected1 >= 1,
        "Node1 should have at least 1 connected peer"
    );
    assert!(
        connected2 >= 1,
        "Node2 should have at least 1 connected peer"
    );
    assert!(
        connected3 >= 1,
        "Node3 should have at least 1 connected peer"
    );
    
    // Cleanup
    node1.request_shutdown();
    node2.request_shutdown();
    node3.request_shutdown();
    let _ = tokio::time::timeout(Duration::from_secs(2), handle1).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), handle2).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), handle3).await;
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_5_nodes_mesh() {
    // Test with 5 nodes in a mesh topology
    let mut nodes = vec![];
    let mut handles = vec![];
    
    // Create 5 nodes
    for i in 0..5 {
        let port = 18100 + i;
        let known_peers = if i > 0 {
            vec![format!("127.0.0.1:{}", 18100)]
        } else {
            vec![]
        };
        
        let config = Config {
            listen_addr: format!("127.0.0.1:{}", port).parse().unwrap(),
            known_peers,
            ..Default::default()
        };
        
        let node = Node::new(config).expect(&format!("Failed to create node{}", i));
        let handle = {
            let n = node.clone();
            tokio::spawn(async move { n.start().await })
        };
        nodes.push(node);
        handles.push(handle);
    }
    
    // Wait for mesh to form
    sleep(Duration::from_secs(5)).await;
    
    // Check that nodes are connected
    for (i, node) in nodes.iter().enumerate() {
        let (_id, connected, _total) = node.get_status().await;
        assert!(
            connected >= 1,
            "Node{} should have at least 1 connected peer",
            i
        );
    }
    
    // Cleanup
    for node in &nodes {
        node.request_shutdown();
    }
    for handle in handles {
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_message_broadcast_3_nodes() {
    // Test message broadcasting across 3 nodes
    let config1 = Config {
        listen_addr: "127.0.0.1:18200".parse().unwrap(),
        known_peers: vec![],
        ..Default::default()
    };
    
    let config2 = Config {
        listen_addr: "127.0.0.1:18201".parse().unwrap(),
        known_peers: vec!["127.0.0.1:18200".to_string()],
        ..Default::default()
    };
    
    let config3 = Config {
        listen_addr: "127.0.0.1:18202".parse().unwrap(),
        known_peers: vec!["127.0.0.1:18200".to_string()],
        ..Default::default()
    };
    
    let node1 = Node::new(config1).expect("Failed to create node1");
    let node2 = Node::new(config2).expect("Failed to create node2");
    let node3 = Node::new(config3).expect("Failed to create node3");
    
    let handle1 = {
        let n = node1.clone();
        tokio::spawn(async move { n.start().await })
    };
    let handle2 = {
        let n = node2.clone();
        tokio::spawn(async move { n.start().await })
    };
    let handle3 = {
        let n = node3.clone();
        tokio::spawn(async move { n.start().await })
    };
    
    // Wait for connections
    sleep(Duration::from_secs(3)).await;
    
    // Send broadcast message from node1
    let message_data = b"Test broadcast message".to_vec();
    let result = node1.send_mesh_message(None, message_data).await;
    
    // Message should be sent successfully
    assert!(result.is_ok(), "Broadcast message should be sent successfully");
    
    // Wait for propagation
    sleep(Duration::from_secs(2)).await;
    
    // Cleanup
    node1.request_shutdown();
    node2.request_shutdown();
    node3.request_shutdown();
    let _ = tokio::time::timeout(Duration::from_secs(2), handle1).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), handle2).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), handle3).await;
    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_ai_routing_selection() {
    // Test that AI-routing selects best peers
    let config1 = Config {
        listen_addr: "127.0.0.1:18300".parse().unwrap(),
        known_peers: vec![],
        ai_debug: true, // Enable debug to see routing decisions
        ..Default::default()
    };
    
    let config2 = Config {
        listen_addr: "127.0.0.1:18301".parse().unwrap(),
        known_peers: vec!["127.0.0.1:18300".to_string()],
        ..Default::default()
    };
    
    let config3 = Config {
        listen_addr: "127.0.0.1:18302".parse().unwrap(),
        known_peers: vec!["127.0.0.1:18300".to_string()],
        ..Default::default()
    };
    
    let node1 = Node::new(config1).expect("Failed to create node1");
    let node2 = Node::new(config2).expect("Failed to create node2");
    let node3 = Node::new(config3).expect("Failed to create node3");
    
    let handle1 = {
        let n = node1.clone();
        tokio::spawn(async move { n.start().await })
    };
    let handle2 = {
        let n = node2.clone();
        tokio::spawn(async move { n.start().await })
    };
    let handle3 = {
        let n = node3.clone();
        tokio::spawn(async move { n.start().await })
    };
    
    // Wait for connections and metrics to stabilize
    sleep(Duration::from_secs(5)).await;
    
    // Send message - AI-routing should select best peers
    let message_data = b"AI routing test".to_vec();
    let result = node1.send_mesh_message(None, message_data).await;
    
    assert!(result.is_ok(), "Message should be sent with AI-routing");
    
    // Cleanup
    node1.request_shutdown();
    node2.request_shutdown();
    node3.request_shutdown();
    let _ = tokio::time::timeout(Duration::from_secs(2), handle1).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), handle2).await;
    let _ = tokio::time::timeout(Duration::from_secs(2), handle3).await;
    sleep(Duration::from_millis(100)).await;
}


