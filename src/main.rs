mod buffer;
mod protocol;
mod state;
mod ws;

use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::extract::State;
use axum::extract::WebSocketUpgrade;
use axum::http::header;
use axum::response::{Html, IntoResponse, Response};
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

async fn stats_json(State(state): State<Arc<RelayState>>) -> Response {
    let groups = state.groups.read().await;
    let active_connections: usize = groups.values().map(|g| g.connections.len()).sum();
    let trust_groups = groups.len();
    let frames = state.frames_routed.load(Ordering::Relaxed);
    let connections_total = state.connections_total.load(Ordering::Relaxed);
    let uptime_secs = state.started_at.elapsed().as_secs();
    drop(groups);

    let json = format!(
        r#"{{"connections":{},"trust_groups":{},"frames_routed":{},"connections_total":{},"uptime_secs":{}}}"#,
        active_connections, trust_groups, frames, connections_total, uptime_secs
    );

    ([(header::CONTENT_TYPE, "application/json")], json).into_response()
}

async fn dashboard() -> Html<&'static str> {
    Html(include_str!("../static/dashboard.html"))
}

async fn relay_svg() -> Response {
    let svg = include_str!("../static/relay.svg");
    ([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response()
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let state = RelayState::new(args.buffer_size, args.max_connections);

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/r2", get(ws_handler))
        .route("/health", get(health))
        .route("/stats", get(stats_json))
        .route("/relay.svg", get(relay_svg))
        .layer(CorsLayer::permissive())
        .with_state(state.clone());

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port)
        .parse()
        .expect("invalid bind address");

    log::info!("r2-relay listening on {}", addr);
    log::info!("  Dashboard: http://{}:{}/", args.bind, args.port);
    log::info!("  WebSocket: ws://{}:{}/r2", args.bind, args.port);
    log::info!("  Buffer: {} frames/group", args.buffer_size);
    log::info!("  Max connections: {}", args.max_connections);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
