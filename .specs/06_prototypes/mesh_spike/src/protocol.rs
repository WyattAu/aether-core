use serde::{Deserialize, Serialize};

pub const MESH_PORT: u16 = 7050;
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshMessage {
    Ping { node_id: u64, timestamp: u64 },
    Pong { node_id: u64, timestamp: u64 },
    Gossip { payload: Vec<u8> },
    StateSync { state_id: u64, data: Vec<u8> },
    CapabilityRequest { cap_id: u64, requester: u64 },
    CapabilityGrant { cap_id: u64, token: Vec<u8> },
}

impl MeshMessage {
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(bincode::serialize(self)?)
    }

    pub fn decode(data: &[u8]) -> anyhow::Result<Self> {
        Ok(bincode::deserialize(data)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: u64,
    pub listen_addr: String,
    pub capabilities: Vec<String>,
}
