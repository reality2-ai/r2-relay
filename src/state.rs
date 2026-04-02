use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};

use crate::buffer::RingBuffer;

/// Trust group hash: first 8 bytes of SHA-256(TG_PK).
pub type TrustGroupHash = [u8; 8];

/// Unique connection ID (monotonic).
pub type ConnId = u64;

/// A connected device.
#[allow(dead_code)]
pub struct Connection {
    pub conn_id: ConnId,
    pub device_id: String,
    pub tx: mpsc::UnboundedSender<Vec<u8>>,
    pub connected_at: Instant,
}

/// State for a single trust group.
pub struct TrustGroupState {
    pub connections: HashMap<ConnId, Connection>,
    pub buffer: RingBuffer,
}

impl TrustGroupState {
    pub fn new(buffer_size: usize) -> Self {
        TrustGroupState {
            connections: HashMap::new(),
            buffer: RingBuffer::new(buffer_size),
        }
    }

    /// Forward a frame to all connections except the sender.
    pub fn broadcast(&self, sender_id: ConnId, frame: &[u8]) {
        for (id, conn) in &self.connections {
            if *id != sender_id {
                let _ = conn.tx.send(frame.to_vec());
            }
        }
    }

    /// Number of connected peers.
    pub fn peer_count(&self) -> usize {
        self.connections.len()
    }
}

/// Global relay state.
pub struct RelayState {
    pub groups: RwLock<HashMap<TrustGroupHash, TrustGroupState>>,
    next_conn_id: AtomicU64,
    pub buffer_size: usize,
    pub max_connections: usize,
    /// Total frames routed since startup.
    pub frames_routed: AtomicU64,
    /// Total connections accepted since startup.
    pub connections_total: AtomicU64,
    /// Startup time.
    pub started_at: Instant,
}

impl RelayState {
    pub fn new(buffer_size: usize, max_connections: usize) -> Arc<Self> {
        Arc::new(RelayState {
            groups: RwLock::new(HashMap::new()),
            next_conn_id: AtomicU64::new(1),
            buffer_size,
            max_connections,
            frames_routed: AtomicU64::new(0),
            connections_total: AtomicU64::new(0),
            started_at: Instant::now(),
        })
    }

    pub fn next_conn_id(&self) -> ConnId {
        self.next_conn_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Total connections across all trust groups.
    pub async fn total_connections(&self) -> usize {
        let groups = self.groups.read().await;
        groups.values().map(|g| g.connections.len()).sum()
    }
}
