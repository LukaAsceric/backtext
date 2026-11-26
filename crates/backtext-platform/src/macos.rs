#![cfg(target_os = "macos")]

use crate::traits::*;
use anyhow::Result;
use async_trait::async_trait;
use backtext_core::ExtractionMethod;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::ffi::{c_char, c_void, CStr};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

// ============================================================================
// In-Process macOS CoreFoundation / ApplicationServices / CoreGraphics FFI
// Critical: macOS TCC grants Accessibility (`Bedienungshilfen`) per main binary
// (`backtext.app` -> `backtext-desktop`), NOT to spawned helper CLIs.
// ============================================================================

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[repr(C)]
struct OpaqueCallbacks {
    _opaque: [u8; 64],
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: OpaqueCallbacks;
    static kCFTypeDictionaryValueCallBacks: OpaqueCallbacks;

    fn CFRelease(cf: *const c_void);
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFStringCreateWithBytes(
        alloc: *const c_void,
        bytes: *const u8,
        num_bytes: isize,
        encoding: u32,
        is_external_representation: u8,
    ) -> *const c_void;
    fn CFStringGetLength(the_string: *const c_void) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetCString(
        the_string: *const c_void,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
    fn CFArrayGetCount(the_array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(the_array: *const c_void, idx: isize) -> *const c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    static kAXTrustedCheckOptionPrompt: *const c_void;
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
    fn AXUIElementCopyAttributeValue(
        element: *const c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightScreenCaptureAccess() -> u8;
    fn CGRequestScreenCaptureAccess() -> u8;
}

fn in_process_ax_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

fn in_process_request_ax_prompt() -> bool {
    unsafe {
        let key = kAXTrustedCheckOptionPrompt;
        let val = kCFBooleanTrue;
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            &key as *const *const c_void,
            &val as *const *const c_void,
            1,
            std::ptr::addr_of!(kCFTypeDictionaryKeyCallBacks) as *const c_void,
            std::ptr::addr_of!(kCFTypeDictionaryValueCallBacks) as *const c_void,
        );
        if dict.is_null() {
            return AXIsProcessTrusted() != 0;
        }
        let trusted = AXIsProcessTrustedWithOptions(dict) != 0;
        CFRelease(dict);
        trusted
    }
}

fn in_process_screen_capture_granted() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() != 0 }
}

unsafe fn cfstring_new(s: &str) -> *const c_void {
    CFStringCreateWithBytes(
        std::ptr::null(),
        s.as_ptr(),
        s.len() as isize,
        K_CF_STRING_ENCODING_UTF8,
        0,
    )
}

unsafe fn cfstring_to_rust(cf: *const c_void) -> Option<String> {
    if cf.is_null() || CFGetTypeID(cf) != CFStringGetTypeID() {
        return None;
    }
    let len = CFStringGetLength(cf);
    let max_size = CFStringGetMaximumSizeForEncoding(len, K_CF_STRING_ENCODING_UTF8) + 1;
    if max_size <= 0 || max_size > 1_000_000 {
        return None;
    }
    let mut buf = vec![0u8; max_size as usize];
    if CFStringGetCString(
        cf,
        buf.as_mut_ptr() as *mut c_char,
        max_size,
        K_CF_STRING_ENCODING_UTF8,
    ) != 0
    {
        let cstr = CStr::from_ptr(buf.as_ptr() as *const c_char);
        return cstr.to_str().ok().map(|s| s.to_string());
    }
    None
}

unsafe fn ax_copy_string_attr(el: *const c_void, attr_name: &str) -> Option<String> {
    let key = cfstring_new(attr_name);
    if key.is_null() {
        return None;
    }
    let mut out: *const c_void = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(el, key, &mut out);
    CFRelease(key);
    if err != 0 || out.is_null() {
        return None;
    }
    let res = cfstring_to_rust(out);
    CFRelease(out);
    res
}

unsafe fn collect_ax_text_in_process(
    el: *const c_void,
    depth: usize,
    max_depth: usize,
    chunks: &mut Vec<String>,
    seen_chars: &mut usize,
    secure_detected: &mut bool,
) {
    if el.is_null() || depth > max_depth || *seen_chars > 3600 {
        return;
    }
    if let Some(role) = ax_copy_string_attr(el, "AXRole") {
        if role == "AXSecureTextField" {
            *secure_detected = true;
            return;
        }
    }
    for attr in ["AXValue", "AXTitle", "AXDescription"] {
        if let Some(txt) = ax_copy_string_attr(el, attr) {
            let trimmed = txt.trim();
            if trimmed.len() >= 2 && trimmed.len() <= 800 {
                if chunks.last().map(|s| s.as_str()) != Some(trimmed) {
                    *seen_chars += trimmed.len();
                    chunks.push(trimmed.to_string());
                }
            }
        }
    }

    let children_key = cfstring_new("AXChildren");
    if children_key.is_null() {
        return;
    }
    let mut children_ref: *const c_void = std::ptr::null();
    let err = AXUIElementCopyAttributeValue(el, children_key, &mut children_ref);
    CFRelease(children_key);
    if err == 0 && !children_ref.is_null() {
        if CFGetTypeID(children_ref) == CFArrayGetTypeID() {
            let count = CFArrayGetCount(children_ref).min(30);
            for i in 0..count {
                if *seen_chars > 3600 {
                    break;
                }
                let child = CFArrayGetValueAtIndex(children_ref, i);
                collect_ax_text_in_process(
                    child,
                    depth + 1,
                    max_depth,
                    chunks,
                    seen_chars,
                    secure_detected,
                );
            }
        }
        CFRelease(children_ref);
    }
}

/// Reads the focused window title, visible AX text, and secure-field flag directly
/// inside the main `backtext-desktop` process so macOS TCC Accessibility grants always apply.
fn read_frontmost_ax_in_process(pid: i32) -> (String, String, bool) {
    if pid <= 0 || !in_process_ax_trusted() {
        return (String::new(), String::new(), false);
    }
    unsafe {
        let app_el = AXUIElementCreateApplication(pid);
        if app_el.is_null() {
            return (String::new(), String::new(), false);
        }
        let mut title = String::new();
        let mut chunks: Vec<String> = Vec::new();
        let mut seen_chars = 0usize;
        let mut secure_focused = false;

        let win_key = cfstring_new("AXFocusedWindow");
        if !win_key.is_null() {
            let mut win_ref: *const c_void = std::ptr::null();
            if AXUIElementCopyAttributeValue(app_el, win_key, &mut win_ref) == 0 && !win_ref.is_null()
            {
                title = ax_copy_string_attr(win_ref, "AXTitle").unwrap_or_default();
                collect_ax_text_in_process(
                    win_ref,
                    0,
                    5,
                    &mut chunks,
                    &mut seen_chars,
                    &mut secure_focused,
                );
                CFRelease(win_ref);
            }
            CFRelease(win_key);
        }

        let foc_key = cfstring_new("AXFocusedUIElement");
        if !foc_key.is_null() {
            let mut foc_ref: *const c_void = std::ptr::null();
            if AXUIElementCopyAttributeValue(app_el, foc_key, &mut foc_ref) == 0 && !foc_ref.is_null()
            {
                if ax_copy_string_attr(foc_ref, "AXRole").as_deref() == Some("AXSecureTextField") {
                    secure_focused = true;
                }
                CFRelease(foc_ref);
            }
            CFRelease(foc_key);
        }

        CFRelease(app_el);
        (title, chunks.join(" · "), secure_focused)
    }
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
        in_qualifier_data: *const c_void,
        io_data_size: *mut u32,
        out_data: *mut c_void,
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
        // Always trust in-process `AXIsProcessTrusted()` and `CGPreflightScreenCaptureAccess()`
        // because macOS TCC grants Accessibility strictly to the main `.app` process (`backtext-desktop`).
        let ax_in_proc = in_process_ax_trusted();
        let sck_in_proc = in_process_screen_capture_granted();

        if let Some(v) = self.run_sensor_json(&["permissions"]).await {
            return PermissionStatus {
                platform: "macos".into(),
                screen_capture_granted: sck_in_proc
                    || v["screen_capture_granted"].as_bool().unwrap_or(false),
                microphone_granted: v["microphone_granted"].as_bool().unwrap_or(false),
                accessibility_granted: ax_in_proc
                    || v["accessibility_granted"].as_bool().unwrap_or(false),
                calendar_granted: v["calendar_granted"].as_bool().unwrap_or(false),
                idle_seconds: v["idle_seconds"].as_u64().unwrap_or(0),
                screen_locked: v["screen_locked"].as_bool().unwrap_or(false),
            };
        }
        PermissionStatus {
            platform: "macos".into(),
            screen_capture_granted: sck_in_proc,
            microphone_granted: true,
            accessibility_granted: ax_in_proc,
            calendar_granted: false,
            idle_seconds: 0,
            screen_locked: false,
        }
    }

    async fn request_permission(&self, kind: &str) -> Result<PermissionStatus> {
        match kind {
            "accessibility" => {
                // Trigger TCC prompt directly from `backtext-desktop` (so `backtext.app` is registered)
                let _ = in_process_request_ax_prompt();
                let _ = self.open_permission_settings("accessibility").await;
            }
            "screen" => {
                unsafe {
                    let _ = CGRequestScreenCaptureAccess();
                }
                let _ = self.open_permission_settings("screen").await;
            }
            _ => {
                let _ = self.run_sensor_json(&["request-permission", kind]).await;
            }
        }
        Ok(self.check_permissions().await)
    }

    async fn open_permission_settings(&self, kind: &str) -> Result<()> {
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
            let pid = v["window_id"].as_u64().unwrap_or(0) as i32;
            let app_bundle_id = v["app_bundle_id"].as_str().unwrap_or("unknown").to_string();
            let app_name = v["app_name"].as_str().unwrap_or("macOS App").to_string();
            let mut window_title = v["window_title"].as_str().unwrap_or("").to_string();
            let browser_url = v["browser_url"].as_str().map(|s| s.to_string());
            let mut visible_text = v["visible_text"].as_str().unwrap_or("").to_string();
            let mut is_private_browsing = v["is_private_browsing"].as_bool().unwrap_or(false);
            let mut is_secure_field_focused =
                v["is_secure_field_focused"].as_bool().unwrap_or(false);
            let mut detected_screen_share_presenter = v["detected_screen_share_presenter"]
                .as_str()
                .map(|s| s.to_string());
            let mut detected_active_speaker_tile = v["detected_active_speaker_tile"]
                .as_str()
                .map(|s| s.to_string());

            // Enrich directly in-process via AXUIElement when main app holds Accessibility TCC grant
            if (window_title.is_empty() || visible_text.len() < 24) && in_process_ax_trusted() {
                let (ax_title, ax_text, ax_secure) =
                    tokio::task::spawn_blocking(move || read_frontmost_ax_in_process(pid))
                        .await
                        .unwrap_or_default();
                if !ax_title.is_empty() {
                    window_title = ax_title;
                }
                if ax_secure {
                    is_secure_field_focused = true;
                }
                let low_title = window_title.to_lowercase();
                if low_title.contains("private")
                    || low_title.contains("inkognito")
                    || low_title.contains("incognito")
                    || low_title.contains("privat")
                {
                    is_private_browsing = true;
                }
                if !is_secure_field_focused && !is_private_browsing && !ax_text.is_empty() {
                    if low_title.contains("zoom")
                        || low_title.contains("meet")
                        || low_title.contains("teams")
                    {
                        for chunk in ax_text.split(" · ") {
                            let low_c = chunk.to_lowercase();
                            if detected_screen_share_presenter.is_none()
                                && (low_c.contains("is presenting") || low_c.contains("präsentiert"))
                            {
                                detected_screen_share_presenter = Some(chunk.trim().to_string());
                            } else if detected_active_speaker_tile.is_none()
                                && (low_c.contains("speaking:") || low_c.contains("spricht"))
                            {
                                detected_active_speaker_tile = Some(
                                    chunk
                                        .replace("Speaking:", "")
                                        .replace("spricht", "")
                                        .trim()
                                        .to_string(),
                                );
                            }
                        }
                    }
                    visible_text = ax_text;
                }
            }

            if is_secure_field_focused || is_private_browsing {
                visible_text.clear();
            } else if visible_text.is_empty() && !window_title.is_empty() {
                visible_text = format!("{app_name} — {window_title}");
            }

            let digest = Sha256::digest(
                format!("{app_bundle_id}:{window_title}:{browser_url:?}:{visible_text}").as_bytes(),
            );
            let dhash_u64 = u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]));

            return Ok(ActiveWindowObservation {
                window_id: pid.max(1) as u32,
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
const pid = front ? front.processIdentifier : 0;
JSON.stringify({ bundleId, appName, pid, winTitle: appName });
"#;
        let out = Command::new("osascript")
            .args(["-l", "JavaScript", "-e", jxa])
            .output()
            .await?;
        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let app_bundle_id = parsed["bundleId"].as_str().unwrap_or("unknown").to_string();
        let app_name = parsed["appName"].as_str().unwrap_or("macOS App").to_string();
        let pid = parsed["pid"].as_i64().unwrap_or(0) as i32;
        let (ax_title, ax_text, ax_secure) =
            tokio::task::spawn_blocking(move || read_frontmost_ax_in_process(pid))
                .await
                .unwrap_or_default();
        let window_title = if !ax_title.is_empty() {
            ax_title
        } else {
            parsed["winTitle"].as_str().unwrap_or("").to_string()
        };
        let visible_text = if ax_secure {
            String::new()
        } else if !ax_text.is_empty() {
            ax_text
        } else {
            format!("{app_name} — {window_title}")
        };
        let digest = Sha256::digest(format!("{app_bundle_id}:{window_title}").as_bytes());
        let dhash_u64 = u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]));

        Ok(ActiveWindowObservation {
            window_id: pid.max(1) as u32,
            app_bundle_id,
            app_name,
            window_title,
            browser_url: None,
            visible_text,
            extraction_method: ExtractionMethod::Accessibility,
            dhash_u64,
            is_private_browsing: false,
            is_secure_field_focused: ax_secure,
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
        if let Some(v) = self.run_sensor_json(&["audio-env"]).await {
            let mic_active = v["microphone_in_use"].as_bool().unwrap_or(false)
                || is_macos_default_mic_running();
            let detected_meeting_app = v["detected_meeting_app"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let detected_meeting_title = v["detected_meeting_title"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());
            let has_call_controls_ui = v["has_call_controls_ui"]
                .as_bool()
                .unwrap_or(detected_meeting_app.is_some());
            let system_audio_active = v["system_audio_active"]
                .as_bool()
                .unwrap_or(mic_active && detected_meeting_app.is_some());
            return Ok(AudioEnvironmentSignal {
                microphone_in_use: mic_active,
                system_audio_active,
                has_call_controls_ui,
                detected_meeting_app,
                detected_meeting_title,
            });
        }

        let mic_active = is_macos_default_mic_running();
        let win = self.observe_active_window().await.ok();
        let mut meeting_app = None;
        let mut meeting_title = None;
        if let Some(ref w) = win {
            let low = format!(
                "{} {} {} {:?}",
                w.app_bundle_id, w.app_name, w.window_title, w.browser_url
            )
            .to_lowercase();
            if low.contains("zoom.us") || low.contains("us.zoom.xos") || low.contains("zoom workplace") {
                meeting_app = Some("Zoom".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("meet.google.com") || low.contains("google meet") {
                meeting_app = Some("Google Meet".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("teams.microsoft")
                || low.contains("teams.live")
                || low.contains("teams.cloud.microsoft")
                || low.contains("com.microsoft.teams")
                || low.contains("microsoft teams")
            {
                meeting_app = Some("Microsoft Teams".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("slack") && low.contains("huddle") {
                meeting_app = Some("Slack Huddle".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("facetime") {
                meeting_app = Some("FaceTime".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("webex") {
                meeting_app = Some("Cisco Webex".into());
                meeting_title = Some(w.window_title.clone());
            } else if low.contains("whereby.com") || low.contains("meet.jit.si") {
                meeting_app = Some("Browser Meeting".into());
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

    async fn start_dual_audio_stream(
        &self,
        out_dir: &str,
        chunk_secs: u64,
    ) -> Result<DualAudioStreamSession> {
        let Some(bin) = self.sensor_bin.clone() else {
            anyhow::bail!("backtext-macos-sensor binary not found inside app bundle");
        };
        let out_dir_owned = out_dir.to_string();
        let chunk_str = chunk_secs.clamp(4, 20).to_string();

        let mut child = Command::new(bin)
            .args(["stream-dual-audio", &out_dir_owned, &chunk_str])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut child_stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open sensor stdout"))?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut stopping = false;

            loop {
                tokio::select! {
                    _ = &mut stop_rx, if !stopping => {
                        stopping = true;
                        if let Some(mut stdin) = child_stdin.take() {
                            let _ = stdin.write_all(b"stop\n").await;
                            let _ = stdin.shutdown().await;
                        }
                        // Allow up to 2.5s for the final WAV chunk to be flushed
                        let _ = tokio::time::timeout(std::time::Duration::from_millis(2500), async {
                            while let Ok(Some(line)) = lines.next_line().await {
                                if let Some(ev) = parse_stream_json_line(&line) {
                                    let _ = tx.send(ev);
                                }
                            }
                        }).await;
                        break;
                    }
                    next = lines.next_line() => {
                        match next {
                            Ok(Some(line)) => {
                                if let Some(ev) = parse_stream_json_line(&line) {
                                    if tx.send(ev).is_err() {
                                        break;
                                    }
                                }
                            }
                            _ => break,
                        }
                    }
                }
            }

            let _ = child.kill().await;
        });

        Ok(DualAudioStreamSession::new(rx, stop_tx))
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

fn parse_stream_json_line(line: &str) -> Option<DualAudioStreamEvent> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    match v["type"].as_str()? {
        "level" => Some(DualAudioStreamEvent::Level {
            mic_rms: v["mic_rms"].as_f64().unwrap_or(0.0) as f32,
            system_rms: v["system_rms"].as_f64().unwrap_or(0.0) as f32,
            mic_active: v["mic_active"].as_bool().unwrap_or(false),
            system_active: v["system_active"].as_bool().unwrap_or(false),
            elapsed_secs: v["elapsed_secs"].as_u64().unwrap_or(0),
            error: v["error"].as_str().map(|s| s.to_string()),
        }),
        "chunk" => Some(DualAudioStreamEvent::Chunk {
            seq: v["seq"].as_u64().unwrap_or(1),
            mic_wav_path: v["mic_wav_path"].as_str().map(|s| s.to_string()),
            mic_samples: v["mic_samples"].as_u64().unwrap_or(0) as usize,
            system_wav_path: v["system_wav_path"].as_str().map(|s| s.to_string()),
            system_samples: v["system_samples"].as_u64().unwrap_or(0) as usize,
        }),
        _ => None,
    }
}
