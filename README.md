# Relay

<p align="center">
  <img src="static/relay.svg" width="96" alt="Relay">
</p>

<p align="center">
  Your own connectivity, on your own terms.<br>
  Part of <a href="https://reality2-ai.github.io">Reality2</a>.
</p>

The relay connects your devices to each other across the internet - without depending on anyone else's servers or services.

**The relay never sees your data.** It forwards encrypted messages between devices that belong to the same trust group. Think of it as a postal service that carries sealed envelopes - it knows where to deliver them, but can't read what's inside.

## Getting Started

You need the relay running somewhere if you want your devices to find each other across the internet (e.g. your laptop at home and your phone on mobile data). If all your devices are on the same local network, you don't need a relay.

### Install (Linux or macOS)

One script handles everything - builds the binary, installs it, and sets it up to start automatically on boot:

```
git clone https://github.com/reality2-ai/r2-relay.git
cd r2-relay
./install.sh
```

If Rust isn't installed, the script installs it for you.

On **Linux**, it creates a systemd service. On **macOS**, it creates a launchd agent. Either way, the relay starts immediately and restarts automatically if it stops.

To remove everything (service + binary):

```
./install.sh --remove
```

### Just build and run (no service)

If you'd rather run it manually:

```
git clone https://github.com/reality2-ai/r2-relay.git
cd r2-relay
cargo run --release
```

### Run on a server

For the relay to be always available (so your devices can sync even when your computer is off), run it on a cheap server - a $5/month VPS, a Raspberry Pi, or any always-on machine:

```
git clone https://github.com/reality2-ai/r2-relay.git
cd r2-relay
./install.sh
```

The install script works the same way on a server as on your laptop.

### Checking it's working

Open a browser and go to `http://<your-ip>:21042`. You'll see the relay dashboard - a live view showing connections, trust groups, and frames being routed. The hexagon pulses each time a message passes through.

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
- **Your data is encrypted** before it reaches the relay - the relay can't read it
- **If the relay restarts**, devices reconnect automatically within seconds
- **If the relay goes down**, your devices still work locally - they just can't reach each other across the internet until it's back

## Options

```
r2-relay [OPTIONS]

  --port <PORT>             Port to listen on [default: 21042]
  --bind <ADDR>             Bind address [default: 0.0.0.0]
  --buffer-size <N>         Recent messages to keep per trust group [default: 1000]
  --max-connections <N>     Maximum simultaneous connections [default: 10000]
```

## Community relay

There is a public community relay available for anyone to use:

```
wss://relay.reality2.ai/r2
```

This relay is untrusted by design - it forwards encrypted bytes and cannot read your data. Use it to get started without setting up your own. You can switch to your own relay at any time.

## Deploy to a VPS

To run your own relay on the internet with automatic HTTPS:

```
./deploy.sh root@your-server relay.yourdomain.com
```

This builds the relay, copies it to your server, installs Caddy for automatic TLS, and sets up systemd services. Your relay will be available at `wss://relay.yourdomain.com/r2`.

Requirements: a VPS with a public IP and a domain pointing to it.

A **Dockerfile** is also included for container deployments.

## For developers

The relay implements the R2-TRANSPORT-RELAY specification. It's a single Rust binary with minimal dependencies (axum, tokio, ed25519-dalek). No R2 protocol crates required - it treats all messages as opaque encrypted bytes.

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
