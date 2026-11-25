use crate::traits::*;
use anyhow::Result;
use async_trait::async_trait;
use backtext_core::ExtractionMethod;
use chrono::{Duration, Utc};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Clone)]
pub struct SimulatorPlatformSensors {
    state: Arc<Mutex<SimState>>,
}

struct SimState {
    active_window: ActiveWindowObservation,
    audio_signal: AudioEnvironmentSignal,
    clipboard: Option<SafeClipboardObservation>,
}

impl Default for SimulatorPlatformSensors {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulatorPlatformSensors {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SimState {
                active_window: ActiveWindowObservation {
                    window_id: 101,
                    app_bundle_id: "com.todesktop.230313mzl4w4u92".into(),
                    app_name: "Cursor".into(),
                    window_title: "backtext — crates/backtext-mcp/src/server.rs".into(),
                    browser_url: None,
                    visible_text: "Active workspace: backtext local-first MCP server & hybrid meeting memory in Rust + Tauri v2.".into(),
                    extraction_method: ExtractionMethod::Accessibility,
                    dhash_u64: 0x4182_93A0_11CC_9812,
                    is_private_browsing: false,
                    is_secure_field_focused: false,
                    detected_screen_share_presenter: None,
                    detected_active_speaker_tile: None,
                },
                audio_signal: AudioEnvironmentSignal {
                    microphone_in_use: false,
                    system_audio_active: false,
                    detected_meeting_app: None,
                    detected_meeting_title: None,
                    has_call_controls_ui: false,
                },
                clipboard: None,
            })),
        }
    }

    pub fn inject_window(&self, obs: ActiveWindowObservation) {
        self.state.lock().active_window = obs;
    }

    pub fn inject_audio_signal(&self, sig: AudioEnvironmentSignal) {
        self.state.lock().audio_signal = sig;
    }

    pub fn inject_clipboard(&self, clip: Option<SafeClipboardObservation>) {
        self.state.lock().clipboard = clip;
    }
}

#[async_trait]
impl PlatformSensors for SimulatorPlatformSensors {
    async fn check_permissions(&self) -> PermissionStatus {
        PermissionStatus {
            platform: "macos-simulator".into(),
            screen_capture_granted: true,
            microphone_granted: true,
            accessibility_granted: true,
            calendar_granted: true,
        }
    }

    async fn observe_active_window(&self) -> Result<ActiveWindowObservation> {
        Ok(self.state.lock().active_window.clone())
    }

    async fn observe_audio_environment(&self) -> Result<AudioEnvironmentSignal> {
        Ok(self.state.lock().audio_signal.clone())
    }

    async fn upcoming_calendar_events(&self) -> Result<Vec<CalendarMeetingEvent>> {
        let now = Utc::now();
        Ok(vec![CalendarMeetingEvent {
            event_id: "cal_evt_9055".into(),
            title: "MCP Workflow Check-in mit Clara Novak (Velox Labs)".into(),
            starts_at: now + Duration::minutes(35),
            ends_at: now + Duration::minutes(65),
            conference_url: Some("https://zoom.us/j/9841220391".into()),
            attendees: vec![CalendarAttendee {
                name: "Clara Novak".into(),
                email: "clara.novak@velox-ai.com".into(),
            }],
        }])
    }

    async fn read_safe_clipboard(&self) -> Result<Option<SafeClipboardObservation>> {
        Ok(self.state.lock().clipboard.clone())
    }

    async fn run_vision_ocr_on_image_path(&self, image_path: &str) -> Result<String> {
        Ok(format!("Simulated Apple Vision OCR output for {image_path}"))
    }
}
