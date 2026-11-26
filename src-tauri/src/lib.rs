pub mod controller;
pub mod daemon_loop;
pub mod http_server;
pub mod updater;

use backtext_core::BacktextEvent;
use controller::{format_shortcut_display, AppController};
use serde_json::Value;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutostartManagerExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
async fn backtext_rpc(
    controller: State<'_, AppController>,
    cmd: String,
    args: Value,
) -> Result<Value, String> {
    controller
        .dispatch_rpc(&cmd, args)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn toggle_hover_overlay(app: tauri::AppHandle) -> Result<bool, String> {
    if let Some(win) = app.get_webview_window("hover") {
        let visible = win.is_visible().unwrap_or(false);
        if visible {
            win.hide().map_err(|e| e.to_string())?;
            Ok(false)
        } else {
            // Keep Hover window invisible to screen-sharing capture (NSWindowSharingNone on macOS)
            let _ = win.set_content_protected(true);
            win.center().ok();
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().ok();
            let _ = win.emit("backtext://hover-shown", ());
            Ok(true)
        }
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn hide_hover_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("hover") {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_autostart_enabled(
    app: tauri::AppHandle,
    controller: State<'_, AppController>,
    enabled: bool,
) -> Result<bool, String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| e.to_string())?;
    } else {
        autolaunch.disable().map_err(|e| e.to_string())?;
    }
    let active = autolaunch.is_enabled().unwrap_or(enabled);
    controller.update_telemetry(|t| t.autostart_enabled = active);
    Ok(active)
}

#[derive(Default)]
struct ShortcutBindings {
    hover: parking_lot::Mutex<Option<Shortcut>>,
    voice_typing: parking_lot::Mutex<Option<Shortcut>>,
}

fn rebind_global_shortcuts(
    app: &tauri::AppHandle,
    bindings: &ShortcutBindings,
    hover_spec: &str,
    vt_spec: &str,
    vt_enabled: bool,
) -> Result<(), String> {
    let hover_sc = Shortcut::from_str(hover_spec.trim())
        .map_err(|e| format!("Ungültiges Hover-Shortcut-Format '{hover_spec}': {e}"))?;
    let vt_sc = if vt_enabled && !vt_spec.trim().is_empty() {
        Some(
            Shortcut::from_str(vt_spec.trim())
                .map_err(|e| format!("Ungültiges Voice-Typing-Shortcut-Format '{vt_spec}': {e}"))?,
        )
    } else {
        None
    };

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    gs.register(hover_sc).map_err(|e| {
        format!(
            "Shortcut '{hover_spec}' konnte nicht registriert werden (evtl. bereits durch macOS oder eine andere App belegt): {e}"
        )
    })?;
    *bindings.hover.lock() = Some(hover_sc);

    if let Some(vt) = vt_sc {
        if vt != hover_sc {
            if let Err(err) = gs.register(vt) {
                tracing::warn!("Could not register voice typing shortcut '{vt_spec}': {err}");
                *bindings.voice_typing.lock() = None;
                return Err(format!(
                    "Voice-Typing-Shortcut '{vt_spec}' konnte nicht registriert werden: {err}"
                ));
            }
            *bindings.voice_typing.lock() = Some(vt);
        } else {
            *bindings.voice_typing.lock() = None;
        }
    } else {
        *bindings.voice_typing.lock() = None;
    }
    Ok(())
}

#[tauri::command]
fn set_global_shortcut(
    app: tauri::AppHandle,
    controller: State<'_, AppController>,
    bindings: State<'_, ShortcutBindings>,
    shortcut: String,
) -> Result<String, String> {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        return Err("Shortcut darf nicht leer sein.".into());
    }
    let ai = controller.storage.get_ai_settings().unwrap_or_default();
    rebind_global_shortcuts(
        &app,
        &bindings,
        trimmed,
        &ai.voice_typing_shortcut,
        ai.voice_typing_enabled,
    )?;

    if let Ok(mut ai_mut) = controller.storage.get_ai_settings() {
        ai_mut.hover_shortcut = trimmed.to_string();
        let _ = controller.storage.save_ai_settings(&ai_mut);
    }
    let display = format_shortcut_display(trimmed);
    let tel = controller.update_telemetry(|t| t.global_shortcut = display.clone());
    controller
        .events
        .publish(BacktextEvent::DaemonTelemetryUpdated(tel));
    Ok(display)
}

#[tauri::command]
fn set_voice_typing_shortcut(
    app: tauri::AppHandle,
    controller: State<'_, AppController>,
    bindings: State<'_, ShortcutBindings>,
    shortcut: String,
) -> Result<String, String> {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        return Err("Shortcut darf nicht leer sein.".into());
    }
    let ai = controller.storage.get_ai_settings().unwrap_or_default();
    rebind_global_shortcuts(
        &app,
        &bindings,
        &ai.hover_shortcut,
        trimmed,
        true,
    )?;

    if let Ok(mut ai_mut) = controller.storage.get_ai_settings() {
        ai_mut.voice_typing_shortcut = trimmed.to_string();
        ai_mut.voice_typing_enabled = true;
        let _ = controller.storage.save_ai_settings(&ai_mut);
    }
    let display = format_shortcut_display(trimmed);
    let tel = controller.update_telemetry(|t| t.voice_typing_shortcut = display.clone());
    controller
        .events
        .publish(BacktextEvent::DaemonTelemetryUpdated(tel));
    Ok(display)
}

fn setup_menubar_tray(
    app: &tauri::App,
    tray_ctrl: AppController,
    shortcut_label: &str,
    vt_label: &str,
) -> tauri::Result<()> {
    let open_main = MenuItem::with_id(app, "open_main", "backtext öffnen", true, None::<&str>)?;
    let open_hover = MenuItem::with_id(
        app,
        "open_hover",
        &format!("Hover-Assistent ({shortcut_label})"),
        true,
        None::<&str>,
    )?;
    let toggle_vt = MenuItem::with_id(
        app,
        "toggle_voice_typing",
        &format!("Voice Typing starten/stoppen ({vt_label})"),
        true,
        None::<&str>,
    )?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let pause_15 = MenuItem::with_id(
        app,
        "pause_15",
        "Kontexterfassung 15 Min pausieren",
        true,
        None::<&str>,
    )?;
    let pause_60 = MenuItem::with_id(
        app,
        "pause_60",
        "Kontexterfassung 1 Std pausieren",
        true,
        None::<&str>,
    )?;
    let resume = MenuItem::with_id(
        app,
        "resume",
        "Kontexterfassung fortsetzen",
        true,
        None::<&str>,
    )?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "backtext beenden", true, None::<&str>)?;

    let tray_menu = Menu::with_items(
        app,
        &[
            &open_main,
            &open_hover,
            &toggle_vt,
            &sep1,
            &pause_15,
            &pause_60,
            &resume,
            &sep2,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::new()
        .menu(&tray_menu)
        .tooltip("backtext — Ambient Context & Meeting Memory")
        .on_menu_event(move |app_handle, event| match event.id.as_ref() {
            "open_main" => {
                if let Some(w) = app_handle.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            "open_hover" => {
                let _ = toggle_hover_overlay(app_handle.clone());
            }
            "toggle_voice_typing" => {
                let ctrl = tray_ctrl.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = ctrl.toggle_voice_typing("system").await {
                        tracing::warn!("Menubar voice typing failed: {err}");
                    }
                });
            }
            "pause_15" => {
                tray_ctrl.set_capture_pause(Some(15), None);
            }
            "pause_60" => {
                tray_ctrl.set_capture_pause(Some(60), None);
            }
            "resume" => {
                tray_ctrl.set_capture_pause(None, None);
            }
            "quit" => {
                app_handle.exit(0);
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    let _ = builder.build(app)?;
    Ok(())
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let controller = AppController::open_default().expect("Failed to initialize backtext storage");
    let setup_controller = controller.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--silent-launch"]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let is_vt = app
                        .try_state::<ShortcutBindings>()
                        .and_then(|b| *b.voice_typing.lock())
                        .map(|vt| vt == *shortcut)
                        .unwrap_or(false);
                    if is_vt {
                        if let Some(ctrl_state) = app.try_state::<AppController>() {
                            let ctrl = ctrl_state.inner().clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(err) = ctrl.toggle_voice_typing("system").await {
                                    tracing::warn!("Voice typing shortcut failed: {err}");
                                }
                            });
                        }
                    } else {
                        let _ = toggle_hover_overlay(app.clone());
                    }
                })
                .build(),
        )
        .manage(controller)
        .manage(ShortcutBindings::default())
        .setup(move |app| {
            // Protect Hover assistant and global Dictation HUD from screen capture / screen sharing
            if let Some(hover_win) = app.get_webview_window("hover") {
                let _ = hover_win.set_content_protected(true);
            }
            if let Some(dict_win) = app.get_webview_window("dictation") {
                let _ = dict_win.set_content_protected(true);
                let _ = dict_win.set_focusable(false);
                let _ = dict_win.set_always_on_top(true);
            }

            // Read initial autostart state cleanly
            let autostart_active = app.autolaunch().is_enabled().unwrap_or(false);
            setup_controller.update_telemetry(|t| t.autostart_enabled = autostart_active);

            // Register user-configured global hotkeys (Hover Overlay + System-Wide Voice Typing)
            let saved_ai = setup_controller.storage.get_ai_settings().unwrap_or_default();
            let hover_spec = saved_ai.hover_shortcut.clone();
            let vt_spec = saved_ai.voice_typing_shortcut.clone();
            let vt_enabled = saved_ai.voice_typing_enabled;
            let bindings = app.state::<ShortcutBindings>();
            if let Err(err) = rebind_global_shortcuts(
                app.handle(),
                &bindings,
                &hover_spec,
                &vt_spec,
                vt_enabled,
            ) {
                tracing::warn!("Initial global shortcut binding warning: {err}");
                // Ensure fallback hover shortcut is attempted
                let fallback_hover = Shortcut::new(Some(Modifiers::ALT), Code::Space);
                if app.global_shortcut().register(fallback_hover).is_ok() {
                    *bindings.hover.lock() = Some(fallback_hover);
                }
            }
            let display_shortcut = format_shortcut_display(&hover_spec);
            let display_vt = format_shortcut_display(&vt_spec);
            setup_controller.update_telemetry(|t| {
                t.global_shortcut = display_shortcut.clone();
                t.voice_typing_shortcut = display_vt.clone();
            });

            // Forward Rust EventBus events directly into Tauri webview windows for live token & audio streaming,
            // and control the global non-activating Dictation HUD window over all apps (Word, Safari, etc.).
            let mut rx = setup_controller.events.subscribe();
            let app_handle_for_events = app.handle().clone();
            let hud_gen = Arc::new(AtomicU64::new(0));
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            if let BacktextEvent::VoiceTypingStateChanged {
                                active,
                                ref mode,
                                ref status,
                                ..
                            } = ev
                            {
                                if mode == "system" {
                                    let next_gen = hud_gen.fetch_add(1, Ordering::SeqCst) + 1;
                                    if let Some(hud) =
                                        app_handle_for_events.get_webview_window("dictation")
                                    {
                                        let _ = hud.set_content_protected(true);
                                        let _ = hud.set_focusable(false);
                                        let _ = hud.set_always_on_top(true);
                                        if let Ok(Some(mon)) = hud.primary_monitor() {
                                            let m_pos = mon.position();
                                            let m_size = mon.size();
                                            let scale = mon.scale_factor();
                                            let w_size = hud.outer_size().unwrap_or_else(|_| {
                                                tauri::PhysicalSize::new(
                                                    (360.0 * scale) as u32,
                                                    (74.0 * scale) as u32,
                                                )
                                            });
                                            let x = m_pos.x
                                                + ((m_size.width as i32 - w_size.width as i32) / 2);
                                            let y = m_pos.y + m_size.height as i32
                                                - w_size.height as i32
                                                - (92.0 * scale) as i32;
                                            let _ = hud.set_position(tauri::Position::Physical(
                                                tauri::PhysicalPosition::new(x, y),
                                            ));
                                        }
                                        let _ = hud.show();
                                    }
                                    // Keep HUD visible while listening or transcribing; auto-hide 2.6s after final confirmation
                                    if !active && !status.contains("Transkribiert & bereinigt") {
                                        let handle_hide = app_handle_for_events.clone();
                                        let gen_ref = hud_gen.clone();
                                        tauri::async_runtime::spawn(async move {
                                            tokio::time::sleep(std::time::Duration::from_millis(
                                                2600,
                                            ))
                                            .await;
                                            if gen_ref.load(Ordering::SeqCst) == next_gen {
                                                if let Some(hud) =
                                                    handle_hide.get_webview_window("dictation")
                                                {
                                                    let _ = hud.hide();
                                                }
                                            }
                                        });
                                    }
                                }
                            }

                            if let BacktextEvent::UpdateInstallProgress { ref stage, .. } = ev {
                                if stage == "restarting" {
                                    let handle_exit = app_handle_for_events.clone();
                                    tauri::async_runtime::spawn(async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(500))
                                            .await;
                                        handle_exit.exit(0);
                                    });
                                }
                            }

                            let _ = app_handle_for_events.emit("backtext://event", &ev);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            // Start autonomous background capture, meeting detector & calendar sync loop
            let daemon_ctrl = setup_controller.clone();
            tauri::async_runtime::spawn(async move {
                daemon_loop::spawn_background_daemon(daemon_ctrl);
            });

            // Spawn embedded Streamable HTTP MCP & SSE server on localhost:39421
            let mcp_port = setup_controller
                .storage
                .get_mcp_policy()
                .map(|p| p.http_port)
                .unwrap_or(39421);
            let server_ctrl = setup_controller.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) =
                    http_server::spawn_local_http_and_mcp_server(server_ctrl, mcp_port).await
                {
                    tracing::warn!("Local HTTP/MCP server already bound or unavailable: {err}");
                }
            });

            if let Err(err) = setup_menubar_tray(
                app,
                setup_controller.clone(),
                &display_shortcut,
                &display_vt,
            ) {
                tracing::warn!("Menubar tray initialization skipped: {err}");
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Keep running quietly in background menubar when closing main window
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            backtext_rpc,
            toggle_hover_overlay,
            hide_hover_overlay,
            set_autostart_enabled,
            set_global_shortcut,
            set_voice_typing_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running backtext Tauri application");
}
