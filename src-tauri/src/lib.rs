pub mod pip;
pub mod ws_client;

use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// Initialize the Tauri application with PiP support
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            pip::open_pip_window,
            pip::close_pip_window,
            pip::toggle_pip_window,
            pip::is_pip_open,
            pip::minimize_pip_window,
        ])
        .setup(|app| {
            // Hide the main window on startup - we'll use the web server
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
            
            // Register global shortcut Ctrl+Alt+P to toggle PiP
            let app_handle = app.handle().clone();
            let shortcut_manager = app.global_shortcut();
            let shortcut = Shortcut::new(
                Some(tauri_plugin_global_shortcut::Modifiers::CONTROL | tauri_plugin_global_shortcut::Modifiers::ALT),
                tauri_plugin_global_shortcut::Code::KeyP,
            );
            
            // Register with handler - only trigger on key press (not release)
            if let Err(e) = shortcut_manager.on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    println!("[Global Shortcut] Ctrl+Alt+P pressed - toggling PiP window");
                    let app_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = pip::toggle_pip_window(app_handle).await;
                    });
                }
            }) {
                println!("[Global Shortcut] Failed to register Ctrl+Alt+P: {}", e);
            } else {
                println!("[Global Shortcut] Registered Ctrl+Alt+P to toggle PiP");
            }
            
            // Start WebSocket client to listen for PiP commands
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Try to connect to the backend WebSocket (port 5000)
                let ws_url = "ws://localhost:5000/ws";
                ws_client::start_ws_client(app_handle, ws_url.to_string()).await;
            });
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
