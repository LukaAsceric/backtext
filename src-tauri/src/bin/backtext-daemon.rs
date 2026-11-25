use anyhow::Result;
use backtext_desktop_lib::{
    controller::AppController, daemon_loop::spawn_background_daemon,
    http_server::spawn_local_http_and_mcp_server,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let controller = AppController::open_default()?;
    spawn_background_daemon(controller.clone());

    let port = std::env::var("BACKTEXT_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .or_else(|| controller.storage.get_mcp_policy().ok().map(|p| p.http_port))
        .unwrap_or(39421);

    println!(
        "backtext-daemon v{} starting on http://127.0.0.1:{port} (MCP at /mcp, SSE at /api/events, encrypted={})",
        env!("CARGO_PKG_VERSION"),
        controller.storage.is_encrypted()
    );
    spawn_local_http_and_mcp_server(controller, port).await
}
