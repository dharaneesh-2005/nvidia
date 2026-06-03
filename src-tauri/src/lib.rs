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
            pip::get_pip_state,
            pip::hide_pip_cursor,
            pip::show_pip_cursor,
            pip::set_pip_opacity,
            pip::set_pip_resizable,
            pip::pip_content_ready,
        ])
        .setup(|app| {
            // Hide the main window on startup - we'll use the web server
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
            
            // Register global shortcut Ctrl+Alt+P to toggle PiP
            let shortcut_manager = app.global_shortcut();
            let shortcut = Shortcut::new(
                Some(tauri_plugin_global_shortcut::Modifiers::CONTROL | tauri_plugin_global_shortcut::Modifiers::ALT),
                tauri_plugin_global_shortcut::Code::KeyP,
            );
            
            // Register with handler - send WebSocket message like the UI button does
            if let Err(e) = shortcut_manager.on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    println!("[Global Shortcut] Ctrl+Alt+P pressed - sending toggle_pip_window message");
                    // Send WebSocket message to backend (same as UI button)
                    tauri::async_runtime::spawn(async move {
                        // Use reqwest or a simple HTTP request to trigger the WebSocket broadcast
                        let client = reqwest::Client::new();
                        let _ = client.post("http://localhost:5000/api/pip-toggle")
                            .timeout(std::time::Duration::from_secs(2))
                            .send()
                            .await;
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
