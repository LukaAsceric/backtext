export type Language = 'de' | 'en';

export type CaptureStatus =
  | { mode: 'active' }
  | { mode: 'recording_meeting'; meeting_id: string; meeting_title: string; started_at: string }
  | { mode: 'paused'; until: string | null; reason: string | null };

export interface PermissionStatus {
  platform: string;
  screen_capture_granted: boolean;
  microphone_granted: boolean;
  accessibility_granted: boolean;
  calendar_granted: boolean;
}

export interface UserProfile {
  id: string;
  display_name: string;
  email: string | null;
  role: string;
  organization: string;
  ui_language: Language;
  preferred_output_language: Language;
  writing_style: string;
  tone_instructions: string;
  active_projects: string[];
  has_self_voiceprint: boolean;
  auto_record_meetings: boolean;
  consent_reminder_enabled: boolean;
  retention_days: number;
  updated_at: string;
}

export interface VocabularyEntry {
  id: string;
  term: string;
  phonetic_hint: string | null;
  category: string;
  project_tag: string | null;
  created_at: string;
}

export interface LearnedPreference {
  id: string;
  category: string;
  fact_text: string;
  confidence: number;
  source_ref: string | null;
  is_active: boolean;
  created_at: string;
}

export interface Person {
  id: string;
  full_name: string;
  email: string | null;
  company: string | null;
  role_notes: string;
  voiceprint_samples: number;
  voiceprint_confidence: number;
  meeting_count: number;
  open_commitments: string[];
  last_seen_at: string | null;
}

export interface ActionItem {
  id: string;
  owner: string;
  task: string;
  due_hint: string | null;
  completed: boolean;
}

export interface MeetingParticipant {
  speaker_label: string;
  display_name: string;
  person_id: string | null;
  email: string | null;
  is_self: boolean;
  identification_source:
    | 'mic_self'
    | 'voiceprint'
    | 'visual_active_speaker'
    | 'calendar_prior'
    | 'manual'
    | 'unidentified_cluster';
  confidence: number;
  talk_time_ms: number;
}

export interface TranscriptSegment {
  id: string;
  meeting_id: string;
  speaker_label: string;
  speaker_name: string;
  person_id: string | null;
  channel: 'microphone' | 'system';
  start_ms: number;
  end_ms: number;
  text: string;
  confidence: number;
}

export interface ScreenShareFrame {
  id: string;
  meeting_id: string;
  captured_at_ms: number;
  presenter_name: string | null;
  dhash_hex: string;
  image_data_url: string | null;
  ocr_text: string;
  slide_title: string;
}

export interface Meeting {
  id: string;
  title: string;
  calendar_event_id: string | null;
  detected_app: string;
  project_tag: string | null;
  started_at: string;
  ended_at: string | null;
  status: 'scheduled' | 'candidate' | 'live' | 'completed';
  template_id: string;
  prep_brief: string | null;
  raw_user_notes: string;
  executive_summary: string;
  enhanced_markdown: string;
  action_items: ActionItem[];
  participants: MeetingParticipant[];
  transcript_count: number;
  screenshare_count: number;
}

export interface ChatSourceRef {
  kind: string;
  ref_id: string;
  label: string;
  snippet: string;
}

export interface ChatMessage {
  id: string;
  thread_id: string;
  meeting_id: string | null;
  role: 'user' | 'assistant' | string;
  content: string;
  recipe_slug: string | null;
  sources: ChatSourceRef[];
  created_at: string;
}

export interface MeetingDetail {
  meeting: Meeting;
  transcript: TranscriptSegment[];
  screenshares: ScreenShareFrame[];
  chat_messages: ChatMessage[];
}

export interface ContextSnapshot {
  id: string;
  captured_at: string;
  app_bundle_id: string;
  app_name: string;
  window_title: string;
  browser_url: string | null;
  dwell_ms: number;
  dhash_hex: string;
  extracted_text: string;
  extraction_method: 'accessibility' | 'vision_ocr';
  relevance_score: number;
  project_tag: string | null;
  redacted_secrets_count: number;
  episode_id: string | null;
}

export interface ContextEpisode {
  id: string;
  started_at: string;
  ended_at: string;
  title: string;
  summary: string;
  primary_apps: string[];
  urls: string[];
  project_tag: string | null;
  importance_score: number;
  snapshot_count: number;
}

export interface ClipboardItem {
  id: string;
  captured_at: string;
  source_app: string;
  content_kind: string;
  text_preview: string;
  content_hash: string;
  relevance_score: number;
  project_tag: string | null;
}

export interface Recipe {
  id: string;
  slug: string;
  name_de: string;
  name_en: string;
  description_de: string;
  description_en: string;
  icon_key: string;
  scope: 'single_meeting' | 'multi_meeting' | 'ambient_workspace';
  prompt_template: string;
  preferred_model: string | null;
  is_builtin: boolean;
  is_favorite: boolean;
  usage_count: number;
}

export interface PrivacyRule {
  id: string;
  kind: string;
  pattern: string;
  description: string;
  is_enabled: boolean;
  is_builtin: boolean;
}

export interface McpGovernancePolicy {
  enabled: boolean;
  http_port: number;
  allow_live_context: boolean;
  allow_raw_transcripts: boolean;
  allow_screenshare_ocr: boolean;
  allow_clipboard: boolean;
  allow_agent_writeback: boolean;
  max_history_days: number;
}

export interface McpClientConfigSnippets {
  stdio_binary_path: string;
  streamable_http_url: string;
  claude_desktop_json: string;
  cursor_mcp_json: string;
  claude_code_cli: string;
}

export interface McpAuditEntry {
  id: string;
  client_name: string;
  transport: string;
  tool_name: string;
  query_summary: string;
  items_returned: number;
  duration_ms: number;
  allowed: boolean;
  created_at: string;
}

export interface AiProviderSettings {
  stt_provider: string;
  stt_model: string;
  llm_provider: string;
  llm_model: string;
  ollama_url: string;
  openai_api_key_set: boolean;
  anthropic_api_key_set: boolean;
  gemini_api_key_set: boolean;
  deepgram_api_key_set: boolean;
}

export interface MemorySearchHit {
  id: string;
  source_kind: string;
  title: string;
  snippet: string;
  timestamp: string;
  score: number;
  project_tag: string | null;
  metadata: Record<string, unknown>;
}

export interface ScrubberPreview {
  blocked: boolean;
  block_reason: string | null;
  scrubbed_text: string;
  redacted_secrets_count: number;
  relevance_score: number;
  matched_project: string | null;
}

export interface BootstrapPayload {
  capture_status: CaptureStatus;
  permissions: PermissionStatus;
  profile: UserProfile;
  meetings: Meeting[];
  episodes: ContextEpisode[];
  snapshots: ContextSnapshot[];
  clipboard: ClipboardItem[];
  persons: Person[];
  recipes: Recipe[];
  privacy_rules: PrivacyRule[];
  vocabulary: VocabularyEntry[];
  learned_preferences: LearnedPreference[];
  mcp_policy: McpGovernancePolicy;
  mcp_snippets: McpClientConfigSnippets;
  mcp_audit_logs: McpAuditEntry[];
  ai_settings: AiProviderSettings;
}
