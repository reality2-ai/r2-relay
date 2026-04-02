use serde::{Deserialize, Serialize};

/// Messages sent by the client (device/browser) to the relay.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "hello")]
    Hello {
        version: u32,
        trust_group: String,
        device_id: String,
        timestamp: u64,
        signature: String,
    },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "catchup")]
    Catchup { since: u64 },
}

/// Messages sent by the relay to the client.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "welcome")]
    Welcome {
        version: u32,
        peers: usize,
        buffer_oldest: u64,
    },
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "catchup_incomplete")]
    CatchupIncomplete { oldest: u64 },
}

/// WebSocket close codes per R2-TRANSPORT-RELAY §3.5.
#[allow(dead_code)]
pub const CLOSE_AUTH_FAILED: u16 = 4401;
#[allow(dead_code)]
pub const CLOSE_BANNED: u16 = 4403;
#[allow(dead_code)]
pub const CLOSE_HEARTBEAT_TIMEOUT: u16 = 4408;
#[allow(dead_code)]
pub const CLOSE_TOO_MANY: u16 = 4429;
