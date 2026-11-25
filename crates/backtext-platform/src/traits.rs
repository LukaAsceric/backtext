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
    #[serde(default)]
    pub idle_seconds: u64,
    #[serde(default)]
    pub screen_locked: bool,
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
pub struct WindowScreenshotOcr {
    pub ok: bool,
    pub dhash_u64: u64,
    pub dhash_hex: String,
    pub ocr_text: String,
    pub image_data_url: Option<String>,
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
pub struct DualAudioCaptureChunk {
    pub mic_wav_path: Option<String>,
    pub mic_samples: usize,
    pub system_wav_path: Option<String>,
    pub system_samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DualAudioStreamEvent {
    Level {
        mic_rms: f32,
        system_rms: f32,
        mic_active: bool,
        system_active: bool,
        elapsed_secs: u64,
        error: Option<String>,
    },
    Chunk {
        seq: u64,
        mic_wav_path: Option<String>,
        mic_samples: usize,
        system_wav_path: Option<String>,
        system_samples: usize,
    },
}

pub struct DualAudioStreamSession {
    pub rx: tokio::sync::mpsc::UnboundedReceiver<DualAudioStreamEvent>,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl DualAudioStreamSession {
    pub fn new(
        rx: tokio::sync::mpsc::UnboundedReceiver<DualAudioStreamEvent>,
        stop_tx: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        Self {
            rx,
            stop_tx: Some(stop_tx),
        }
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for DualAudioStreamSession {
    fn drop(&mut self) {
        self.stop();
    }
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
    async fn request_permission(&self, kind: &str) -> anyhow::Result<PermissionStatus>;
    async fn open_permission_settings(&self, kind: &str) -> anyhow::Result<()>;
    async fn observe_active_window(&self) -> anyhow::Result<ActiveWindowObservation>;
    async fn capture_window_screenshot_ocr(
        &self,
        include_image: bool,
    ) -> anyhow::Result<WindowScreenshotOcr>;
    async fn observe_audio_environment(&self) -> anyhow::Result<AudioEnvironmentSignal>;
    async fn record_dual_audio_chunk(
        &self,
        out_dir: &str,
        duration_secs: u64,
    ) -> anyhow::Result<DualAudioCaptureChunk>;
    async fn start_dual_audio_stream(
        &self,
        out_dir: &str,
        chunk_secs: u64,
    ) -> anyhow::Result<DualAudioStreamSession>;
    async fn record_mic_wav(&self, out_path: &str, duration_secs: u64) -> anyhow::Result<bool>;
    async fn upcoming_calendar_events(&self) -> anyhow::Result<Vec<CalendarMeetingEvent>>;
    async fn read_safe_clipboard(&self) -> anyhow::Result<Option<SafeClipboardObservation>>;
    async fn run_vision_ocr_on_image_path(&self, image_path: &str) -> anyhow::Result<String>;
    async fn send_native_notification(&self, title: &str, body: &str) -> anyhow::Result<()>;
}
