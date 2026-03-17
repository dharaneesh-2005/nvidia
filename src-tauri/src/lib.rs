pub mod pip;
pub mod ws_client;

use tauri::Manager;

/// Initialize the Tauri application with PiP support
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            pip::open_pip_window,
            pip::close_pip_window,
            pip::toggle_pip_window,
            pip::is_pip_open,
        ])
        .setup(|app| {
            // Hide the main window on startup - we'll use the web server
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
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
