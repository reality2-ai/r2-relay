#!/bin/bash
# Install R2 Relay binary and auto-start service.
# Supports Linux (systemd) and macOS (launchd).
#
# Usage:
#   cd r2-relay
#   ./install.sh            # build, install, start on boot
#   ./install.sh --remove   # stop and remove service + binary

set -e

USER_NAME="$(whoami)"
USER_HOME="$HOME"
PORT="${R2_RELAY_PORT:-21042}"
BIND="${R2_RELAY_BIND:-0.0.0.0}"
OS="$(uname -s)"
INSTALL_DIR="/usr/local/bin"
BINARY="target/release/r2-relay"

echo "R2 Relay installer"
echo "User: $USER_NAME"
echo "Platform: $OS"
echo "Port: $PORT"
echo ""

# ── Remove ──

if [ "${1:-}" = "--remove" ]; then
    echo "Removing R2 Relay..."

    if [ "$OS" = "Darwin" ]; then
        PLIST_FILE="$USER_HOME/Library/LaunchAgents/ai.reality2.relay.plist"
        launchctl bootout "gui/$(id -u)/ai.reality2.relay" 2>/dev/null || true
        rm -f "$PLIST_FILE"
        echo "  launchd service removed"
    else
        if command -v systemctl &>/dev/null; then
            sudo systemctl stop r2-relay 2>/dev/null || true
            sudo systemctl disable r2-relay 2>/dev/null || true
            sudo rm -f /etc/systemd/system/r2-relay.service
            sudo systemctl daemon-reload
            echo "  systemd service removed"
        fi
    fi

    sudo rm -f "$INSTALL_DIR/r2-relay"
    echo "  Binary removed"
    echo ""
    echo "Done."
    exit 0
fi

# ── Check Rust ──

if ! command -v cargo &>/dev/null; then
    echo "Rust not found — installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
fi
echo "Rust: $(rustc --version)"
echo ""

# ── Build ──

echo "Building release binary..."
cargo build --release
echo "  Built: $BINARY ($(du -h "$BINARY" | cut -f1))"
echo ""

# ── Install binary ──

# Stop service if running (can't overwrite binary while in use).
if [ "$OS" != "Darwin" ] && command -v systemctl &>/dev/null; then
    if systemctl is-active --quiet r2-relay 2>/dev/null; then
        echo "Stopping existing service..."
        sudo systemctl stop r2-relay
        sleep 1
    fi
fi

echo "Installing binary to $INSTALL_DIR..."
if [ ! -d "$INSTALL_DIR" ]; then
    sudo mkdir -p "$INSTALL_DIR"
fi
sudo cp "$BINARY" "$INSTALL_DIR/r2-relay"
sudo chmod 755 "$INSTALL_DIR/r2-relay"

# ── Platform service ──

if [ "$OS" = "Darwin" ]; then
    # macOS — launchd
    PLIST_NAME="ai.reality2.relay"
    PLIST_DIR="$USER_HOME/Library/LaunchAgents"
    PLIST_FILE="$PLIST_DIR/$PLIST_NAME.plist"
    LOG_DIR="$USER_HOME/Library/Logs/r2-relay"

    echo "Installing launchd service..."
    mkdir -p "$PLIST_DIR" "$LOG_DIR"

    cat > "$PLIST_FILE" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${PLIST_NAME}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${INSTALL_DIR}/r2-relay</string>
        <string>--port</string>
        <string>${PORT}</string>
        <string>--bind</string>
        <string>${BIND}</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>${LOG_DIR}/relay.log</string>
    <key>StandardErrorPath</key>
    <string>${LOG_DIR}/relay.err</string>
</dict>
</plist>
EOF

    launchctl bootout "gui/$(id -u)/$PLIST_NAME" 2>/dev/null || true
    launchctl bootstrap "gui/$(id -u)" "$PLIST_FILE"

    echo ""
    echo "============================================"
    echo "  R2 Relay installed successfully! (macOS)"
    echo "============================================"
    echo ""
    echo "  Binary:    $INSTALL_DIR/r2-relay"
    echo "  Service:   $PLIST_FILE"
    echo "  Logs:      tail -f $LOG_DIR/relay.log"
    echo ""
    echo "  Service commands:"
    echo "    launchctl kickstart gui/$(id -u)/$PLIST_NAME   # restart"
    echo "    launchctl kill SIGTERM gui/$(id -u)/$PLIST_NAME # stop"
    echo "    launchctl bootout gui/$(id -u)/$PLIST_NAME     # unload"
    echo ""

else
    # Linux — systemd
    echo "Installing systemd service..."

    sudo tee /etc/systemd/system/r2-relay.service > /dev/null <<EOF
[Unit]
Description=R2 Relay
After=network.target
Documentation=https://github.com/reality2-ai/r2-relay

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/r2-relay --port ${PORT} --bind ${BIND}
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable --now r2-relay

    echo ""
    echo "============================================"
    echo "  R2 Relay installed successfully! (Linux)"
    echo "============================================"
    echo ""
    echo "  Binary:    $INSTALL_DIR/r2-relay"
    echo "  Service:   /etc/systemd/system/r2-relay.service"
    echo "  Logs:      journalctl -u r2-relay -f"
    echo ""
    echo "  Service commands:"
    echo "    sudo systemctl status r2-relay    # check status"
    echo "    sudo systemctl restart r2-relay   # restart"
    echo "    sudo systemctl stop r2-relay      # stop"
    echo ""
fi

# ── Done ──

echo "  ──────────────────────────────────"
echo ""
echo "  Dashboard:  http://localhost:${PORT}/"
echo "  WebSocket:  ws://localhost:${PORT}/r2"
echo ""
echo "  To use with Notekeeper, enter this relay address:"
echo "    ws://$(hostname):${PORT}/r2"
echo ""
echo "  To remove:  ./install.sh --remove"
echo ""
