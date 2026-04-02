#!/bin/bash
# Deploy R2 Relay to a VPS with automatic HTTPS via Caddy.
#
# Prerequisites:
#   - A VPS with a public IP
#   - A domain pointing to that IP (e.g. relay.reality2.ai)
#   - SSH access to the VPS
#
# Usage:
#   ./deploy.sh user@your-server relay.yourdomain.com
#
# This script:
#   1. Builds the relay binary for Linux
#   2. Copies it to the server
#   3. Installs Caddy if not present
#   4. Sets up systemd services for relay + Caddy
#   5. Starts everything with automatic HTTPS

set -e

if [ $# -lt 2 ]; then
    echo "Usage: ./deploy.sh user@server relay.yourdomain.com"
    echo ""
    echo "Example: ./deploy.sh root@203.0.113.45 relay.reality2.ai"
    exit 1
fi

SSH_TARGET="$1"
DOMAIN="$2"
PORT=21042

echo "R2 Relay Deployment"
echo "  Server: $SSH_TARGET"
echo "  Domain: $DOMAIN"
echo ""

# Build for Linux (cross-compile if needed)
echo "Building relay binary..."
if [ "$(uname -s)" = "Linux" ] && [ "$(uname -m)" = "x86_64" ]; then
    cargo build --release
    BINARY="target/release/r2-relay"
else
    # Cross-compile for Linux x86_64
    echo "  Cross-compiling for linux/amd64..."
    rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
    cargo build --release --target x86_64-unknown-linux-gnu
    BINARY="target/x86_64-unknown-linux-gnu/release/r2-relay"
fi
echo "  Built: $(du -h "$BINARY" | cut -f1)"

# Copy binary to server
echo "Copying binary to server..."
scp "$BINARY" "$SSH_TARGET":/tmp/r2-relay

# Set up on server
echo "Setting up on server..."
ssh "$SSH_TARGET" bash -s "$DOMAIN" "$PORT" << 'REMOTE'
DOMAIN="$1"
PORT="$2"

set -e

# Install binary
sudo mv /tmp/r2-relay /usr/local/bin/r2-relay
sudo chmod 755 /usr/local/bin/r2-relay

# Install Caddy if not present
if ! command -v caddy &>/dev/null; then
    echo "Installing Caddy..."
    sudo apt-get update -qq
    sudo apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https curl
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
    sudo apt-get update -qq
    sudo apt-get install -y -qq caddy
fi

# Relay systemd service
sudo tee /etc/systemd/system/r2-relay.service > /dev/null <<EOF
[Unit]
Description=R2 Relay
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/r2-relay --port $PORT --bind 127.0.0.1
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

# Caddyfile
sudo tee /etc/caddy/Caddyfile > /dev/null <<EOF
$DOMAIN {
    reverse_proxy localhost:$PORT
}
EOF

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable --now r2-relay
sudo systemctl restart caddy

echo ""
echo "Done! R2 Relay is running at:"
echo "  wss://$DOMAIN/r2"
echo "  https://$DOMAIN/ (dashboard)"
echo ""
echo "Caddy is handling TLS automatically via Let's Encrypt."
REMOTE

echo ""
echo "============================================"
echo "  Relay deployed!"
echo ""
echo "  WebSocket: wss://$DOMAIN/r2"
echo "  Dashboard: https://$DOMAIN/"
echo ""
echo "  Use in Notekeeper:"
echo "    wss://$DOMAIN/r2"
echo "============================================"
