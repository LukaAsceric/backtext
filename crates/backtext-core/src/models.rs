use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    De,
    En,
}

impl Default for Language {
    fn default() -> Self {
        Self::De
    }
}

impl Language {
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::De => "de",
            Self::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code.trim().to_lowercase().as_str() {
            "en" | "en-us" | "en-gb" => Self::En,
            _ => Self::De,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CaptureStatus {
    Active,
    RecordingMeeting {
        meeting_id: String,
        meeting_title: String,
        started_at: DateTime<Utc>,
    },
    Paused {
        until: Option<DateTime<Utc>>,
        reason: Option<String>,
    },
}

impl Default for CaptureStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub organization: String,
    pub ui_language: Language,
    pub preferred_output_language: Language,
    pub writing_style: String,
    pub tone_instructions: String,
    pub active_projects: Vec<String>,
    pub has_self_voiceprint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_voiceprint: Option<Vec<f32>>,
    pub auto_record_meetings: bool,
    pub consent_reminder_enabled: bool,
    pub retention_days: u32,
    pub updated_at: DateTime<Utc>,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            id: "default_user".to_string(),
            display_name: "Luka".to_string(),
            email: None,
            role: "Product & AI Builder".to_string(),
            organization: "Independent Studio".to_string(),
            ui_language: Language::De,
            preferred_output_language: Language::De,
            writing_style: "Prägnant, strukturiert, entscheidungs- und aktionsorientiert".to_string(),
            tone_instructions: "Keine Floskeln. Hebe konkrete Entscheidungen, Risiken, Zahlen und Verantwortlichkeiten klar hervor.".to_string(),
            active_projects: vec!["backtext".to_string(), "AI Agent Workflows".to_string()],
            has_self_voiceprint: false,
            self_voiceprint: None,
            auto_record_meetings: true,
            consent_reminder_enabled: true,
            retention_days: 90,
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabularyEntry {
    pub id: String,
    pub term: String,
    pub phonetic_hint: Option<String>,
    pub category: String,
    pub project_tag: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedPreference {
    pub id: String,
    pub category: String,
    pub fact_text: String,
    pub confidence: f32,
    pub source_ref: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub full_name: String,
    pub email: Option<String>,
    pub company: Option<String>,
    pub role_notes: String,
    pub voiceprint_samples: u32,
    pub voiceprint_confidence: f32,
    pub meeting_count: u32,
    pub open_commitments: Vec<String>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingStatus {
    Scheduled,
    Candidate,
    Live,
    Completed,
}

impl MeetingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Candidate => "candidate",
            Self::Live => "live",
            Self::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "scheduled" => Self::Scheduled,
            "candidate" => Self::Candidate,
            "live" => Self::Live,
            _ => Self::Completed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationSource {
    MicSelf,
    Voiceprint,
    VisualActiveSpeaker,
    CalendarPrior,
    Manual,
    UnidentifiedCluster,
}

impl IdentificationSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MicSelf => "mic_self",
            Self::Voiceprint => "voiceprint",
            Self::VisualActiveSpeaker => "visual_active_speaker",
            Self::CalendarPrior => "calendar_prior",
            Self::Manual => "manual",
            Self::UnidentifiedCluster => "unidentified_cluster",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "mic_self" => Self::MicSelf,
            "voiceprint" => Self::Voiceprint,
            "visual_active_speaker" => Self::VisualActiveSpeaker,
            "calendar_prior" => Self::CalendarPrior,
            "manual" => Self::Manual,
            _ => Self::UnidentifiedCluster,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioChannel {
    Microphone,
    System,
}

impl AudioChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Microphone => "mic",
            Self::System => "system",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "mic" | "microphone" => Self::Microphone,
            _ => Self::System,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub id: String,
    pub owner: String,
    pub task: String,
    pub due_hint: Option<String>,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingParticipant {
    pub speaker_label: String,
    pub display_name: String,
    pub person_id: Option<String>,
    pub email: Option<String>,
    pub is_self: bool,
    pub identification_source: IdentificationSource,
    pub confidence: f32,
    pub talk_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub meeting_id: String,
    pub speaker_label: String,
    pub speaker_name: String,
    pub person_id: Option<String>,
    pub channel: AudioChannel,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenShareFrame {
    pub id: String,
    pub meeting_id: String,
    pub captured_at_ms: u64,
    pub presenter_name: Option<String>,
    pub dhash_hex: String,
    pub image_data_url: Option<String>,
    pub ocr_text: String,
    pub slide_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub calendar_event_id: Option<String>,
    pub detected_app: String,
    pub project_tag: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub status: MeetingStatus,
    pub template_id: String,
    pub prep_brief: Option<String>,
    pub raw_user_notes: String,
    pub executive_summary: String,
    pub enhanced_markdown: String,
    pub action_items: Vec<ActionItem>,
    pub participants: Vec<MeetingParticipant>,
    pub transcript_count: usize,
    pub screenshare_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingDetail {
    pub meeting: Meeting,
    pub transcript: Vec<TranscriptSegment>,
    pub screenshares: Vec<ScreenShareFrame>,
    pub chat_messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMethod {
    Accessibility,
    VisionOcr,
}

impl ExtractionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::VisionOcr => "vision_ocr",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "vision_ocr" => Self::VisionOcr,
            _ => Self::Accessibility,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub id: String,
    pub captured_at: DateTime<Utc>,
    pub app_bundle_id: String,
    pub app_name: String,
    pub window_title: String,
    pub browser_url: Option<String>,
    pub dwell_ms: u64,
    pub dhash_hex: String,
    pub extracted_text: String,
    pub extraction_method: ExtractionMethod,
    pub relevance_score: f32,
    pub project_tag: Option<String>,
    pub redacted_secrets_count: u32,
    pub episode_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEpisode {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub title: String,
    pub summary: String,
    pub primary_apps: Vec<String>,
    pub urls: Vec<String>,
    pub project_tag: Option<String>,
    pub importance_score: f32,
    pub snapshot_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    pub captured_at: DateTime<Utc>,
    pub source_app: String,
    pub content_kind: String,
    pub text_preview: String,
    pub content_hash: String,
    pub relevance_score: f32,
    pub project_tag: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeScope {
    SingleMeeting,
    MultiMeeting,
    AmbientWorkspace,
}

impl RecipeScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SingleMeeting => "single_meeting",
            Self::MultiMeeting => "multi_meeting",
            Self::AmbientWorkspace => "ambient_workspace",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "multi_meeting" => Self::MultiMeeting,
            "ambient_workspace" => Self::AmbientWorkspace,
            _ => Self::SingleMeeting,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub slug: String,
    pub name_de: String,
    pub name_en: String,
    pub description_de: String,
    pub description_en: String,
    pub icon_key: String,
    pub scope: RecipeScope,
    pub prompt_template: String,
    pub preferred_model: Option<String>,
    pub is_builtin: bool,
    pub is_favorite: bool,
    pub usage_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSourceRef {
    pub kind: String,
    pub ref_id: String,
    pub label: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub thread_id: String,
    pub meeting_id: Option<String>,
    pub role: String,
    pub content: String,
    pub recipe_slug: Option<String>,
    pub sources: Vec<ChatSourceRef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyRule {
    pub id: String,
    pub kind: String,
    pub pattern: String,
    pub description: String,
    pub is_enabled: bool,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpGovernancePolicy {
    pub enabled: bool,
    pub http_port: u16,
    pub allow_live_context: bool,
    pub allow_raw_transcripts: bool,
    pub allow_screenshare_ocr: bool,
    pub allow_clipboard: bool,
    pub allow_agent_writeback: bool,
    pub max_history_days: u32,
}

impl Default for McpGovernancePolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            http_port: 39421,
            allow_live_context: true,
            allow_raw_transcripts: true,
            allow_screenshare_ocr: true,
            allow_clipboard: true,
            allow_agent_writeback: true,
            max_history_days: 90,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAuditEntry {
    pub id: String,
    pub client_name: String,
    pub transport: String,
    pub tool_name: String,
    pub query_summary: String,
    pub items_returned: usize,
    pub duration_ms: u64,
    pub allowed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProviderSettings {
    pub stt_provider: String,
    pub stt_model: String,
    #[serde(default = "default_whisper_model_id")]
    pub whisper_model_id: String,
    pub llm_provider: String,
    pub llm_model: String,
    #[serde(default = "default_embedding_provider")]
    pub embedding_provider: String,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    pub ollama_url: String,
    pub openai_api_key_set: bool,
    pub anthropic_api_key_set: bool,
    pub gemini_api_key_set: bool,
    pub deepgram_api_key_set: bool,
    #[serde(default)]
    pub groq_api_key_set: bool,
}

fn default_whisper_model_id() -> String {
    "ggml-large-v3-turbo-q5_0".to_string()
}

fn default_embedding_provider() -> String {
    "local_hybrid".to_string()
}

fn default_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

impl Default for AiProviderSettings {
    fn default() -> Self {
        Self {
            stt_provider: "local_whisper".to_string(),
            stt_model: "whisper-large-v3-turbo (Apple Metal)".to_string(),
            whisper_model_id: default_whisper_model_id(),
            llm_provider: "hybrid_auto".to_string(),
            llm_model: "claude-sonnet-4-20250514".to_string(),
            embedding_provider: default_embedding_provider(),
            embedding_model: default_embedding_model(),
            ollama_url: "http://127.0.0.1:11434".to_string(),
            openai_api_key_set: false,
            anthropic_api_key_set: false,
            gemini_api_key_set: false,
            deepgram_api_key_set: false,
            groq_api_key_set: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchHit {
    pub id: String,
    pub source_kind: String,
    pub title: String,
    pub snippet: String,
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub project_tag: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarFeedConfig {
    pub id: String,
    pub name: String,
    pub ics_url: String,
    pub enabled: bool,
    pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperModelInfo {
    pub id: String,
    pub name: String,
    pub size_mb: u32,
    pub language_support: String,
    pub download_url: String,
    pub installed: bool,
    pub local_path: Option<String>,
    pub downloading: bool,
    pub progress_pct: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonTelemetry {
    pub loop_running: bool,
    pub poll_interval_secs: u64,
    pub ticks_processed: u64,
    pub snapshots_accepted: u64,
    pub snapshots_filtered: u64,
    pub last_window_app: Option<String>,
    pub last_window_title: Option<String>,
    pub last_tick_at: Option<DateTime<Utc>>,
    pub meeting_detector_phase: String,
    pub db_encrypted: bool,
    pub keychain_backend: String,
    pub autostart_enabled: bool,
    pub global_shortcut: String,
    pub demo_data_present: bool,
    pub whisper_cli_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub release_notes: String,
}

