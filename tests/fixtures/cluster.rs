//! Test Cluster Fixtures
//!
//! Mock cluster for integration testing.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use super::{ActorState, Message, TestActorConfig};

/// Test node in the cluster
#[derive(Debug)]
pub struct TestNode {
    /// Node ID
    pub id: String,
    /// Node address
    pub address: String,
    /// Actors on this node
    actors: Arc<RwLock<HashMap<String, ActorInfo>>>,
    /// Whether this node is the leader
    pub is_leader: bool,
}

/// Actor information
#[derive(Debug, Clone)]
pub struct ActorInfo {
    /// Actor ID
    pub id: String,
    /// Actor state
    pub state: ActorState,
    /// Actor config
    pub config: TestActorConfig,
}

impl TestNode {
    /// Create a new test node
    pub fn new(id: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            address: address.into(),
            actors: Arc::new(RwLock::new(HashMap::new())),
            is_leader: false,
        }
    }

    /// Get peer node addresses
    pub async fn get_peers(&self) -> Result<Vec<String>, TestError> {
        // Return mock peer addresses
        Ok(vec![
            "127.0.0.1:7001".to_string(),
            "127.0.0.1:7002".to_string(),
        ])
    }

    /// Get the QUIC endpoint (mock for testing)
    pub fn endpoint(&self) -> &str {
        &self.address
    }

    /// Deploy an actor to this node
    pub async fn deploy_actor(&self, _wat: &str) -> Result<String, TestError> {
        let config = TestActorConfig::default();
        let actor_id = config.id.clone();

        let info = ActorInfo {
            id: actor_id.clone(),
            state: ActorState::Creating,
            config,
        };

        self.actors.write().await.insert(actor_id.clone(), info);

        // Simulate startup
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Transition to running
        if let Some(actor) = self.actors.write().await.get_mut(&actor_id) {
            actor.state = ActorState::Running;
        }

        Ok(actor_id)
    }

    /// Get actor status
    pub async fn get_actor_status(&self, actor_id: &str) -> Result<ActorInfo, TestError> {
        self.actors
            .read()
            .await
            .get(actor_id)
            .cloned()
            .ok_or_else(|| TestError::ActorNotFound(actor_id.to_string()))
    }

    /// Stop an actor
    pub async fn stop_actor(&self, actor_id: &str) -> Result<(), TestError> {
        let mut actors = self.actors.write().await;
        if let Some(actor) = actors.get_mut(actor_id) {
            actor.state = ActorState::Stopped;
            Ok(())
        } else {
            Err(TestError::ActorNotFound(actor_id.to_string()))
        }
    }

    /// Send a message to an actor
    pub async fn send_message(&self, _actor_id: &str, _message: Message) -> Result<(), TestError> {
        // Simulate message processing
        tokio::time::sleep(Duration::from_millis(1)).await;
        Ok(())
    }

    /// Get actor count
    pub async fn actor_count(&self) -> usize {
        self.actors.read().await.len()
    }
}

/// Test cluster
#[derive(Debug)]
pub struct TestCluster {
    /// Nodes in the cluster
    nodes: Vec<TestNode>,
    /// Cluster size
    size: usize,
    /// Whether the cluster is running
    running: bool,
}

impl TestCluster {
    /// Create a new test cluster
    pub async fn new(size: usize) -> Result<Self, TestError> {
        let mut nodes = Vec::with_capacity(size);

        for i in 0..size {
            let mut node = TestNode::new(format!("node-{i}"), format!("127.0.0.1:{}", 7000 + i));
            node.is_leader = i == 0;
            nodes.push(node);
        }

        Ok(Self {
            nodes,
            size,
            running: false,
        })
    }

    /// Start the cluster
    pub async fn start(&mut self) -> Result<(), TestError> {
        // Simulate cluster startup
        tokio::time::sleep(Duration::from_millis(50)).await;
        self.running = true;
        Ok(())
    }

    /// Stop the cluster
    pub async fn stop(&mut self) -> Result<(), TestError> {
        self.running = false;
        Ok(())
    }

    /// Get the leader node
    pub fn leader(&self) -> &TestNode {
        self.nodes
            .iter()
            .find(|n| n.is_leader)
            .unwrap_or(&self.nodes[0])
    }

    /// Get all nodes
    pub fn nodes(&self) -> &[TestNode] {
        &self.nodes
    }

    /// Get cluster size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Check if cluster is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Wait for cluster to be ready
    pub async fn wait_for_cluster_ready(&self, _timeout: Duration) -> Result<(), TestError> {
        // Simulate waiting for cluster readiness
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    }

    /// Get a node by index or ID
    pub fn get_node(&self, id: impl AsRef<str>) -> Option<&TestNode> {
        let id_str = id.as_ref();
        // Try parsing as numeric index first
        if let Ok(index) = id_str.parse::<usize>() {
            return self.nodes.get(index);
        }
        // Try matching by node ID (e.g., "node-0")
        self.nodes.iter().find(|n| n.id == id_str)
    }
}

/// Test error type
#[derive(Debug, thiserror::Error)]
pub enum TestError {
    /// Actor not found
    #[error("actor not found: {0}")]
    ActorNotFound(String),
    /// Cluster error
    #[error("cluster error: {0}")]
    ClusterError(String),
    /// Timeout
    #[error("operation timed out")]
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cluster_creation() {
        let cluster = TestCluster::new(3).await.unwrap();
        assert_eq!(cluster.size(), 3);
        assert!(!cluster.is_running());
    }

    #[tokio::test]
    async fn test_cluster_start_stop() {
        let mut cluster = TestCluster::new(1).await.unwrap();
        cluster.start().await.unwrap();
        assert!(cluster.is_running());
        cluster.stop().await.unwrap();
        assert!(!cluster.is_running());
    }

    #[tokio::test]
    async fn test_actor_deployment() {
        let cluster = TestCluster::new(1).await.unwrap();
        let node = cluster.leader();

        let actor_id = node.deploy_actor("(module)").await.unwrap();
        let status = node.get_actor_status(&actor_id).await.unwrap();
        assert_eq!(status.state, ActorState::Running);
    }
}
