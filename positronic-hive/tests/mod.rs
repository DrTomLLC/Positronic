use positronic_hive::{HiveEvent, HiveNode, Peer};

// ============================================================================
// HiveNode Tests
// ============================================================================

#[test]
fn test_hive_node_creation() {
    let (node, _rx) = HiveNode::new("TestUser");
    assert_eq!(node.local_peer.name, "TestUser");
    assert!(node.local_peer.id.starts_with("TestUser-"));
    assert_eq!(node.local_peer.address, "127.0.0.1");
}

#[test]
fn test_hive_node_capabilities() {
    let (node, _rx) = HiveNode::new("Test");
    assert!(node.local_peer.capabilities.contains(&"terminal-sharing".to_string()));
    assert!(node.local_peer.capabilities.contains(&"file-transfer".to_string()));
}

#[test]
fn test_hive_node_unique_ids() {
    let (node1, _rx1) = HiveNode::new("User1");
    let (node2, _rx2) = HiveNode::new("User2");
    assert_ne!(node1.local_peer.id, node2.local_peer.id);
}

#[test]
fn test_hive_node_debug() {
    let (node, _rx) = HiveNode::new("DebugTest");
    let debug = format!("{:?}", node);
    assert!(debug.contains("HiveNode"));
    assert!(debug.contains("DebugTest"));
}

#[tokio::test]
async fn test_hive_broadcast_block() {
    let (node, mut rx) = HiveNode::new("Broadcaster");
    let data = b"Hello Mesh!".to_vec();

    node.broadcast_block(data.clone()).await.unwrap();

    let event = rx.recv().await.unwrap();
    match event {
        HiveEvent::BlockReceived { from, content } => {
            assert!(from.starts_with("Broadcaster"));
            assert_eq!(content, b"Hello Mesh!");
        }
        _ => panic!("Expected BlockReceived event"),
    }
}

#[tokio::test]
async fn test_hive_broadcast_empty_block_fails() {
    let (node, _rx) = HiveNode::new("EmptyTest");
    let result = node.broadcast_block(vec![]).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[tokio::test]
async fn test_hive_broadcast_large_block() {
    let (node, mut rx) = HiveNode::new("LargeTest");
    let data = vec![0xAB; 10_000]; // 10KB block

    node.broadcast_block(data.clone()).await.unwrap();

    let event = rx.recv().await.unwrap();
    match event {
        HiveEvent::BlockReceived { content, .. } => {
            assert_eq!(content.len(), 10_000);
        }
        _ => panic!("Expected BlockReceived event"),
    }
}

#[tokio::test]
async fn test_hive_join_session_valid() {
    let (node, _rx) = HiveNode::new("Joiner");
    let result = node.join_session("session-12345").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_hive_join_session_too_short() {
    let (node, _rx) = HiveNode::new("Joiner");
    let result = node.join_session("ab").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid"));
}

#[tokio::test]
async fn test_hive_start_discovery() {
    let (node, _rx) = HiveNode::new("DiscoveryTest");
    let result = node.start_discovery().await;
    assert!(result.is_ok());
    // Clean shutdown
    node.shutdown().await;
}

#[tokio::test]
async fn test_hive_shutdown() {
    let (node, _rx) = HiveNode::new("ShutdownTest");
    node.start_discovery().await.unwrap();
    node.shutdown().await;
    // Should not panic
}

// ============================================================================
// Peer Tests
// ============================================================================

#[test]
fn test_peer_creation() {
    let peer = Peer {
        id: "test-123".to_string(),
        name: "Alice".to_string(),
        address: "192.168.1.100".to_string(),
        capabilities: vec!["chat".to_string()],
        last_seen: 1700000000,
    };
    assert_eq!(peer.name, "Alice");
    assert_eq!(peer.capabilities.len(), 1);
}

#[test]
fn test_peer_clone() {
    let peer = Peer {
        id: "clone-test".to_string(),
        name: "Bob".to_string(),
        address: "10.0.0.1".to_string(),
        capabilities: vec![],
        last_seen: 0,
    };
    let cloned = peer.clone();
    assert_eq!(peer.id, cloned.id);
    assert_eq!(peer.name, cloned.name);
}

#[test]
fn test_peer_serialization() {
    let peer = Peer {
        id: "serde-test".to_string(),
        name: "Serde".to_string(),
        address: "127.0.0.1".to_string(),
        capabilities: vec!["a".to_string(), "b".to_string()],
        last_seen: 42,
    };
    let json = serde_json::to_string(&peer).unwrap();
    let deserialized: Peer = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "serde-test");
    assert_eq!(deserialized.capabilities.len(), 2);
}

// ============================================================================
// HiveEvent Tests
// ============================================================================

#[test]
fn test_hive_event_peer_discovered() {
    let event = HiveEvent::PeerDiscovered {
        peer_id: "id-1".to_string(),
        name: "Alice".to_string(),
    };
    match event {
        HiveEvent::PeerDiscovered { peer_id, name } => {
            assert_eq!(peer_id, "id-1");
            assert_eq!(name, "Alice");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hive_event_peer_lost() {
    let event = HiveEvent::PeerLost {
        peer_id: "id-2".to_string(),
    };
    assert!(matches!(event, HiveEvent::PeerLost { .. }));
}

#[test]
fn test_hive_event_block_received() {
    let event = HiveEvent::BlockReceived {
        from: "sender".to_string(),
        content: vec![1, 2, 3],
    };
    match event {
        HiveEvent::BlockReceived { from, content } => {
            assert_eq!(from, "sender");
            assert_eq!(content, vec![1, 2, 3]);
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hive_event_live_session_invite() {
    let event = HiveEvent::LiveSessionInvite {
        from: "host".to_string(),
        session_id: "sess-abc".to_string(),
    };
    match event {
        HiveEvent::LiveSessionInvite { from, session_id } => {
            assert_eq!(from, "host");
            assert_eq!(session_id, "sess-abc");
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hive_event_error() {
    let event = HiveEvent::Error("network down".to_string());
    match event {
        HiveEvent::Error(msg) => assert_eq!(msg, "network down"),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hive_event_clone() {
    let event = HiveEvent::PeerDiscovered {
        peer_id: "x".to_string(),
        name: "y".to_string(),
    };
    let cloned = event.clone();
    assert!(matches!(cloned, HiveEvent::PeerDiscovered { .. }));
}

#[test]
fn test_hive_event_serialization() {
    let event = HiveEvent::Error("test error".to_string());
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: HiveEvent = serde_json::from_str(&json).unwrap();
    assert!(matches!(deserialized, HiveEvent::Error(_)));
}
