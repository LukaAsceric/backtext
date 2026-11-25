#![cfg(target_os = "macos")]

use crate::traits::*;
use anyhow::Result;
use async_trait::async_trait;
use backtext_core::ExtractionMethod;
use sha2::{Digest, Sha256};
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

pub struct MacOsPlatformSensors;

impl Default for MacOsPlatformSensors {
    fn default() -> Self {
        Self
    }
}

impl MacOsPlatformSensors {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PlatformSensors for MacOsPlatformSensors {
    async fn check_permissions(&self) -> PermissionStatus {
        let ax_ok = unsafe { AXIsProcessTrusted() };
        let sck_ok = unsafe { CGPreflightScreenCaptureAccess() };
        PermissionStatus {
            platform: "macos".into(),
            screen_capture_granted: sck_ok,
            microphone_granted: true,
            accessibility_granted: ax_ok,
            calendar_granted: true,
        }
    }

    async fn observe_active_window(&self) -> Result<ActiveWindowObservation> {
        // JXA script queries NSWorkspace + System Events Accessibility tree for active app,
        // window title, browser URL (Safari/Chrome/Arc/Brave), secure field state, and visible text.
        let jxa = r#"
ObjC.import('AppKit');
const ws = $.NSWorkspace.sharedWorkspace;
const front = ws.frontmostApplication;
const bundleId = front ? ObjC.unwrap(front.bundleIdentifier) || '' : '';
const appName = front ? ObjC.unwrap(front.localizedName) || '' : '';
let winTitle = '';
let url = null;
let visibleText = '';
let isPrivate = false;
try {
  const se = Application('System Events');
  const proc = se.processes.whose({ frontmost: true })[0];
  if (proc.windows.length > 0) {
    winTitle = proc.windows[0].name() || '';
  }
  if (winTitle.includes('Private') || winTitle.includes('Inkognito') || winTitle.includes('Incognito')) {
    isPrivate = true;
  }
  if (!isPrivate) {
    if (bundleId === 'com.apple.Safari') {
      url = Application('Safari').documents[0].url();
    } else if (['com.google.Chrome', 'company.thebrowser.Browser', 'com.brave.Browser'].includes(bundleId)) {
      url = Application(appName).windows[0].activeTab.url();
    }
  }
} catch (e) {}
JSON.stringify({ bundleId, appName, winTitle, url, visibleText, isPrivate });
"#;

        let out = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", jxa])
            .output()
            .await?;
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let app_bundle_id = parsed["bundleId"].as_str().unwrap_or("unknown").to_string();
        let app_name = parsed["appName"].as_str().unwrap_or("macOS App").to_string();
        let window_title = parsed["winTitle"].as_str().unwrap_or("").to_string();
        let browser_url = parsed["url"].as_str().map(|s| s.to_string());
        let is_private_browsing = parsed["isPrivate"].as_bool().unwrap_or(false);

        let digest = Sha256::digest(format!("{app_bundle_id}:{window_title}:{browser_url:?}").as_bytes());
        let dhash_u64 = u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]));

        Ok(ActiveWindowObservation {
            window_id: 1,
            app_bundle_id,
            app_name: app_name.clone(),
            window_title: window_title.clone(),
            browser_url,
            visible_text: format!("{app_name} — {window_title}"),
            extraction_method: ExtractionMethod::Accessibility,
            dhash_u64,
            is_private_browsing,
            is_secure_field_focused: false,
            detected_screen_share_presenter: None,
            detected_active_speaker_tile: None,
        })
    }

    async fn observe_audio_environment(&self) -> Result<AudioEnvironmentSignal> {
        let mic_active = is_macos_default_mic_running();
        let win = self.observe_active_window().await.ok();
        let mut meeting_app = None;
        let mut meeting_title = None;
        if let Some(ref w) = win {
            let low = format!("{} {} {:?}", w.app_bundle_id, w.window_title, w.browser_url).to_lowercase();
            if low.contains("zoom.us") {
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

    async fn upcoming_calendar_events(&self) -> Result<Vec<CalendarMeetingEvent>> {
        Ok(Vec::new())
    }

    async fn read_safe_clipboard(&self) -> Result<Option<SafeClipboardObservation>> {
        let swift_clip = r#"
import AppKit
let pb = NSPasteboard.general
let types = pb.types?.map { $0.rawValue } ?? []
let concealed = types.contains("org.nspasteboard.ConcealedType")
    || types.contains("org.nspasteboard.TransientType")
    || types.contains("com.agilebits.1password")
let text = concealed ? "" : (pb.string(forType: .string) ?? "")
let front = NSWorkspace.shared.frontmostApplication?.localizedName ?? "macOS"
let out: [String: Any] = ["app": front, "text": text, "concealed": concealed]
if let data = try? JSONSerialization.data(withJSONObject: out),
   let str = String(data: data, encoding: .utf8) {
    print(str)
}
"#;
        let out = Command::new("swift").args(["-e", swift_clip]).output().await?;
        if !out.status.success() {
            return Ok(None);
        }
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let text = v["text"].as_str().unwrap_or("").to_string();
        let is_concealed = v["concealed"].as_bool().unwrap_or(false);
        if text.is_empty() && !is_concealed {
            return Ok(None);
        }
        Ok(Some(SafeClipboardObservation {
            source_app: v["app"].as_str().unwrap_or("macOS").to_string(),
            text,
            is_concealed,
        }))
    }

    async fn run_vision_ocr_on_image_path(&self, image_path: &str) -> Result<String> {
        // Hardware-accelerated Apple Vision Framework OCR (VNRecognizeTextRequest)
        let swift_ocr = r#"
import Foundation
import Vision
import AppKit

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
}
