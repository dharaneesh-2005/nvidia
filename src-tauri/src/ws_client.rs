//! WebSocket Client for Tauri PiP
//!
//! This module connects to the backend WebSocket and listens for
//! "open_pip_window" and "close_pip_window" messages from the browser.

use tauri::AppHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};

/// Start WebSocket client to listen for PiP commands from backend
pub async fn start_ws_client(app: AppHandle, url: String) {
    println!("[WS Client] Connecting to {}", url);
    
    loop {
        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                println!("[WS Client] Connected to backend WebSocket");
                let (mut write, mut read) = ws_stream.split();
                
                // Send a ping to keep connection alive
                let _ = write.send(Message::Text("ping".to_string())).await;
                
                // Listen for messages
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            println!("[WS Client] Received: {}", text);
                            
                            // Try to parse as JSON
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(msg_type) = json.get("type").and_then(|v| v.as_str()) {
                                    match msg_type {
                                        "open_pip_window" => {
                                            println!("[WS Client] Opening PiP window");
                                            let app_clone = app.clone();
                                            tokio::spawn(async move {
                                                let _ = crate::pip::open_pip_window(
                                                    app_clone, None, None, None, None
                                                ).await;
                                            });
                                        }
                                        "close_pip_window" => {
                                            println!("[WS Client] Closing PiP window");
                                            let app_clone = app.clone();
                                            tokio::spawn(async move {
                                                let _ = crate::pip::close_pip_window(app_clone).await;
                                            });
                                        }
                                        "toggle_pip_window" => {
                                            println!("[WS Client] Toggling PiP window");
                                            let app_clone = app.clone();
                                            tokio::spawn(async move {
                                                let _ = crate::pip::toggle_pip_window(app_clone).await;
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Ok(Message::Close(_)) => {
                            println!("[WS Client] Connection closed");
                            break;
                        }
                        Err(e) => {
                            println!("[WS Client] Error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                println!("[WS Client] Failed to connect: {}. Retrying in 3 seconds...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        }
    }
}
