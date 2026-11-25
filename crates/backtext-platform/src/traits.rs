use async_trait::async_trait;
use backtext_core::ExtractionMethod;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub platform: String,
    pub screen_capture_granted: bool,
    pub microphone_granted: bool,
    pub accessibility_granted: bool,
    pub calendar_granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveWindowObservation {
    pub window_id: u32,
    pub app_bundle_id: String,
    pub app_name: String,
    pub window_title: String,
    pub browser_url: Option<String>,
    pub visible_text: String,
    pub extraction_method: ExtractionMethod,
    pub dhash_u64: u64,
    pub is_private_browsing: bool,
    pub is_secure_field_focused: bool,
    pub detected_screen_share_presenter: Option<String>,
    pub detected_active_speaker_tile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEnvironmentSignal {
    pub microphone_in_use: bool,
    pub system_audio_active: bool,
    pub detected_meeting_app: Option<String>,
    pub detected_meeting_title: Option<String>,
    pub has_call_controls_ui: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarAttendee {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarMeetingEvent {
    pub event_id: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub conference_url: Option<String>,
    pub attendees: Vec<CalendarAttendee>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeClipboardObservation {
    pub source_app: String,
    pub text: String,
    pub is_concealed: bool,
}

#[async_trait]
pub trait PlatformSensors: Send + Sync {
    async fn check_permissions(&self) -> PermissionStatus;
    async fn observe_active_window(&self) -> anyhow::Result<ActiveWindowObservation>;
    async fn observe_audio_environment(&self) -> anyhow::Result<AudioEnvironmentSignal>;
    async fn upcoming_calendar_events(&self) -> anyhow::Result<Vec<CalendarMeetingEvent>>;
    async fn read_safe_clipboard(&self) -> anyhow::Result<Option<SafeClipboardObservation>>;
    async fn run_vision_ocr_on_image_path(&self, image_path: &str) -> anyhow::Result<String>;
}
