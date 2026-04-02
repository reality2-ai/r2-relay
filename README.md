# R2 Relay

The relay connects your devices to each other across the internet. It's part of [Reality2](https://reality2-ai.github.io) — a protocol for private, encrypted communication between your devices.

**The relay never sees your data.** It forwards encrypted messages between devices that belong to the same trust group. Think of it as a postal service that carries sealed envelopes — it knows where to deliver them, but can't read what's inside.

## Getting Started

You need the relay running somewhere if you want your devices to find each other across the internet (e.g. your laptop at home and your phone on mobile data). If all your devices are on the same local network, you don't need a relay.

### Option 1: Use a pre-built binary

Download the latest release for your platform from the [Releases](https://github.com/reality2-ai/r2-relay/releases) page.

Then run it:

```
./r2-relay
```

That's it. The relay is now running on port 21042. Any R2-enabled tool (like [Notekeeper](https://github.com/reality2-ai/r2-notekeeper)) can connect to it at `ws://your-machine:21042/r2`.

### Option 2: Build from source

You'll need [Rust](https://rustup.rs) installed (the installer is one command on any platform).

```
git clone https://github.com/reality2-ai/r2-relay.git
cd r2-relay
cargo run --release
```

### Option 3: Run on a server

If you want the relay always available (so your devices can sync even when your computer is off), run it on a cheap server — a $5/month VPS, a Raspberry Pi, or any always-on machine.

1. Build the binary: `cargo build --release`
2. Copy `target/release/r2-relay` to the server
3. Run it: `./r2-relay --port 21042`

To keep it running after you log out, use systemd (Linux) or any process manager:

```ini
# /etc/systemd/system/r2-relay.service
[Unit]
Description=R2 Relay
After=network.target

[Service]
ExecStart=/usr/local/bin/r2-relay --port 21042
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Then:
```
sudo cp target/release/r2-relay /usr/local/bin/
sudo systemctl enable r2-relay
sudo systemctl start r2-relay
```

### Checking it's working

Open a browser and go to `http://your-machine:21042/health`. You should see `r2-relay ok`.

## Using the relay

Once the relay is running, you give its address to the R2 tools you use. For example, in [Notekeeper](https://github.com/reality2-ai/r2-notekeeper), you enter the relay URL in Settings:

```
ws://your-machine:21042/r2
```

If your relay is on the internet with a domain name and TLS (via nginx or caddy):

```
wss://relay.yourdomain.com/r2
```

## How it works

The relay is deliberately simple. When a device connects, it says which trust group it belongs to. The relay puts it in a room with all other devices from the same trust group. Messages sent by any device in the room are forwarded to every other device in that room.

- **Multiple trust groups** share one relay without seeing each other
- **Your data is encrypted** before it reaches the relay — the relay can't read it
- **If the relay restarts**, devices reconnect automatically within seconds
- **If the relay goes down**, your devices still work locally — they just can't reach each other across the internet until it's back

## Options

```
r2-relay [OPTIONS]

  --port <PORT>             Port to listen on [default: 21042]
  --bind <ADDR>             Bind address [default: 0.0.0.0]
  --buffer-size <N>         Recent messages to keep per trust group [default: 1000]
  --max-connections <N>     Maximum simultaneous connections [default: 10000]
```

## For developers

The relay implements the R2-TRANSPORT-RELAY specification. It's a single Rust binary with minimal dependencies (axum, tokio, ed25519-dalek). No R2 protocol crates required — it treats all messages as opaque encrypted bytes.

**Protocol:** JSON handshake (HELLO/WELCOME) with Ed25519 signature verification, then binary WebSocket frames forwarded by trust group hash. Heartbeat timeout at 90 seconds.

**TLS:** The relay itself doesn't handle TLS. Put nginx or caddy in front for HTTPS/WSS:

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
