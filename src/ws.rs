use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{CloseFrame, Message, WebSocket};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use tokio::sync::mpsc;

use crate::protocol::*;
use crate::state::*;

const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(90);
const TIMESTAMP_WINDOW: u64 = 60;

/// Handle a single WebSocket connection through its full lifecycle.
pub async fn handle_connection(mut socket: WebSocket, state: Arc<RelayState>) {
    // Phase 1: Handshake — wait for HELLO, verify, respond with WELCOME.
    let (trust_group, device_id, conn_id) = match handshake(&mut socket, &state).await {
        Some(result) => result,
        None => return, // handshake failed, socket already closed
    };

    state.connections_total.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    log::info!(
        "[{}] device {} joined tg:{}",
        conn_id,
        &device_id[..16],
        hex::encode(&trust_group)
    );

    // Phase 2: Frame routing.
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Register connection.
    {
        let mut groups = state.groups.write().await;
        let tg = groups
            .entry(trust_group)
            .or_insert_with(|| TrustGroupState::new(state.buffer_size));
        tg.connections.insert(
            conn_id,
            Connection {
                conn_id,
                device_id: device_id.clone(),
                tx,
                connected_at: Instant::now(),
            },
        );
    }

    let mut last_activity = Instant::now();

    loop {
        tokio::select! {
            // Receive from WebSocket (client → relay)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Binary(data))) => {
                        last_activity = Instant::now();
                        let now_secs = now_unix();
                        let mut groups = state.groups.write().await;
                        if let Some(tg) = groups.get_mut(&trust_group) {
                            tg.buffer.push(data.to_vec(), now_secs);
                            tg.broadcast(conn_id, &data);
                            state.frames_routed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        last_activity = Instant::now();
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(ClientMessage::Ping) => {
                                let pong = serde_json::to_string(&ServerMessage::Pong).unwrap();
                                if socket.send(Message::Text(pong.into())).await.is_err() {
                                    break;
                                }
                            }
                            Ok(ClientMessage::Catchup { since }) => {
                                let groups = state.groups.read().await;
                                if let Some(tg) = groups.get(&trust_group) {
                                    if since < tg.buffer.oldest_timestamp() && tg.buffer.len() > 0 {
                                        let msg = serde_json::to_string(&ServerMessage::CatchupIncomplete {
                                            oldest: tg.buffer.oldest_timestamp(),
                                        }).unwrap();
                                        let _ = socket.send(Message::Text(msg.into())).await;
                                    }
                                    for frame in tg.buffer.since(since) {
                                        if socket.send(Message::Binary(frame.data.clone().into())).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            _ => {} // ignore unknown or repeated HELLO
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {} // ping/pong handled by axum
                }
            }

            // Send to WebSocket (relay → client, from other connections)
            frame = rx.recv() => {
                match frame {
                    Some(data) => {
                        if socket.send(Message::Binary(data.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break, // channel closed
                }
            }

            // Heartbeat timeout
            _ = tokio::time::sleep(HEARTBEAT_TIMEOUT) => {
                if last_activity.elapsed() >= HEARTBEAT_TIMEOUT {
                    log::warn!("[{}] heartbeat timeout", conn_id);
                    let _ = socket.send(Message::Close(Some(CloseFrame {
                        code: CLOSE_HEARTBEAT_TIMEOUT,
                        reason: "heartbeat timeout".into(),
                    }))).await;
                    break;
                }
            }
        }
    }

    // Cleanup: remove connection from trust group.
    {
        let mut groups = state.groups.write().await;
        if let Some(tg) = groups.get_mut(&trust_group) {
            tg.connections.remove(&conn_id);
            log::info!(
                "[{}] device {} left tg:{} ({} peers remaining)",
                conn_id,
                &device_id[..16.min(device_id.len())],
                hex::encode(&trust_group),
                tg.connections.len()
            );
            // Clean up empty trust groups.
            if tg.connections.is_empty() {
                groups.remove(&trust_group);
            }
        }
    }
}

/// Perform the HELLO/WELCOME handshake.
///
/// Returns (trust_group_hash, device_id_hex, conn_id) on success, None on failure.
async fn handshake(
    socket: &mut WebSocket,
    state: &Arc<RelayState>,
) -> Option<(TrustGroupHash, String, ConnId)> {
    // Wait for HELLO (with timeout).
    let hello = tokio::time::timeout(Duration::from_secs(10), socket.recv()).await;

    let hello_text = match hello {
        Ok(Some(Ok(Message::Text(text)))) => text.to_string(),
        _ => {
            close_with(socket, CLOSE_AUTH_FAILED, "expected HELLO").await;
            return None;
        }
    };

    let msg: ClientMessage = match serde_json::from_str(&hello_text) {
        Ok(msg) => msg,
        Err(_) => {
            close_with(socket, CLOSE_AUTH_FAILED, "malformed HELLO").await;
            return None;
        }
    };

    let (version, trust_group_hex, device_id_hex, timestamp, signature_hex) = match msg {
        ClientMessage::Hello {
            version,
            trust_group,
            device_id,
            timestamp,
            signature,
        } => (version, trust_group, device_id, timestamp, signature),
        _ => {
            close_with(socket, CLOSE_AUTH_FAILED, "expected HELLO").await;
            return None;
        }
    };

    if version != 1 {
        close_with(socket, CLOSE_AUTH_FAILED, "unsupported version").await;
        return None;
    }

    // Verify timestamp is within window.
    let now = now_unix();
    if timestamp > now + TIMESTAMP_WINDOW || now > timestamp + TIMESTAMP_WINDOW {
        close_with(socket, CLOSE_AUTH_FAILED, "timestamp out of range").await;
        return None;
    }

    // Verify Ed25519 signature.
    let device_pk_bytes = match hex_decode(&device_id_hex) {
        Some(b) if b.len() == 32 => b,
        _ => {
            close_with(socket, CLOSE_AUTH_FAILED, "invalid device_id").await;
            return None;
        }
    };

    let sig_bytes = match hex_decode(&signature_hex) {
        Some(b) if b.len() == 64 => b,
        _ => {
            close_with(socket, CLOSE_AUTH_FAILED, "invalid signature").await;
            return None;
        }
    };

    let vk = match VerifyingKey::from_bytes(device_pk_bytes[..32].try_into().unwrap()) {
        Ok(vk) => vk,
        Err(_) => {
            close_with(socket, CLOSE_AUTH_FAILED, "invalid public key").await;
            return None;
        }
    };

    let sig = match Signature::from_bytes(sig_bytes[..64].try_into().unwrap()) {
        sig => sig,
    };

    // Message to verify: "trust_group:device_id:timestamp"
    let msg_to_verify = format!("{}:{}:{}", trust_group_hex, device_id_hex, timestamp);
    if vk.verify(msg_to_verify.as_bytes(), &sig).is_err() {
        close_with(socket, CLOSE_AUTH_FAILED, "signature verification failed").await;
        return None;
    }

    // Parse trust group hash (exact 8 bytes or 3-hex-char prefix).
    let tg_hash_bytes = if trust_group_hex.len() == 16 {
        // Exact match (standard HELLO)
        match hex_decode(&trust_group_hex) {
            Some(b) if b.len() == 8 => {
                let mut h = [0u8; 8];
                h.copy_from_slice(&b);
                h
            }
            _ => {
                close_with(socket, CLOSE_AUTH_FAILED, "invalid trust_group").await;
                return None;
            }
        }
    } else if trust_group_hex.len() >= 3 && trust_group_hex.len() <= 6 {
        // Prefix match (word code HELLO) - find matching active group
        let prefix = &trust_group_hex;
        let groups = state.groups.read().await;
        let matches: Vec<TrustGroupHash> = groups.keys()
            .filter(|h| hex::encode(*h).starts_with(prefix))
            .copied()
            .collect();
        drop(groups);

        match matches.len() {
            0 => {
                log::warn!("prefix {} matched no active trust groups", prefix);
                close_with(socket, CLOSE_AUTH_FAILED, "no matching trust group").await;
                return None;
            }
            1 => matches[0],
            _ => {
                log::warn!("prefix {} matched {} trust groups (ambiguous)", prefix, matches.len());
                close_with(socket, CLOSE_AUTH_FAILED, "ambiguous trust group prefix").await;
                return None;
            }
        }
    } else {
        close_with(socket, CLOSE_AUTH_FAILED, "invalid trust_group length").await;
        return None;
    };

    // Check connection limits.
    if state.total_connections().await >= state.max_connections {
        close_with(socket, CLOSE_TOO_MANY, "too many connections").await;
        return None;
    }

    // Note: rate limiting is checked per-IP at the transport layer.
    // For WebSocket upgrades without IP extraction, the rate limit
    // is applied by the reverse proxy (nginx/caddy) or can be added
    // here when axum's ConnectInfo extractor is wired in.

    let conn_id = state.next_conn_id();

    // Send WELCOME.
    let peers = {
        let groups = state.groups.read().await;
        groups.get(&tg_hash_bytes).map(|g| g.peer_count()).unwrap_or(0)
    };
    let buffer_oldest = {
        let groups = state.groups.read().await;
        groups
            .get(&tg_hash_bytes)
            .map(|g| g.buffer.oldest_timestamp())
            .unwrap_or(0)
    };

    let welcome = serde_json::to_string(&ServerMessage::Welcome {
        version: 1,
        peers,
        buffer_oldest,
    })
    .unwrap();

    if socket.send(Message::Text(welcome.into())).await.is_err() {
        return None;
    }

    Some((tg_hash_bytes, device_id_hex, conn_id))
}

async fn close_with(socket: &mut WebSocket, code: u16, reason: &str) {
    log::warn!("closing connection: {} ({})", reason, code);
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
