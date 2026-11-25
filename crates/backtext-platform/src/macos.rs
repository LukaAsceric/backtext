#![cfg(target_os = "macos")]

use crate::traits::*;
use anyhow::Result;
use async_trait::async_trait;
use backtext_core::ExtractionMethod;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::process::Command;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
}

#[repr(C)]
struct AudioObjectPropertyAddress {
    m_selector: u32,
    m_scope: u32,
    m_element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyData(
        in_object_id: u32,
        in_address: *const AudioObjectPropertyAddress,
        in_qualifier_data_size: u32,
        in_qualifier_data: *const std::ffi::c_void,
        io_data_size: *mut u32,
        out_data: *mut std::ffi::c_void,
    ) -> i32;
}

const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
const K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE: u32 = u32::from_be_bytes(*b"dIn ");
const K_AUDIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE: u32 = u32::from_be_bytes(*b"gone");
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;

fn is_macos_default_mic_running() -> bool {
    unsafe {
        let mut device_id: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let addr_dev = AudioObjectPropertyAddress {
            m_selector: K_AUDIO_HARDWARE_PROPERTY_DEFAULT_INPUT_DEVICE,
            m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };
        let status = AudioObjectGetPropertyData(
            K_AUDIO_OBJECT_SYSTEM_OBJECT,
            &addr_dev,
            0,
            std::ptr::null(),
            &mut size,
            &mut device_id as *mut u32 as *mut _,
        );
        if status != 0 || device_id == 0 {
            return false;
        }

        let mut is_running: u32 = 0;
        let mut run_size = std::mem::size_of::<u32>() as u32;
        let addr_run = AudioObjectPropertyAddress {
            m_selector: K_AUDIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE,
            m_scope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
            m_element: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
        };
        let run_status = AudioObjectGetPropertyData(
            device_id,
            &addr_run,
            0,
            std::ptr::null(),
            &mut run_size,
            &mut is_running as *mut u32 as *mut _,
        );
        run_status == 0 && is_running != 0
    }
}

pub struct MacOsPlatformSensors {
    sensor_bin: Option<PathBuf>,
}

impl Default for MacOsPlatformSensors {
    fn default() -> Self {
        Self::new()
    }
}

impl MacOsPlatformSensors {
    pub fn new() -> Self {
        let sensor_bin = Self::locate_sensor_binary();
        Self { sensor_bin }
    }

    fn locate_sensor_binary() -> Option<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("backtext-macos-sensor"));
            }
        }
        candidates.push(PathBuf::from(
            "/Applications/backtext.app/Contents/MacOS/backtext-macos-sensor",
        ));
        candidates.push(PathBuf::from("macos-sensor/backtext-macos-sensor"));
        candidates.into_iter().find(|p| p.exists())
    }

    async fn run_sensor_json(&self, args: &[&str]) -> Option<serde_json::Value> {
        let bin = self.sensor_bin.as_ref()?;
        let out = Command::new(bin).args(args).output().await.ok()?;
        if !out.status.success() {
            return None;
        }
        serde_json::from_slice::<serde_json::Value>(&out.stdout).ok()
    }
}

#[async_trait]
impl PlatformSensors for MacOsPlatformSensors {
    async fn check_permissions(&self) -> PermissionStatus {
        if let Some(v) = self.run_sensor_json(&["permissions"]).await {
            return PermissionStatus {
                platform: "macos".into(),
                screen_capture_granted: v["screen_capture_granted"].as_bool().unwrap_or(false),
                microphone_granted: v["microphone_granted"].as_bool().unwrap_or(false),
                accessibility_granted: v["accessibility_granted"].as_bool().unwrap_or(false),
                calendar_granted: v["calendar_granted"].as_bool().unwrap_or(false),
                idle_seconds: v["idle_seconds"].as_u64().unwrap_or(0),
                screen_locked: v["screen_locked"].as_bool().unwrap_or(false),
            };
        }
        let ax_ok = unsafe { AXIsProcessTrusted() };
        let sck_ok = unsafe { CGPreflightScreenCaptureAccess() };
        PermissionStatus {
            platform: "macos".into(),
            screen_capture_granted: sck_ok,
            microphone_granted: true,
            accessibility_granted: ax_ok,
            calendar_granted: false,
            idle_seconds: 0,
            screen_locked: false,
        }
    }

    async fn request_permission(&self, kind: &str) -> Result<PermissionStatus> {
        let _ = self.run_sensor_json(&["request-permission", kind]).await;
        Ok(self.check_permissions().await)
    }

    async fn open_permission_settings(&self, kind: &str) -> Result<()> {
        if self.run_sensor_json(&["open-settings", kind]).await.is_some() {
            return Ok(());
        }
        let pane = match kind {
            "screen" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "calendar" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars",
            _ => "x-apple.systempreferences:com.apple.preference.security?Privacy",
        };
        let _ = Command::new("open").arg(pane).status().await;
        Ok(())
    }

    async fn observe_active_window(&self) -> Result<ActiveWindowObservation> {
        if let Some(v) = self.run_sensor_json(&["active-window"]).await {
            let app_bundle_id = v["app_bundle_id"].as_str().unwrap_or("unknown").to_string();
            let app_name = v["app_name"].as_str().unwrap_or("macOS App").to_string();
            let window_title = v["window_title"].as_str().unwrap_or("").to_string();
            let browser_url = v["browser_url"].as_str().map(|s| s.to_string());
            let visible_text = v["visible_text"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let is_private_browsing = v["is_private_browsing"].as_bool().unwrap_or(false);
            let is_secure_field_focused = v["is_secure_field_focused"].as_bool().unwrap_or(false);
            let detected_screen_share_presenter = v["detected_screen_share_presenter"]
                .as_str()
                .map(|s| s.to_string());
            let detected_active_speaker_tile = v["detected_active_speaker_tile"]
                .as_str()
                .map(|s| s.to_string());

            let digest = Sha256::digest(
                format!("{app_bundle_id}:{window_title}:{browser_url:?}:{visible_text}").as_bytes(),
            );
            let dhash_u64 = u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]));

            return Ok(ActiveWindowObservation {
                window_id: v["window_id"].as_u64().unwrap_or(1) as u32,
                app_bundle_id,
                app_name,
                window_title,
                browser_url,
                visible_text,
                extraction_method: ExtractionMethod::Accessibility,
                dhash_u64,
                is_private_browsing,
                is_secure_field_focused,
                detected_screen_share_presenter,
                detected_active_speaker_tile,
            });
        }

        // Lightweight JXA fallback if compiled Swift helper isn't beside binary
        let jxa = r#"
ObjC.import('AppKit');
const ws = $.NSWorkspace.sharedWorkspace;
const front = ws.frontmostApplication;
const bundleId = front ? ObjC.unwrap(front.bundleIdentifier) || '' : '';
const appName = front ? ObjC.unwrap(front.localizedName) || '' : '';
JSON.stringify({ bundleId, appName, winTitle: appName });
"#;
        let out = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", jxa])
            .output()
            .await?;
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let app_bundle_id = parsed["bundleId"].as_str().unwrap_or("unknown").to_string();
        let app_name = parsed["appName"].as_str().unwrap_or("macOS App").to_string();
        let window_title = parsed["winTitle"].as_str().unwrap_or("").to_string();
        let digest = Sha256::digest(format!("{app_bundle_id}:{window_title}").as_bytes());
        let dhash_u64 = u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]));

        Ok(ActiveWindowObservation {
            window_id: 1,
            app_bundle_id,
            app_name: app_name.clone(),
            window_title: window_title.clone(),
            browser_url: None,
            visible_text: format!("{app_name} — {window_title}"),
            extraction_method: ExtractionMethod::Accessibility,
            dhash_u64,
            is_private_browsing: false,
            is_secure_field_focused: false,
            detected_screen_share_presenter: None,
            detected_active_speaker_tile: None,
        })
    }

    async fn capture_window_screenshot_ocr(
        &self,
        include_image: bool,
    ) -> Result<WindowScreenshotOcr> {
        let args: Vec<&str> = if include_image {
            vec!["capture-window-ocr", "--image"]
        } else {
            vec!["capture-window-ocr"]
        };
        if let Some(v) = self.run_sensor_json(&args).await {
            return Ok(WindowScreenshotOcr {
                ok: v["ok"].as_bool().unwrap_or(false),
                dhash_u64: v["dhash_u64"].as_u64().unwrap_or(0),
                dhash_hex: v["dhash_hex"]
                    .as_str()
                    .unwrap_or("0000000000000000")
                    .to_string(),
                ocr_text: v["ocr_text"].as_str().unwrap_or("").to_string(),
                image_data_url: v["image_data_url"].as_str().map(|s| s.to_string()),
            });
        }
        Ok(WindowScreenshotOcr {
            ok: false,
            dhash_u64: 0,
            dhash_hex: "0000000000000000".into(),
            ocr_text: String::new(),
            image_data_url: None,
        })
    }

    async fn observe_audio_environment(&self) -> Result<AudioEnvironmentSignal> {
        let mic_active = is_macos_default_mic_running();
        let win = self.observe_active_window().await.ok();
        let mut meeting_app = None;
        let mut meeting_title = None;
        if let Some(ref w) = win {
            let low = format!(
                "{} {} {:?}",
                w.app_bundle_id, w.window_title, w.browser_url
            )
            .to_lowercase();
            if low.contains("zoom.us") || low.contains("us.zoom.xos") {
                meeting_app = Some("Zoom".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("meet.google.com") {
                meeting_app = Some("Google Meet".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("teams.microsoft") || low.contains("com.microsoft.teams") {
                meeting_app = Some("Microsoft Teams".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("slack") && low.contains("huddle") {
                meeting_app = Some("Slack Huddle".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("facetime") {
                meeting_app = Some("FaceTime".into());
                meeting_title = Some(w.window_title.clone());
            }
        }
        Ok(AudioEnvironmentSignal {
            microphone_in_use: mic_active,
            system_audio_active: mic_active && meeting_app.is_some(),
            has_call_controls_ui: meeting_app.is_some(),
            detected_meeting_app: meeting_app,
            detected_meeting_title: meeting_title,
        })
    }

    async fn record_dual_audio_chunk(
        &self,
        out_dir: &str,
        duration_secs: u64,
    ) -> Result<DualAudioCaptureChunk> {
        let dur_str = duration_secs.to_string();
        if let Some(v) = self
            .run_sensor_json(&["record-dual-audio", out_dir, &dur_str])
            .await
        {
            return Ok(DualAudioCaptureChunk {
                mic_wav_path: v["mic_wav_path"].as_str().map(|s| s.to_string()),
                mic_samples: v["mic_samples"].as_u64().unwrap_or(0) as usize,
                system_wav_path: v["system_wav_path"].as_str().map(|s| s.to_string()),
                system_samples: v["system_samples"].as_u64().unwrap_or(0) as usize,
            });
        }
        Ok(DualAudioCaptureChunk {
            mic_wav_path: None,
            mic_samples: 0,
            system_wav_path: None,
            system_samples: 0,
        })
    }

    async fn record_mic_wav(&self, out_path: &str, duration_secs: u64) -> Result<bool> {
        let dur_str = duration_secs.to_string();
        if let Some(v) = self
            .run_sensor_json(&["record-mic", out_path, &dur_str])
            .await
        {
            return Ok(v["ok"].as_bool().unwrap_or(false));
        }
        Ok(false)
    }

    async fn upcoming_calendar_events(&self) -> Result<Vec<CalendarMeetingEvent>> {
        let Some(v) = self.run_sensor_json(&["calendar"]).await else {
            return Ok(Vec::new());
        };
        let Some(arr) = v["events"].as_array() else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for ev in arr {
            let Some(starts_str) = ev["starts_at"].as_str() else {
                continue;
            };
            let Some(ends_str) = ev["ends_at"].as_str() else {
                continue;
            };
            let Ok(starts_at) = DateTime::parse_from_rfc3339(starts_str) else {
                continue;
            };
            let Ok(ends_at) = DateTime::parse_from_rfc3339(ends_str) else {
                continue;
            };
            let attendees = ev["attendees"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .map(|a| CalendarAttendee {
                            name: a["name"].as_str().unwrap_or("").to_string(),
                            email: a["email"].as_str().unwrap_or("").to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            out.push(CalendarMeetingEvent {
                event_id: ev["event_id"].as_str().unwrap_or("").to_string(),
                title: ev["title"].as_str().unwrap_or("Meeting").to_string(),
                starts_at: starts_at.with_timezone(&Utc),
                ends_at: ends_at.with_timezone(&Utc),
                conference_url: ev["conference_url"].as_str().map(|s| s.to_string()),
                attendees,
            });
        }
        Ok(out)
    }

    async fn read_safe_clipboard(&self) -> Result<Option<SafeClipboardObservation>> {
        if let Some(v) = self.run_sensor_json(&["clipboard"]).await {
            let text = v["text"].as_str().unwrap_or("").to_string();
            let is_concealed = v["is_concealed"].as_bool().unwrap_or(false);
            if text.is_empty() && !is_concealed {
                return Ok(None);
            }
            return Ok(Some(SafeClipboardObservation {
                source_app: v["source_app"].as_str().unwrap_or("macOS").to_string(),
                text,
                is_concealed,
            }));
        }
        Ok(None)
    }

    async fn run_vision_ocr_on_image_path(&self, image_path: &str) -> Result<String> {
        let swift_ocr = r#"
import Foundation
import Vision
let args = CommandLine.arguments
guard args.count > 1 else { exit(1) }
let url = URL(fileURLWithPath: args[1])
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.recognitionLanguages = ["de-DE", "en-US"]
request.usesLanguageCorrection = true
let handler = VNImageRequestHandler(url: url, options: [:])
try handler.perform([request])
let lines = request.results?.compactMap { $0.topCandidates(1).first?.string } ?? []
print(lines.joined(separator: "\n"))
"#;
        let out = Command::new("swift")
            .args(["-e", swift_ocr, image_path])
            .output()
            .await?;
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    async fn send_native_notification(&self, title: &str, body: &str) -> Result<()> {
        let safe_t = title.replace('"', "'");
        let safe_b = body.replace('"', "'");
        let script = format!("display notification \"{safe_b}\" with title \"{safe_t}\"");
        let _ = Command::new("osascript").args(["-e", &script]).status().await;
        Ok(())
    }
}
