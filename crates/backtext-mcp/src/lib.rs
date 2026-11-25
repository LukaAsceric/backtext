use anyhow::Result;
use backtext_core::*;
use backtext_storage::StorageEngine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientConfigSnippets {
    pub stdio_binary_path: String,
    pub streamable_http_url: String,
    pub claude_desktop_json: String,
    pub cursor_mcp_json: String,
    pub claude_code_cli: String,
}

#[derive(Clone)]
pub struct McpServer {
    storage: StorageEngine,
    events: EventBus,
}

impl McpServer {
    pub fn new(storage: StorageEngine, events: EventBus) -> Self {
        Self { storage, events }
    }

    pub fn client_config_snippets(&self, binary_hint: Option<&str>) -> Result<McpClientConfigSnippets> {
        let policy = self.storage.get_mcp_policy()?;
        let db_path = self.storage.db_path().display().to_string();
        let bin_path = binary_hint
            .map(|s| s.to_string())
            .unwrap_or_else(|| "/Applications/backtext.app/Contents/MacOS/backtext-mcp".to_string());
        let http_url = format!("http://127.0.0.1:{}/mcp", policy.http_port);

        let claude_desktop = json!({
            "mcpServers": {
                "backtext": {
                    "command": bin_path,
                    "args": ["--db", db_path]
                }
            }
        });
        let cursor = json!({
            "mcpServers": {
                "backtext": {
                    "url": http_url
                }
            }
        });

        Ok(McpClientConfigSnippets {
            stdio_binary_path: bin_path.clone(),
            streamable_http_url: http_url.clone(),
            claude_desktop_json: serde_json::to_string_pretty(&claude_desktop)?,
            cursor_mcp_json: serde_json::to_string_pretty(&cursor)?,
            claude_code_cli: format!("claude mcp add --transport http backtext {http_url}"),
        })
    }

    /// Processes a single JSON-RPC 2.0 MCP message. Returns `None` for notifications.
    pub fn handle_jsonrpc(&self, req: &Value, client_name: &str, transport: &str) -> Option<Value> {
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = req.get("id").cloned();

        // JSON-RPC notifications have no `id`
        if id.is_none() || method.starts_with("notifications/") {
            return None;
        }
        let rpc_id = id.unwrap_or(Value::Null);

        let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
        match self.dispatch_method(method, &params, client_name, transport) {
            Ok(result) => Some(json!({
                "jsonrpc": "2.0",
                "id": rpc_id,
                "result": result
            })),
            Err(err) => Some(json!({
                "jsonrpc": "2.0",
                "id": rpc_id,
                "error": {
                    "code": -32603,
                    "message": err.to_string()
                }
            })),
        }
    }

    fn dispatch_method(
        &self,
        method: &str,
        params: &Value,
        client_name: &str,
        transport: &str,
    ) -> Result<Value> {
        match method {
            "initialize" => {
                let requested_version = params
                    .get("protocolVersion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("2025-06-18");
                Ok(json!({
                    "protocolVersion": requested_version,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "subscribe": false, "listChanged": false },
                        "prompts": { "listChanged": true }
                    },
                    "serverInfo": {
                        "name": "backtext",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": "backtext provides ambient desktop work context, speaker-attributed meeting notes, deduplicated screen-sharing OCR, person/project dossiers, and user personalization."
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": Self::tool_definitions() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
                let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                self.execute_tool(name, &args, client_name, transport)
            }
            "resources/list" => Ok(json!({
                "resources": [
                    {
                        "uri": "backtext://context/now",
                        "name": "Live Ambient Workspace Context",
                        "description": "Active focus, recent work episodes, latest meeting, and clipboard snippets.",
                        "mimeType": "text/markdown"
                    },
                    {
                        "uri": "backtext://user/profile",
                        "name": "User Personalization & Glossary Dossier",
                        "description": "User role, style guidelines, active projects, custom vocabulary, and learned preferences.",
                        "mimeType": "text/markdown"
                    },
                    {
                        "uri": "backtext://meetings/recent",
                        "name": "Recent Meeting Summaries & Action Items",
                        "description": "Structured summaries and open commitments from recent calls.",
                        "mimeType": "text/markdown"
                    }
                ]
            })),
            "resources/read" => {
                let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
                let text = match uri {
                    "backtext://context/now" => self.build_live_context_markdown(90)?,
                    "backtext://user/profile" => {
                        let profile = self.storage.get_user_profile()?;
                        let vocab = self.storage.list_vocabulary()?;
                        let learned = self.storage.list_learned_preferences()?;
                        render_personalization_dossier(&profile, &vocab, &learned)
                    }
                    "backtext://meetings/recent" => {
                        let mtgs = self.storage.list_meetings()?;
                        mtgs.iter()
                            .take(5)
                            .map(|m| format!("## {} ({})\n{}\n", m.title, m.started_at.to_rfc3339(), m.executive_summary))
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                    _ => anyhow::bail!("Unknown resource URI: {uri}"),
                };
                Ok(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "text/markdown",
                        "text": text
                    }]
                }))
            }
            "prompts/list" => {
                let recipes = self.storage.list_recipes()?;
                let prompts: Vec<Value> = recipes
                    .into_iter()
                    .map(|r| {
                        let prompt_name = r.slug.trim_start_matches('/').to_string();
                        json!({
                            "name": prompt_name,
                            "description": format!("{} ({})", r.description_de, r.scope.as_str()),
                            "arguments": [
                                {
                                    "name": "meeting_id",
                                    "description": "Optional meeting ID when running a single-meeting recipe",
                                    "required": false
                                }
                            ]
                        })
                    })
                    .collect();
                Ok(json!({ "prompts": prompts }))
            }
            "prompts/get" => {
                let raw_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let slug = if raw_name.starts_with('/') {
                    raw_name.to_string()
                } else {
                    format!("/{raw_name}")
                };
                let recipes = self.storage.list_recipes()?;
                let rec = recipes
                    .into_iter()
                    .find(|r| r.slug.eq_ignore_ascii_case(&slug))
                    .ok_or_else(|| anyhow::anyhow!("Recipe prompt '{slug}' not found"))?;
                let live_ctx = self.build_live_context_markdown(60)?;
                Ok(json!({
                    "description": rec.description_de,
                    "messages": [{
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": format!("{}\n\n---\n{}", rec.prompt_template, live_ctx)
                        }
                    }]
                }))
            }
            _ => anyhow::bail!("Unsupported MCP method: {method}"),
        }
    }

    pub fn execute_tool(
        &self,
        tool_name: &str,
        args: &Value,
        client_name: &str,
        transport: &str,
    ) -> Result<Value> {
        let start = Instant::now();
        let policy = self.storage.get_mcp_policy()?;

        let record_audit = |allowed: bool, items: usize, summary: String| {
            let entry = McpAuditEntry {
                id: format!("aud_{}", uuid::Uuid::new_v4().simple()),
                client_name: client_name.to_string(),
                transport: transport.to_string(),
                tool_name: tool_name.to_string(),
                query_summary: summary,
                items_returned: items,
                duration_ms: start.elapsed().as_millis().max(1) as u64,
                allowed,
                created_at: Utc::now(),
            };
            let _ = self.storage.record_mcp_audit(&entry);
            self.events.publish(BacktextEvent::McpToolInvoked(entry));
        };

        if !policy.enabled {
            record_audit(false, 0, "MCP server disabled by user policy".into());
            return Ok(Self::tool_error("backtext MCP server is currently paused by user policy."));
        }

        match tool_name {
            "get_live_context" => {
                if !policy.allow_live_context {
                    record_audit(false, 0, "Blocked by allow_live_context=false".into());
                    return Ok(Self::tool_error("Live screen context access is disabled in backtext MCP Governance."));
                }
                let lookback_mins = args
                    .get("lookback_minutes")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(90);
                let md = self.build_live_context_markdown(lookback_mins)?;
                record_audit(true, 1, format!("lookback_minutes={lookback_mins}"));
                Ok(Self::tool_text(md))
            }
            "search_memory" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
                let source = args.get("source_kind").and_then(|v| v.as_str());
                let project = args.get("project").and_then(|v| v.as_str());
                let hits = self.storage.search_hybrid(
                    query,
                    limit,
                    source,
                    project,
                    Some(policy.max_history_days),
                )?;
                let count = hits.len();
                let mut md = format!("# backtext Memory Search (`{query}` — {count} Treffer)\n\n");
                for h in &hits {
                    md.push_str(&format!(
                        "- **[{}] {}** (Score `{:.2}`, `{}`)\n  {}\n",
                        h.source_kind,
                        h.title,
                        h.score,
                        h.timestamp.format("%Y-%m-%d %H:%M UTC"),
                        h.snippet
                    ));
                }
                record_audit(true, count, format!("query='{query}'"));
                Ok(Self::tool_text(md))
            }
            "list_meetings" => {
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let meetings: Vec<Meeting> = self
                    .storage
                    .list_meetings()?
                    .into_iter()
                    .take(limit)
                    .collect();
                let count = meetings.len();
                let mut md = String::from("# Meetings in backtext\n\n");
                for m in &meetings {
                    let parts: Vec<String> = m
                        .participants
                        .iter()
                        .map(|p| p.display_name.clone())
                        .collect();
                    md.push_str(&format!(
                        "- **`{}` — {}** (`{}`, Status: `{}`)\n  - *Teilnehmer:* {}\n  - *Summary:* {}\n  - *Action Items:* {}\n",
                        m.id,
                        m.title,
                        m.started_at.format("%Y-%m-%d %H:%M UTC"),
                        m.status.as_str(),
                        parts.join(", "),
                        if m.executive_summary.is_empty() {
                            m.prep_brief.as_deref().unwrap_or("Geplant")
                        } else {
                            m.executive_summary.as_str()
                        },
                        m.action_items.len()
                    ));
                }
                record_audit(true, count, format!("limit={limit}"));
                Ok(Self::tool_text(md))
            }
            "get_meeting_context" => {
                let meeting_id = args.get("meeting_id").and_then(|v| v.as_str()).unwrap_or("");
                let Some(detail) = self.storage.get_meeting_detail(meeting_id)? else {
                    record_audit(true, 0, format!("meeting_id='{meeting_id}' (not found)"));
                    return Ok(Self::tool_error(&format!("Meeting '{meeting_id}' not found.")));
                };
                let m = &detail.meeting;
                let parts = m
                    .participants
                    .iter()
                    .map(|p| format!("{} ({}, {:.0}%)", p.display_name, p.identification_source.as_str(), p.confidence * 100.0))
                    .collect::<Vec<_>>()
                    .join(", ");
                let actions = m
                    .action_items
                    .iter()
                    .map(|a| format!("- [{}] **{}**: {} ({})", if a.completed { "x" } else { " " }, a.owner, a.task, a.due_hint.as_deref().unwrap_or("offen")))
                    .collect::<Vec<_>>()
                    .join("\n");
                let md = format!(
                    "# {}\n- **App:** {}\n- **Teilnehmer:** {}\n\n## Executive Summary\n{}\n\n## Enhanced Notes\n{}\n\n## Eigene Notizen\n{}\n\n## Action Items\n{}",
                    m.title, m.detected_app, parts, m.executive_summary, m.enhanced_markdown, m.raw_user_notes, actions
                );
                record_audit(true, 1, format!("meeting_id='{meeting_id}'"));
                Ok(Self::tool_text(md))
            }
            "get_meeting_transcript" => {
                if !policy.allow_raw_transcripts {
                    record_audit(false, 0, "Blocked by allow_raw_transcripts=false".into());
                    return Ok(Self::tool_error("Raw transcript access is restricted by backtext MCP Governance."));
                }
                let meeting_id = args.get("meeting_id").and_then(|v| v.as_str()).unwrap_or("");
                let speaker_filter = args.get("speaker").and_then(|v| v.as_str());
                let Some(detail) = self.storage.get_meeting_detail(meeting_id)? else {
                    return Ok(Self::tool_error(&format!("Meeting '{meeting_id}' not found.")));
                };
                let segs: Vec<&TranscriptSegment> = detail
                    .transcript
                    .iter()
                    .filter(|s| {
                        speaker_filter
                            .map(|sf| s.speaker_name.to_lowercase().contains(&sf.to_lowercase()))
                            .unwrap_or(true)
                    })
                    .collect();
                let count = segs.len();
                let body = segs
                    .iter()
                    .map(|s| {
                        format!(
                            "[{:02}:{:02} · {}] **{}**: {}",
                            s.start_ms / 60_000,
                            (s.start_ms % 60_000) / 1000,
                            s.channel.as_str(),
                            s.speaker_name,
                            s.text
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                record_audit(true, count, format!("meeting_id='{meeting_id}'"));
                Ok(Self::tool_text(format!(
                    "# Volltranskript — {}\n\n{}",
                    detail.meeting.title, body
                )))
            }
            "get_meeting_screenshots" => {
                if !policy.allow_screenshare_ocr {
                    record_audit(false, 0, "Blocked by allow_screenshare_ocr=false".into());
                    return Ok(Self::tool_error("Screen-share OCR access is disabled by policy."));
                }
                let meeting_id = args.get("meeting_id").and_then(|v| v.as_str()).unwrap_or("");
                let Some(detail) = self.storage.get_meeting_detail(meeting_id)? else {
                    return Ok(Self::tool_error(&format!("Meeting '{meeting_id}' not found.")));
                };
                let count = detail.screenshares.len();
                let body = detail
                    .screenshares
                    .iter()
                    .map(|f| {
                        format!(
                            "### {:02}:{:02} — {} (Präsentiert von: {}, dHash `{}`)\n{}",
                            f.captured_at_ms / 60_000,
                            (f.captured_at_ms % 60_000) / 1000,
                            f.slide_title,
                            f.presenter_name.as_deref().unwrap_or("Unbekannt"),
                            f.dhash_hex,
                            f.ocr_text
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                record_audit(true, count, format!("meeting_id='{meeting_id}'"));
                Ok(Self::tool_text(format!(
                    "# Screen-Sharing-Folien & OCR — {}\n\n{}",
                    detail.meeting.title, body
                )))
            }
            "get_person_dossier" => {
                let name_q = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let persons = self.storage.list_persons()?;
                let matched: Vec<&Person> = persons
                    .iter()
                    .filter(|p| p.full_name.to_lowercase().contains(&name_q.to_lowercase()))
                    .collect();
                let count = matched.len();
                let mut md = format!("# Personen-Dossier (`{name_q}`)\n\n");
                for p in matched {
                    md.push_str(&format!(
                        "## {} ({})\n- **E-Mail:** {}\n- **Rolle & Kontext:** {}\n- **Voiceprint-Status:** {} Stimmproben ({:.0}% Konfidenz)\n- **Offene Zusagen:**\n{}\n\n",
                        p.full_name,
                        p.company.as_deref().unwrap_or("Extern"),
                        p.email.as_deref().unwrap_or("—"),
                        p.role_notes,
                        p.voiceprint_samples,
                        p.voiceprint_confidence * 100.0,
                        p.open_commitments
                            .iter()
                            .map(|c| format!("  - {c}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
                record_audit(true, count, format!("name='{name_q}'"));
                Ok(Self::tool_text(md))
            }
            "get_project_context" => {
                let project = args.get("project").and_then(|v| v.as_str()).unwrap_or("backtext");
                let hits = self.storage.search_hybrid(project, 12, None, Some(project), None)?;
                let count = hits.len();
                let body = hits
                    .iter()
                    .map(|h| format!("- **[{}] {}**: {}", h.source_kind, h.title, h.snippet))
                    .collect::<Vec<_>>()
                    .join("\n");
                record_audit(true, count, format!("project='{project}'"));
                Ok(Self::tool_text(format!(
                    "# Projekt-Dossier: `{project}`\n\n{body}"
                )))
            }
            "get_user_profile" => {
                let profile = self.storage.get_user_profile()?;
                let vocab = self.storage.list_vocabulary()?;
                let learned = self.storage.list_learned_preferences()?;
                let md = render_personalization_dossier(&profile, &vocab, &learned);
                record_audit(true, 1, "Personalization profile requested".into());
                Ok(Self::tool_text(md))
            }
            "remember_agent_insight" => {
                if !policy.allow_agent_writeback {
                    record_audit(false, 0, "Blocked by allow_agent_writeback=false".into());
                    return Ok(Self::tool_error("Agent write-back is disabled in backtext settings."));
                }
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Agent Insight");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let project = args.get("project").and_then(|v| v.as_str());
                let ep = ContextEpisode {
                    id: format!("ep_agent_{}", uuid::Uuid::new_v4().simple()),
                    started_at: Utc::now(),
                    ended_at: Utc::now(),
                    title: format!("[Agent Note] {title}"),
                    summary: content.to_string(),
                    primary_apps: vec![client_name.to_string()],
                    urls: vec![],
                    project_tag: project.map(|s| s.to_string()),
                    importance_score: 0.90,
                    snapshot_count: 1,
                };
                self.storage.upsert_context_episode(&ep)?;
                self.events.publish(BacktextEvent::ContextEpisodeUpdated(ep.clone()));
                record_audit(true, 1, format!("Saved insight '{title}'"));
                Ok(Self::tool_text(format!(
                    "Saved insight `{}` (`{}`) to backtext memory.",
                    ep.title, ep.id
                )))
            }
            _ => {
                record_audit(false, 0, format!("Unknown tool '{tool_name}'"));
                Ok(Self::tool_error(&format!("Unknown tool: {tool_name}")))
            }
        }
    }

    pub fn build_live_context_markdown(&self, _lookback_mins: i64) -> Result<String> {
        let profile = self.storage.get_user_profile()?;
        let policy = self.storage.get_mcp_policy()?;
        let snapshots = self.storage.list_recent_snapshots(4)?;
        let episodes = self.storage.list_context_episodes(4)?;
        let meetings = self.storage.list_meetings()?;

        let mut md = format!(
            "# backtext Live Context (Nutzer: {}, Rolle: {})\n\n",
            profile.display_name, profile.role
        );

        if let Some(front) = snapshots.first() {
            md.push_str(&format!(
                "## Aktiver Fokus (Frontmost App)\n- **App & Fenster:** {} — `{}`\n",
                front.app_name, front.window_title
            ));
            if let Some(ref url) = front.browser_url {
                md.push_str(&format!("- **Aktive URL:** {url}\n"));
            }
            md.push_str(&format!("- **Sichtbarer Kontext:** {}\n\n", front.extracted_text));
        }

        if !episodes.is_empty() {
            md.push_str("## Jüngste Arbeits-Episoden\n");
            for ep in &episodes {
                md.push_str(&format!(
                    "- **{}** ({}): {}\n",
                    ep.title,
                    ep.primary_apps.join(", "),
                    ep.summary
                ));
            }
            md.push('\n');
        }

        if policy.allow_clipboard {
            let clips = self.storage.list_clipboard_items(3)?;
            if !clips.is_empty() {
                md.push_str("## Relevante Zwischenablage (gefiltert)\n");
                for c in &clips {
                    md.push_str(&format!("- `[{} · {}]` {}\n", c.source_app, c.content_kind, c.text_preview));
                }
                md.push('\n');
            }
        }

        if let Some(latest_mtg) = meetings.iter().find(|m| m.status == MeetingStatus::Completed || m.status == MeetingStatus::Live) {
            md.push_str(&format!(
                "## Letztes / Aktives Meeting: {}\n- **Zusammenfassung:** {}\n",
                latest_mtg.title, latest_mtg.executive_summary
            ));
        }

        Ok(md)
    }

    fn tool_text(text: String) -> Value {
        json!({
            "content": [{ "type": "text", "text": text }],
            "isError": false
        })
    }

    fn tool_error(msg: &str) -> Value {
        json!({
            "content": [{ "type": "text", "text": msg }],
            "isError": true
        })
    }

    pub fn tool_definitions() -> Vec<Value> {
        vec![
            json!({
                "name": "get_live_context",
                "description": "Returns what the user is actively working on right now: frontmost window/tab, compacted recent work episodes, filtered clipboard snippets, and latest meeting summary.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "lookback_minutes": { "type": "integer", "description": "Lookback horizon in minutes (default: 90)" }
                    }
                }
            }),
            json!({
                "name": "search_memory",
                "description": "Hybrid BM25 full-text + semantic vector search across meetings, transcripts, screen-sharing OCR, work episodes, and clipboard.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Natural language or keyword query" },
                        "source_kind": { "type": "string", "description": "Optional filter: all | meeting | transcript | screenshare | episode | screen_context | clipboard" },
                        "project": { "type": "string", "description": "Optional project tag filter" },
                        "limit": { "type": "integer", "description": "Max results (default: 8)" }
                    },
                    "required": ["query"]
                }
            }),
            json!({
                "name": "list_meetings",
                "description": "Lists recent and upcoming meetings with attendees, executive summary, and action item count.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "Max meetings to list (default: 10)" }
                    }
                }
            }),
            json!({
                "name": "get_meeting_context",
                "description": "Retrieves structured Granola-style enhanced notes, user notes, identified participants, and action items for a meeting.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "meeting_id": { "type": "string", "description": "Meeting ID (e.g. from list_meetings)" }
                    },
                    "required": ["meeting_id"]
                }
            }),
            json!({
                "name": "get_meeting_transcript",
                "description": "Retrieves the speaker-attributed transcript of a meeting (optional speaker filter).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "meeting_id": { "type": "string" },
                        "speaker": { "type": "string", "description": "Optional speaker name filter" }
                    },
                    "required": ["meeting_id"]
                }
            }),
            json!({
                "name": "get_meeting_screenshots",
                "description": "Retrieves deduplicated screen-sharing slide titles and Apple Vision OCR text captured during a meeting.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "meeting_id": { "type": "string" }
                    },
                    "required": ["meeting_id"]
                }
            }),
            json!({
                "name": "get_person_dossier",
                "description": "Returns known context about a person: role, company, voiceprint status, and open commitments.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Person name or partial name" }
                    },
                    "required": ["name"]
                }
            }),
            json!({
                "name": "get_project_context",
                "description": "Bundles meetings, screen episodes, URLs, and notes associated with a project.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project name" }
                    },
                    "required": ["project"]
                }
            }),
            json!({
                "name": "get_user_profile",
                "description": "Returns the user's role, preferred output language, writing style, custom vocabulary glossary, and learned working preferences.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            json!({
                "name": "remember_agent_insight",
                "description": "Writes an agent summary, finding, or follow-up back into backtext memory so future agents and meetings have it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "content": { "type": "string" },
                        "project": { "type": "string" }
                    },
                    "required": ["title", "content"]
                }
            }),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_jsonrpc_handshake_tools_and_audit_log_work_end_to_end() {
        let store = StorageEngine::open_in_memory().unwrap();
        let bus = EventBus::default();
        let mcp = McpServer::new(store.clone(), bus);

        let init_resp = mcp
            .handle_jsonrpc(
                &json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": { "protocolVersion": "2025-06-18" }
                }),
                "claude-desktop",
                "stdio",
            )
            .unwrap();
        assert_eq!(init_resp["result"]["serverInfo"]["name"], "backtext");

        let list_resp = mcp
            .handle_jsonrpc(
                &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
                "claude-desktop",
                "stdio",
            )
            .unwrap();
        assert_eq!(list_resp["result"]["tools"].as_array().unwrap().len(), 10);

        let live_resp = mcp
            .handle_jsonrpc(
                &json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": "get_live_context",
                        "arguments": { "lookback_minutes": 60 }
                    }
                }),
                "cursor-agent",
                "http",
            )
            .unwrap();
        let text = live_resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("backtext Live Context"));

        // Verify governance enforcement when transcripts are disabled
        let mut pol = store.get_mcp_policy().unwrap();
        pol.allow_raw_transcripts = false;
        store.save_mcp_policy(&pol).unwrap();

        let blocked_resp = mcp
            .handle_jsonrpc(
                &json!({
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "tools/call",
                    "params": {
                        "name": "get_meeting_transcript",
                        "arguments": { "meeting_id": "mtg_arch_sync" }
                    }
                }),
                "cursor-agent",
                "http",
            )
            .unwrap();
        assert_eq!(blocked_resp["result"]["isError"], true);

        let audits = store.list_mcp_audit_logs(10).unwrap();
        assert_eq!(audits.len(), 2);
        assert!(!audits[0].allowed);
        assert!(audits[1].allowed);
    }
}
