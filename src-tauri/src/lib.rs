pub mod controller;
pub mod http_server;

use controller::AppController;
use serde_json::Value;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, State,
};

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
            Ok(true)
        }
    } else {
        Ok(false)
    }
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init()
        .ok();

    let controller = AppController::open_default().expect("Failed to initialize backtext storage");
    let http_controller = controller.clone();

    tauri::Builder::default()
        .manage(controller)
        .setup(move |app| {
            // Protect Hover assistant from screen capture / screen sharing
            if let Some(hover_win) = app.get_webview_window("hover") {
                let _ = hover_win.set_content_protected(true);
            }

            // Spawn embedded Streamable HTTP MCP server on localhost:39421
            let mcp_port = http_controller
                .storage
                .get_mcp_policy()
                .map(|p| p.http_port)
                .unwrap_or(39421);
            let server_ctrl = http_controller.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = http_server::spawn_local_http_and_mcp_server(server_ctrl, mcp_port).await {
                    tracing::warn!("Local HTTP/MCP server already bound or unavailable: {err}");
                }
            });

            // Build calm macOS Menubar / System Tray menu
            let open_main = MenuItem::with_id(app, "open_main", "backtext öffnen", true, None::<&str>)?;
            let open_hover = MenuItem::with_id(app, "open_hover", "Hover-Assistent (⌥ Space)", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let pause_15 = MenuItem::with_id(app, "pause_15", "Kontexterfassung 15 Min pausieren", true, None::<&str>)?;
            let pause_60 = MenuItem::with_id(app, "pause_60", "Kontexterfassung 1 Std pausieren", true, None::<&str>)?;
            let resume = MenuItem::with_id(app, "resume", "Kontexterfassung fortsetzen", true, None::<&str>)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "backtext beenden", true, None::<&str>)?;

            let tray_menu = Menu::with_items(
                app,
                &[&open_main, &open_hover, &sep1, &pause_15, &pause_60, &resume, &sep2, &quit],
            )?;

            let tray_ctrl = http_controller.clone();
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
        .invoke_handler(tauri::generate_handler![backtext_rpc, toggle_hover_overlay])
        .run(tauri::generate_context!())
        .expect("error while running backtext Tauri application");
}
