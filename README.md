# r2-relay

Transport relay for the [Reality2](https://reality2-ai.github.io) mesh protocol. Routes encrypted R2-WIRE frames between devices via WebSocket, indexed by trust group hash.

## What it does

The relay is **untrusted infrastructure**. It sits below the trust layer — it never decrypts, never parses frame payloads, never verifies trust group membership. It simply:

1. Accepts WebSocket connections
2. Validates device identity (Ed25519 signature in HELLO handshake)
3. Associates each connection with a trust group hash
4. Forwards binary frames to all other connections with the same trust group hash
5. Buffers recent frames for catchup on reconnect

Multiple trust groups share one relay without seeing each other's traffic.

## Quick start

```bash
cargo install --path .
r2-relay --port 21042
```

Or build and run directly:

```bash
cargo run --release -- --port 21042
```

The relay listens for WebSocket connections at `ws://host:21042/r2`.

## Options

```
r2-relay [OPTIONS]

Options:
  --port <PORT>              Port to listen on [default: 21042]
  --bind <ADDR>              Bind address [default: 0.0.0.0]
  --buffer-size <N>          Event buffer per trust group [default: 1000]
  --max-connections <N>      Maximum total connections [default: 10000]
```

## Protocol

Per the [R2-TRANSPORT-RELAY](https://reality2-ai.github.io) specification:

**Handshake:**
- Client sends JSON HELLO with trust group hash, device ID, timestamp, and Ed25519 signature
- Relay verifies signature and timestamp (60-second window)
- Relay responds with JSON WELCOME including peer count

**After handshake:**
- Binary WebSocket messages are R2-WIRE frames (opaque, forwarded as-is)
- Text messages are control: `ping`/`pong`, `catchup`
- Connections closed after 90 seconds without activity (code 4408)

**Close codes:**
| Code | Meaning |
|------|---------|
| 4401 | Authentication failed |
| 4408 | Heartbeat timeout |
| 4429 | Too many connections |

## Dependencies

Minimal — no R2 protocol crates required:

- `axum` + `tokio` — WebSocket server
- `ed25519-dalek` — HELLO signature verification
- `clap` — CLI arguments

## Deployment

The relay is a single static binary. Deploy with systemd, Docker, or any process manager.

```ini
# /etc/systemd/system/r2-relay.service
[Unit]
Description=R2 Transport Relay
After=network.target

[Service]
ExecStart=/usr/local/bin/r2-relay --port 21042
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

For TLS, put nginx or caddy in front:

```nginx
location /r2 {
    proxy_pass http://127.0.0.1:21042;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
}
```

## Licence

MIT OR Apache-2.0
