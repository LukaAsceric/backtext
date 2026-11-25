use crate::controller::AppController;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use futures_util::stream::Stream;
use serde_json::{json, Value};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};

pub fn build_http_router(controller: AppController) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_handler))
        .route("/mcp", post(mcp_http_handler).get(mcp_info_handler))
        .route("/api/events", get(sse_events_handler))
        .route("/api/invoke/:cmd", post(rpc_invoke_handler))
        .layer(cors)
        .with_state(controller)
}

pub async fn spawn_local_http_and_mcp_server(controller: AppController, port: u16) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let app = build_http_router(controller);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("backtext local MCP, SSE & API bridge listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler(State(controller): State<AppController>) -> Json<Value> {
    let tel = controller.get_telemetry();
    Json(json!({
        "status": "ok",
        "service": "backtext-daemon",
        "version": env!("CARGO_PKG_VERSION"),
        "mcp_endpoint": "/mcp",
        "events_endpoint": "/api/events",
        "db_encrypted": tel.db_encrypted,
        "loop_running": tel.loop_running
    }))
}

async fn mcp_info_handler() -> Json<Value> {
    Json(json!({
        "name": "backtext",
        "protocol": "Model Context Protocol (Streamable HTTP)",
        "version": env!("CARGO_PKG_VERSION"),
        "tools_count": 10
    }))
}

async fn sse_events_handler(
    State(controller): State<AppController>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = controller.events.subscribe();
    let stream = async_stream_from_broadcast(rx);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

fn async_stream_from_broadcast(
    rx: tokio::sync::broadcast::Receiver<backtext_core::BacktextEvent>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if let Ok(json_str) = serde_json::to_string(&ev) {
                        return Some((Ok(Event::default().data(json_str)), rx));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    })
}

async fn mcp_http_handler(
    State(controller): State<AppController>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let client_name = headers
        .get("x-mcp-client-name")
        .or_else(|| headers.get("user-agent"))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("mcp-http-agent");

    match controller
        .mcp
        .handle_jsonrpc(&payload, client_name, "streamable-http")
    {
        Some(resp) => (StatusCode::OK, Json(resp)).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

async fn rpc_invoke_handler(
    State(controller): State<AppController>,
    Path(cmd): Path<String>,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    match controller.dispatch_rpc(&cmd, args).await {
        Ok(data) => (StatusCode::OK, Json(json!({ "ok": true, "data": data }))),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": err.to_string() })),
        ),
    }
}
