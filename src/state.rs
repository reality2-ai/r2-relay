use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex, RwLock};

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

/// Rate limit entry per IP.
struct RateEntry {
    count: u32,
    window_start: Instant,
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
    /// Rate limiting per IP (max 5 connections per minute).
    rate_limits: Mutex<HashMap<IpAddr, RateEntry>>,
}

/// Max connection attempts per IP per window.
const RATE_LIMIT_MAX: u32 = 5;
/// Rate limit window in seconds.
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

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
            rate_limits: Mutex::new(HashMap::new()),
        })
    }

    /// Check rate limit for an IP. Returns true if allowed, false if rate limited.
    pub async fn check_rate_limit(&self, ip: IpAddr) -> bool {
        let mut limits = self.rate_limits.lock().await;
        let now = Instant::now();

        let entry = limits.entry(ip).or_insert(RateEntry {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(entry.window_start).as_secs() >= RATE_LIMIT_WINDOW_SECS {
            entry.count = 0;
            entry.window_start = now;
        }

        entry.count += 1;
        entry.count <= RATE_LIMIT_MAX
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
