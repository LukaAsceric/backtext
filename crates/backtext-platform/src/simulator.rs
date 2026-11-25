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

fn write_synthetic_voice_wav(path: &str, fundamental_hz: f32, duration_secs: u64) -> Result<usize> {
    let sample_rate = 16_000_u32;
    let total_samples = (sample_rate as usize) * (duration_secs.clamp(1, 5) as usize);
    let mut pcm: Vec<i16> = Vec::with_capacity(total_samples);
    for i in 0..total_samples {
        let t = (i as f32) / (sample_rate as f32);
        // Synthetic voiced harmonic signal (fundamental + 2nd/3rd formants) for real DSP tests
        let val = 0.45 * (2.0 * std::f32::consts::PI * fundamental_hz * t).sin()
            + 0.25 * (2.0 * std::f32::consts::PI * (fundamental_hz * 2.0) * t).sin()
            + 0.15 * (2.0 * std::f32::consts::PI * (fundamental_hz * 3.1) * t).sin();
        pcm.push((val * 16_000.0) as i16);
    }

    let data_size = (pcm.len() * 2) as u32;
    let chunk_size = 36 + data_size;
    let mut buf = Vec::with_capacity(44 + data_size as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&chunk_size.to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16_u32.to_le_bytes());
    buf.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1_u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    buf.extend_from_slice(&2_u16.to_le_bytes());
    buf.extend_from_slice(&16_u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for s in &pcm {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, buf)?;
    Ok(pcm.len())
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
            idle_seconds: 0,
            screen_locked: false,
        }
    }

    async fn request_permission(&self, _kind: &str) -> Result<PermissionStatus> {
        Ok(self.check_permissions().await)
    }

    async fn open_permission_settings(&self, _kind: &str) -> Result<()> {
        Ok(())
    }

    async fn observe_active_window(&self) -> Result<ActiveWindowObservation> {
        Ok(self.state.lock().active_window.clone())
    }

    async fn capture_window_screenshot_ocr(
        &self,
        _include_image: bool,
    ) -> Result<WindowScreenshotOcr> {
        let w = self.state.lock().active_window.clone();
        Ok(WindowScreenshotOcr {
            ok: true,
            dhash_u64: w.dhash_u64,
            dhash_hex: format!("{:016x}", w.dhash_u64),
            ocr_text: w.visible_text,
            image_data_url: None,
        })
    }

    async fn observe_audio_environment(&self) -> Result<AudioEnvironmentSignal> {
        Ok(self.state.lock().audio_signal.clone())
    }

    async fn record_dual_audio_chunk(
        &self,
        out_dir: &str,
        duration_secs: u64,
    ) -> Result<DualAudioCaptureChunk> {
        let mic_path = format!("{out_dir}/mic.wav");
        let sys_path = format!("{out_dir}/system.wav");
        let mic_samples = write_synthetic_voice_wav(&mic_path, 145.0, duration_secs.min(2))?;
        let sys_samples = write_synthetic_voice_wav(&sys_path, 210.0, duration_secs.min(2))?;
        Ok(DualAudioCaptureChunk {
            mic_wav_path: Some(mic_path),
            mic_samples,
            system_wav_path: Some(sys_path),
            system_samples: sys_samples,
        })
    }

    async fn record_mic_wav(&self, out_path: &str, duration_secs: u64) -> Result<bool> {
        let _ = write_synthetic_voice_wav(out_path, 138.0, duration_secs.min(2))?;
        Ok(true)
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

    async fn send_native_notification(&self, _title: &str, _body: &str) -> Result<()> {
        Ok(())
    }
}
