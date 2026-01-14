#![allow(clippy::field_reassign_with_default)]
/// Stability tests - ensure no panics, proper error handling, timeouts
use base64::Engine;
use meshlink_core::config::Config;
use meshlink_core::node::Node;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

// Helper to create unique data dir for each test
fn unique_data_dir(test_name: &str) -> PathBuf {
    PathBuf::from(format!(".ely/test-{}-{}", test_name, std::process::id()))
}

#[tokio::test]
async fn test_content_request_timeout() {
    // Create a node
    let mut config = Config::default();
    config.listen_addr = "127.0.0.1:0".parse().unwrap();
    config.api_addr = Some("127.0.0.1:0".parse().unwrap());
    config.data_dir = Some(unique_data_dir("timeout"));

    let node = Node::new(config).unwrap();

    // Try to fetch non-existent content with short timeout
    let result = timeout(
        Duration::from_secs(2),
        node.fetch_content(
            "ely://nonexistent_node/some/path",
            Duration::from_millis(500),
        ),
    )
    .await;

    // Should timeout or return error, but not panic
    match result {
        Ok(Ok(None)) => {
            // Content not found - OK
        }
        Ok(Err(e)) => {
            // Error or timeout - OK, as long as it doesn't panic
            println!("Expected error/timeout: {}", e);
        }
        Err(_) => {
            // Outer timeout - OK
        }
        Ok(Ok(Some(_))) => {
            panic!("Unexpected: content found");
        }
    }
}

#[tokio::test]
async fn test_invalid_url_handling() {
    // Create a node with unique data dir to avoid lock conflicts
    use std::path::PathBuf;
    let mut config = Config::default();
    config.listen_addr = "127.0.0.1:0".parse().unwrap();
    config.api_addr = Some("127.0.0.1:0".parse().unwrap());
    config.data_dir = Some(PathBuf::from(format!(
        ".ely/test-invalid-url-{}",
        std::process::id()
    )));

    let node = Node::new(config).unwrap();

    // Test various invalid URLs - should not panic
    let invalid_urls = vec![
        "not-an-ely-url",
        "ely://",
        "ely://node",
        "http://example.com",
        "",
        "ely://node/",
    ];

    for url in invalid_urls {
        let result = node.fetch_content(url, Duration::from_secs(1)).await;
        // Should return error, not panic
        assert!(
            result.is_err() || result.unwrap().is_none(),
            "URL '{}' should return error or None, not panic",
            url
        );
    }
}

#[tokio::test]
async fn test_web_gateway_error_handling() {
    // This test verifies that web_gateway doesn't panic on errors
    // We can't easily test the full HTTP server, but we can test error handling logic

    // Test that invalid base64 doesn't crash
    let invalid_base64 = "!!!invalid!!!";
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(invalid_base64)
        .ok();

    // Should return None, not panic
    assert!(decoded.is_none());
}

#[tokio::test]
async fn test_node_creation_with_invalid_config() {
    // Test that invalid configs are handled gracefully
    let mut config = Config::default();
    config.listen_addr = "127.0.0.1:0".parse().unwrap();
    config.data_dir = Some(unique_data_dir("invalid-config"));

    // Valid config should work
    let node_result = Node::new(config.clone());
    assert!(node_result.is_ok(), "Valid config should create node");

    // Test with invalid API port (too high) - parse will fail
    let invalid_addr_result = "127.0.0.1:99999".parse::<std::net::SocketAddr>();
    if let Ok(addr) = invalid_addr_result {
        config.api_addr = Some(addr);
        // This should still work (port validation happens at bind time)
        let node_result2 = Node::new(config);
        // Should either succeed or return a clear error, not panic
        if let Err(e) = node_result2 {
            println!("Expected error for invalid port: {}", e);
        }
    } else {
        // Port parsing failed, which is expected
        println!("Port parsing failed as expected for invalid port");
    }
}

#[tokio::test]
async fn test_concurrent_content_requests() {
    // Test that multiple concurrent requests don't cause issues
    let mut config = Config::default();
    config.listen_addr = "127.0.0.1:0".parse().unwrap();
    config.api_addr = Some("127.0.0.1:0".parse().unwrap());
    config.data_dir = Some(unique_data_dir("concurrent"));

    let node = Node::new(config).unwrap();
    let node_arc = std::sync::Arc::new(node);

    // Spawn multiple concurrent requests
    let mut handles = vec![];
    for i in 0..5 {
        let node_clone = node_arc.clone();
        let handle = tokio::spawn(async move {
            let url = format!("ely://node{}/path{}", i, i);
            node_clone
                .fetch_content(&url, Duration::from_millis(100))
                .await
        });
        handles.push(handle);
    }

    // Wait for all requests with timeout
    let result = timeout(Duration::from_secs(2), async {
        for handle in handles {
            let _ = handle.await;
        }
    })
    .await;

    // Should complete without panic
    assert!(
        result.is_ok(),
        "Concurrent requests should complete without panic"
    );
}

#[tokio::test]
async fn test_empty_content_store() {
    // Test fetching from empty content store
    let mut config = Config::default();
    config.listen_addr = "127.0.0.1:0".parse().unwrap();
    config.api_addr = Some("127.0.0.1:0".parse().unwrap());

    let node = Node::new(config).unwrap();

    // Try to fetch local content that doesn't exist
    // Get node ID from status or use a known format
    let node_id = "test_node_id"; // We'll use a placeholder since id() might not be public
    let local_url = format!("ely://{}/nonexistent/path", node_id);
    let result = node.fetch_content(&local_url, Duration::from_secs(1)).await;

    // Should return Ok(None), not panic
    match result {
        Ok(None) => {
            // Expected - content not found
        }
        Ok(Some(_)) => {
            panic!("Unexpected: found content in empty store");
        }
        Err(e) => {
            // Error is also acceptable
            println!("Error fetching from empty store: {}", e);
        }
    }
}

#[tokio::test]
async fn test_very_long_url() {
    // Test handling of very long URLs (potential buffer overflow)
    let mut config = Config::default();
    config.listen_addr = "127.0.0.1:0".parse().unwrap();
    config.api_addr = Some("127.0.0.1:0".parse().unwrap());
    config.data_dir = Some(unique_data_dir("long-url"));

    let node = Node::new(config).unwrap();

    // Create a very long URL
    let long_path = "a".repeat(10000);
    let long_url = format!("ely://node/{}", long_path);

    // Should handle gracefully, not panic
    let result = node
        .fetch_content(&long_url, Duration::from_millis(100))
        .await;
    // Should return error or None, not panic
    assert!(result.is_err() || result.unwrap().is_none());
}

#[tokio::test]
async fn test_special_characters_in_url() {
    // Test handling of special characters in URLs
    let mut config = Config::default();
    config.listen_addr = "127.0.0.1:0".parse().unwrap();
    config.api_addr = Some("127.0.0.1:0".parse().unwrap());
    config.data_dir = Some(unique_data_dir("special-chars"));

    let node = Node::new(config).unwrap();

    let special_urls = vec![
        "ely://node/path with spaces",
        "ely://node/path%20with%20encoding",
        "ely://node/path\nwith\nnewlines",
        "ely://node/path\twith\ttabs",
        "ely://node/path/with/../parent",
        "ely://node/path/with/../../root",
    ];

    for url in special_urls {
        let result = node.fetch_content(url, Duration::from_millis(100)).await;
        // Should not panic
        let _ = result;
    }
}
