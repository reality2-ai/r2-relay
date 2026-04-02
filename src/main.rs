mod buffer;
mod protocol;
mod state;
mod ws;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use clap::Parser;
use tower_http::cors::CorsLayer;

use state::RelayState;

#[derive(Parser)]
#[command(name = "r2-relay", about = "R2 Transport Relay — routes opaque R2-WIRE frames by trust group hash")]
struct Args {
    /// Port to listen on.
    #[arg(long, default_value = "21042")]
    port: u16,

    /// Bind address.
    #[arg(long, default_value = "0.0.0.0")]
    bind: String,

    /// Event buffer size per trust group.
    #[arg(long, default_value = "1000")]
    buffer_size: usize,

    /// Maximum total connections.
    #[arg(long, default_value = "10000")]
    max_connections: usize,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws::handle_connection(socket, state))
}

async fn health() -> &'static str {
    "r2-relay ok"
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let state = RelayState::new(args.buffer_size, args.max_connections);

    let app = Router::new()
        .route("/r2", get(ws_handler))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port)
        .parse()
        .expect("invalid bind address");

    log::info!("r2-relay listening on {}", addr);
    log::info!("  WebSocket: ws://{}:{}/r2", args.bind, args.port);
    log::info!("  Buffer: {} frames/group", args.buffer_size);
    log::info!("  Max connections: {}", args.max_connections);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
