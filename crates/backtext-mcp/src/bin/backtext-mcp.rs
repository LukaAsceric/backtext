use anyhow::Result;
use backtext_core::EventBus;
use backtext_mcp::McpServer;
use backtext_storage::StorageEngine;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

fn parse_db_arg() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for window in args.windows(2) {
        if window[0] == "--db" {
            return PathBuf::from(&window[1]);
        }
    }
    if let Ok(env_db) = std::env::var("BACKTEXT_DB_PATH") {
        return PathBuf::from(env_db);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".backtext-data")
        .join("backtext.db")
}

fn main() -> Result<()> {
    let db_path = parse_db_arg();
    let storage = StorageEngine::open(db_path)?;
    let mcp = McpServer::new(storage, EventBus::default());

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        if let Some(resp) = mcp.handle_jsonrpc(&req, "mcp-stdio-client", "stdio") {
            let encoded = serde_json::to_string(&resp)?;
            writeln!(stdout, "{encoded}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
