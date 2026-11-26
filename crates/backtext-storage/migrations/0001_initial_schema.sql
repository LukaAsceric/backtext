CREATE TABLE IF NOT EXISTS kv_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_profile (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    email TEXT,
    role TEXT NOT NULL,
    organization TEXT NOT NULL,
    ui_language TEXT NOT NULL DEFAULT 'de',
    preferred_output_language TEXT NOT NULL DEFAULT 'de',
    writing_style TEXT NOT NULL,
    tone_instructions TEXT NOT NULL,
    active_projects_json TEXT NOT NULL DEFAULT '[]',
    self_voiceprint_blob BLOB,
    auto_record_meetings INTEGER NOT NULL DEFAULT 1,
    consent_reminder_enabled INTEGER NOT NULL DEFAULT 1,
    retention_days INTEGER NOT NULL DEFAULT 90,
    meeting_content_width TEXT NOT NULL DEFAULT 'standard',
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS custom_vocabulary (
    id TEXT PRIMARY KEY,
    term TEXT NOT NULL UNIQUE,
    phonetic_hint TEXT,
    category TEXT NOT NULL,
    project_tag TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS learned_preferences (
    id TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    fact_text TEXT NOT NULL,
    confidence REAL NOT NULL,
    source_ref TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS persons (
    id TEXT PRIMARY KEY,
    full_name TEXT NOT NULL,
    email TEXT,
    company TEXT,
    role_notes TEXT NOT NULL DEFAULT '',
    voiceprint_blob BLOB,
    voiceprint_samples INTEGER NOT NULL DEFAULT 0,
    voiceprint_confidence REAL NOT NULL DEFAULT 0.0,
    open_commitments_json TEXT NOT NULL DEFAULT '[]',
    last_seen_at TEXT
);

CREATE TABLE IF NOT EXISTS meetings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    calendar_event_id TEXT,
    detected_app TEXT NOT NULL,
    project_tag TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    status TEXT NOT NULL,
    template_id TEXT NOT NULL DEFAULT 'general',
    prep_brief TEXT,
    raw_user_notes TEXT NOT NULL DEFAULT '',
    executive_summary TEXT NOT NULL DEFAULT '',
    enhanced_markdown TEXT NOT NULL DEFAULT '',
    action_items_json TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS meeting_participants (
    meeting_id TEXT NOT NULL,
    speaker_label TEXT NOT NULL,
    display_name TEXT NOT NULL,
    person_id TEXT,
    email TEXT,
    is_self INTEGER NOT NULL DEFAULT 0,
    identification_source TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.0,
    talk_time_ms INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (meeting_id, speaker_label)
);

CREATE TABLE IF NOT EXISTS transcript_segments (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    speaker_label TEXT NOT NULL,
    speaker_name TEXT NOT NULL,
    person_id TEXT,
    channel TEXT NOT NULL,
    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL,
    text TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.95
);

CREATE INDEX IF NOT EXISTS idx_transcript_meeting ON transcript_segments(meeting_id, start_ms);

CREATE TABLE IF NOT EXISTS screenshare_frames (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    captured_at_ms INTEGER NOT NULL,
    presenter_name TEXT,
    dhash_hex TEXT NOT NULL,
    image_data_url TEXT,
    ocr_text TEXT NOT NULL,
    slide_title TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_screenshare_meeting ON screenshare_frames(meeting_id, captured_at_ms);

CREATE TABLE IF NOT EXISTS context_episodes (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    primary_apps_json TEXT NOT NULL DEFAULT '[]',
    urls_json TEXT NOT NULL DEFAULT '[]',
    project_tag TEXT,
    importance_score REAL NOT NULL DEFAULT 0.5,
    snapshot_count INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_episodes_started ON context_episodes(started_at DESC);

CREATE TABLE IF NOT EXISTS context_snapshots (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    app_bundle_id TEXT NOT NULL,
    app_name TEXT NOT NULL,
    window_title TEXT NOT NULL,
    browser_url TEXT,
    dwell_ms INTEGER NOT NULL DEFAULT 3000,
    dhash_hex TEXT NOT NULL,
    extracted_text TEXT NOT NULL,
    extraction_method TEXT NOT NULL,
    relevance_score REAL NOT NULL,
    project_tag TEXT,
    redacted_secrets_count INTEGER NOT NULL DEFAULT 0,
    episode_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_snapshots_captured ON context_snapshots(captured_at DESC);

CREATE TABLE IF NOT EXISTS clipboard_items (
    id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    source_app TEXT NOT NULL,
    content_kind TEXT NOT NULL,
    text_preview TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    relevance_score REAL NOT NULL,
    project_tag TEXT
);

CREATE INDEX IF NOT EXISTS idx_clipboard_captured ON clipboard_items(captured_at DESC);

CREATE TABLE IF NOT EXISTS recipes (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name_de TEXT NOT NULL,
    name_en TEXT NOT NULL,
    description_de TEXT NOT NULL,
    description_en TEXT NOT NULL,
    icon_key TEXT NOT NULL DEFAULT 'sparkles',
    scope TEXT NOT NULL,
    prompt_template TEXT NOT NULL,
    preferred_model TEXT,
    is_builtin INTEGER NOT NULL DEFAULT 0,
    is_favorite INTEGER NOT NULL DEFAULT 0,
    usage_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    meeting_id TEXT,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    recipe_slug TEXT,
    sources_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chat_thread ON chat_messages(thread_id, created_at);

CREATE TABLE IF NOT EXISTS privacy_rules (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    pattern TEXT NOT NULL,
    description TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    is_builtin INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS mcp_audit_logs (
    id TEXT PRIMARY KEY,
    client_name TEXT NOT NULL,
    transport TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    query_summary TEXT NOT NULL,
    items_returned INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    allowed INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mcp_audit_created ON mcp_audit_logs(created_at DESC);

-- Unified Hybrid Search Index (FTS5 BM25 + 256-dim Vector Store)
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    doc_id UNINDEXED,
    source_kind UNINDEXED,
    title,
    body,
    project_tag,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE IF NOT EXISTS memory_vectors (
    doc_id TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,
    title TEXT NOT NULL,
    snippet TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    project_tag TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    embedding_blob BLOB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_vectors_ts ON memory_vectors(timestamp DESC);

-- v0.5.0 Granola Parity Tables (Templates, Folders, Auto-Series & Voice Typing History)
CREATE TABLE IF NOT EXISTS meeting_templates (
    id TEXT PRIMARY KEY,
    name_de TEXT NOT NULL,
    name_en TEXT NOT NULL,
    description_de TEXT NOT NULL DEFAULT '',
    description_en TEXT NOT NULL DEFAULT '',
    icon_key TEXT NOT NULL DEFAULT 'file-text',
    sections_prompt TEXT NOT NULL,
    is_builtin INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meeting_folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT 'sage',
    description TEXT NOT NULL DEFAULT '',
    auto_series_keys_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meeting_folder_links (
    meeting_id TEXT NOT NULL,
    folder_id TEXT NOT NULL,
    PRIMARY KEY (meeting_id, folder_id)
);

CREATE INDEX IF NOT EXISTS idx_folder_links_folder ON meeting_folder_links(folder_id);

CREATE TABLE IF NOT EXISTS dictation_history (
    id TEXT PRIMARY KEY,
    raw_text TEXT NOT NULL,
    cleaned_text TEXT NOT NULL,
    target_app TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    pasted INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_dictation_created ON dictation_history(created_at DESC);
