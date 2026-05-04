mod modules;

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::collections::{VecDeque, HashSet};
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;
use tracing::{info, error};
use crossbeam_channel::RecvTimeoutError;

use modules::{
    audio::AudioCapture,
    audio_processor::AudioProcessor,
    screen::ScreenCapture,
    groq::{GroqClient, ConversationMessage},
    code::CodeManager,
    config::Config,
    search::SearchHotkey,
    debug::DebugHotkey,
    mcq::McqHandler,
    mcq_hotkey::McqHotkey,
};

type ConversationHistory = Arc<RwLock<Vec<ConversationMessage>>>;
type MessageBuffer = Arc<RwLock<VecDeque<String>>>;

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
    groq: Arc<GroqClient>,
    code_manager: Arc<CodeManager>,
    conversation: ConversationHistory,
    message_buffer: MessageBuffer,
    client_connections: Arc<AtomicUsize>,
    search_hotkey: Arc<tokio::sync::Mutex<SearchHotkey>>,
    multi_capture_cancel: Arc<AtomicBool>,
}

const SILENCE_TIMEOUT: Duration = Duration::from_millis(1200);
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(1000);
const MIN_AVERAGE_ENERGY: f32 = 0.025;
const ENERGY_DROP_THRESHOLD: f32 = 0.55;
const ENERGY_DROP_TIMEOUT: Duration = Duration::from_millis(350);
const MIN_SPEECH_CHUNKS: usize = 12;
const MAX_AUDIO_DURATION: Duration = Duration::from_secs(30);
const SPEECH_ENERGY_THRESHOLD: f32 = 0.012;
const MIN_PEAK_ENERGY: f32 = 0.020;

async fn send_or_buffer(tx: &broadcast::Sender<String>, msg: String, buffer: &MessageBuffer, connections: &Arc<AtomicUsize>) {
    if connections.load(Ordering::SeqCst) > 0 {
        let _ = tx.send(msg);
    } else {
        let mut buf = buffer.write().await;
        buf.push_back(msg);
        if buf.len() > 100 {
            buf.pop_front();
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    info!("Starting Nvidia...");
    
    let config = Config::load().expect("Failed to load config");
    let (tx, _rx) = broadcast::channel(100);
    
    let groq = Arc::new(GroqClient::new(config.groq_api_key.clone()));
    let code_manager = Arc::new(CodeManager::new(config.project_path.clone()));
    
    // Start search hotkey listener
    let search_hotkey = Arc::new(tokio::sync::Mutex::new(SearchHotkey::new()));
    let multi_capture_cancel = Arc::new(AtomicBool::new(false));
    tokio::spawn(start_search_hotkey_listener(tx.clone(), search_hotkey.clone()));
    
    let state = AppState {
        tx: tx.clone(),
        groq: groq.clone(),
        code_manager: code_manager.clone(),
        conversation: Arc::new(RwLock::new(Vec::new())),
        message_buffer: Arc::new(RwLock::new(VecDeque::new())),
        client_connections: Arc::new(AtomicUsize::new(0)),
        search_hotkey: search_hotkey,
        multi_capture_cancel: multi_capture_cancel.clone(),
    };
    
    // Start audio capture with proper streaming
    tokio::spawn(start_audio_capture(tx.clone(), groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone()));
    
    // Start screen capture hotkey listener
    tokio::spawn(start_screen_capture_hotkey(
        tx.clone(),
        groq.clone(),
        config.hotkey.clone(),
        state.conversation.clone(),
        state.message_buffer.clone(),
        state.client_connections.clone(),
        multi_capture_cancel.clone(),
    ));
    
    // Start debug hotkey listener
    tokio::spawn(start_debug_hotkey_listener(tx.clone(), groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone()));
    
    // Start MCQ hotkey listener
    tokio::spawn(start_mcq_hotkey_listener(tx.clone(), groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone()));
    
    // Build web server
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .route("/api/code/files", get(get_files))
        .route("/api/code/content", post(get_file_content))
        .route("/api/code/query", post(query_code))
        .route("/api/pip-toggle", post(pip_toggle_handler))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);
    
    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Server running on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn pip_toggle_handler(State(state): State<AppState>) -> impl IntoResponse {
    info!("[PiP] HTTP toggle request received - broadcasting toggle_pip_window message");
    // Broadcast the toggle message to all WebSocket clients
    // This will be handled by the Tauri app and the browser UI
    let _ = state.tx.send(serde_json::json!({
        "type": "toggle_pip_window"
    }).to_string());
    
    // Also broadcast state change request to sync all clients
    let _ = state.tx.send(serde_json::json!({
        "type": "get_pip_state"
    }).to_string());
    
    axum::response::Json(serde_json::json!({
        "status": "ok",
        "message": "PiP toggle broadcasted"
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

struct ConnectionGuard {
    count: Arc<AtomicUsize>,
}

impl ConnectionGuard {
    fn new(count: Arc<AtomicUsize>) -> Self {
        count.fetch_add(1, Ordering::SeqCst);
        Self { count }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    let _conn_guard = ConnectionGuard::new(state.client_connections.clone());
    
    // Send buffered messages first
    {
        let mut buffer = state.message_buffer.write().await;
        while let Some(msg) = buffer.pop_front() {
            if socket.send(axum::extract::ws::Message::Text(msg)).await.is_err() {
                return;
            }
        }
    }
    
    loop {
        tokio::select! {
            msg = rx.recv() => {
                if let Ok(msg) = msg {
                    if socket.send(axum::extract::ws::Message::Text(msg.clone())).await.is_err() {
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                if let Some(Ok(axum::extract::ws::Message::Text(text))) = msg {
                    if text == "capture_screen" {
                        tokio::spawn(handle_screen_capture(state.tx.clone(), state.groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone()));
                    } else if text == "capture_mcq" {
                        tokio::spawn(handle_mcq_capture(state.tx.clone(), state.groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone()));
                    } else if text == "debug_code" {
                        tokio::spawn(handle_debug_code(state.tx.clone(), state.groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone()));
                    } else if text == "search_closed" {
                        let hotkey = state.search_hotkey.lock().await;
                        hotkey.deactivate();
                    } else if text == "multi_capture_cancel" {
                        state.multi_capture_cancel.store(true, Ordering::SeqCst);
                    } else if text.starts_with("{") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if msg_type == "open_pip_window" {
                                // Broadcast to all connected clients (including Tauri app)
                                let _ = state.tx.send(serde_json::json!({
                                    "type": "open_pip_window"
                                }).to_string());
                            } else if msg_type == "close_pip_window" {
                                // Broadcast to all connected clients (including Tauri app)
                                let _ = state.tx.send(serde_json::json!({
                                    "type": "close_pip_window"
                                }).to_string());
                            } else if msg_type == "set_pip_opacity" {
                                let opacity = json.get("opacity").and_then(|v| v.as_f64()).unwrap_or(1.0);
                                // Broadcast to Tauri app
                                let _ = state.tx.send(serde_json::json!({
                                    "type": "set_pip_opacity",
                                    "opacity": opacity
                                }).to_string());
                            } else if msg_type == "solve_problem" {
                                let question = json.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                if !question.is_empty() {
                                    let tx_clone = state.tx.clone();
                                    let groq = state.groq.clone();
                                    let conversation = state.conversation.clone();
                                    let buffer = state.message_buffer.clone();
                                    let connected = state.client_connections.clone();
                                    tokio::spawn(async move {
                                        let history = conversation.read().await.clone();
                                        match groq.solve_coding_problem(&question, &history).await {
                                            Ok(solution) => {
                                                conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: question.clone() });
                                                conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: solution.clone() });
                                                send_or_buffer(&tx_clone, serde_json::json!({
                                                    "type": "answer",
                                                    "text": solution
                                                }).to_string(), &buffer, &connected).await;
                                            },
                                            Err(e) => {
                                                send_or_buffer(&tx_clone, serde_json::json!({
                                                    "type": "answer",
                                                    "text": format!("Error: {}", e)
                                                }).to_string(), &buffer, &connected).await;
                                            }
                                        }
                                    });
                                }
                            } else if msg_type == "manual_question" {
                                let question = json.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                info!("🔍 [SEARCH LOG] Received manual_question: '{}'", question);
                                
                                if !question.is_empty() {
                                    let tx_clone = state.tx.clone();
                                    let groq = state.groq.clone();
                                    let conversation = state.conversation.clone();
                                    let buffer = state.message_buffer.clone();
                                    let connected = state.client_connections.clone();
                                    tokio::spawn(async move {
                                        info!("🔍 [SEARCH LOG] Processing question: '{}'", question);
                                        
                                        send_or_buffer(&tx_clone, serde_json::json!({
                                            "type": "transcription",
                                            "text": question
                                        }).to_string(), &buffer, &connected).await;

                                        let history = conversation.read().await.clone();
                                        info!("🔍 [SEARCH LOG] Conversation history length: {}", history.len());
                                        
                                        // Log the conversation history for debugging
                                        for (i, msg) in history.iter().enumerate() {
                                            let content_preview = if msg.content.chars().count() > 100 {
                                                format!("{}...", msg.content.chars().take(100).collect::<String>())
                                            } else {
                                                msg.content.clone()
                                            };
                                            info!("🔍 [SEARCH LOG] History[{}]: {} - '{}'", i, msg.role, content_preview);
                                        }
                                        
                                        info!("🔍 [SEARCH LOG] Sending to AI model: '{}'", question);
                                        
                                        // Try with 4-second timeout, retry once if it fails
                                        let timeout_duration = Duration::from_secs(4);
                                        let mut attempt = 1;
                                        let max_attempts = 2;
                                        
                                        loop {
                                            info!("🔍 [SEARCH LOG] Attempt {} of {}", attempt, max_attempts);
                                            
                                            let result = tokio::time::timeout(
                                                timeout_duration,
                                                groq.chat_with_history(&question, &history)
                                            ).await;
                                            
                                            match result {
                                                Ok(Ok(answer)) => {
                                                    info!("🔍 [SEARCH LOG] AI response received (length: {})", answer.len());
                                                    let answer_preview = if answer.chars().count() > 200 {
                                                        format!("{}...", answer.chars().take(200).collect::<String>())
                                                    } else {
                                                        answer.clone()
                                                    };
                                                    info!("🔍 [SEARCH LOG] AI response preview: '{}'", answer_preview);
                                                    
                                                    conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: question.clone() });
                                                    conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: answer.clone() });
                                                    send_or_buffer(&tx_clone, serde_json::json!({
                                                        "type": "answer",
                                                        "text": answer
                                                    }).to_string(), &buffer, &connected).await;
                                                    break;
                                                },
                                                Ok(Err(e)) => {
                                                    error!("🔍 [SEARCH LOG] AI request failed: {}", e);
                                                    if attempt < max_attempts {
                                                        info!("🔍 [SEARCH LOG] Retrying after error...");
                                                        attempt += 1;
                                                        continue;
                                                    }
                                                    send_or_buffer(&tx_clone, serde_json::json!({
                                                        "type": "answer",
                                                        "text": format!("Error: {}", e)
                                                    }).to_string(), &buffer, &connected).await;
                                                    break;
                                                },
                                                Err(_) => {
                                                    error!("🔍 [SEARCH LOG] AI request timed out after {} seconds", timeout_duration.as_secs());
                                                    if attempt < max_attempts {
                                                        info!("🔍 [SEARCH LOG] Retrying after timeout...");
                                                        attempt += 1;
                                                        continue;
                                                    }
                                                    send_or_buffer(&tx_clone, serde_json::json!({
                                                        "type": "answer",
                                                        "text": "Request timed out after retries. Please try asking again."
                                                    }).to_string(), &buffer, &connected).await;
                                                    break;
                                                }
                                            }
                                        }
                                    });
                                } else {
                                    info!("🔍 [SEARCH LOG] Empty question received, ignoring");
                                }
                            } else if msg_type == "mic_audio" {
                                let audio_base64 = json.get("audio").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                if !audio_base64.is_empty() {
                                    let tx_clone = state.tx.clone();
                                    let groq = state.groq.clone();
                                    let conversation = state.conversation.clone();
                                    let buffer = state.message_buffer.clone();
                                    let connected = state.client_connections.clone();
                                    tokio::spawn(async move {
                                        send_or_buffer(&tx_clone, serde_json::json!({
                                            "type": "transcription",
                                            "text": "Processing microphone audio..."
                                        }).to_string(), &buffer, &connected).await;

                                        match base64::decode(&audio_base64) {
                                            Ok(audio_data) => {
                                                match groq.transcribe(&audio_data).await {
                                                    Ok(transcription) => {
                                                        let text = transcription.trim();
                                                        if !text.is_empty() {
                                                            // Update the same box with transcription
                                                            send_or_buffer(&tx_clone, serde_json::json!({
                                                                "type": "update_transcription",
                                                                "text": text
                                                            }).to_string(), &buffer, &connected).await;

                                                            let history = conversation.read().await.clone();
                                                            match groq.chat_with_history(text, &history).await {
                                                                Ok(answer) => {
                                                                    conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: text.to_string() });
                                                                    conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: answer.clone() });
                                                                    send_or_buffer(&tx_clone, serde_json::json!({
                                                                        "type": "answer",
                                                                        "text": answer
                                                                    }).to_string(), &buffer, &connected).await;
                                                                },
                                                                Err(e) => {
                                                                    send_or_buffer(&tx_clone, serde_json::json!({
                                                                        "type": "answer",
                                                                        "text": format!("Error: {}", e)
                                                                    }).to_string(), &buffer, &connected).await;
                                                                }
                                                            }
                                                        }
                                                    },
                                                    Err(e) => {
                                                        send_or_buffer(&tx_clone, serde_json::json!({
                                                            "type": "answer",
                                                            "text": format!("Transcription error: {}", e)
                                                        }).to_string(), &buffer, &connected).await;
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                send_or_buffer(&tx_clone, serde_json::json!({
                                                    "type": "answer",
                                                    "text": format!("Audio decode error: {}", e)
                                                }).to_string(), &buffer, &connected).await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }
}

async fn get_files(State(state): State<AppState>) -> Json<serde_json::Value> {
    let files = state.code_manager.get_file_tree();
    Json(files)
}

async fn get_file_content(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let path = payload["path"].as_str().unwrap_or("");
    let content = state.code_manager.get_file_content(path);
    Json(serde_json::json!({ "content": content }))
}

async fn query_code(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let query = payload["query"].as_str().unwrap_or("");
    let context = state.code_manager.get_relevant_context(query);
    
    let response = state.groq.chat(&format!(
        "Context:\n{}\n\nQuestion: {}",
        context, query
    )).await.unwrap_or_else(|_| "Error processing query".to_string());
    
    Json(serde_json::json!({ "response": response }))
}

async fn start_audio_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    let (_audio_capture, audio_receiver) = AudioCapture::new();
    
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        rt.block_on(async {
            let mut processor = AudioProcessor::new(
                audio_receiver,
                groq,
                tx,
                conversation,
                buffer,
                connected
            );
            processor.run().await;
        });
    });
}



async fn handle_screen_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => {
            process_screenshot(vec![image_data], &groq, &tx, conversation, buffer, connected).await;
        }
        Err(e) => {
            error!("Screen capture failed: {}", e);
        }
    }
}

async fn handle_debug_code(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => {
            process_debug_screenshot(image_data, &groq, &tx, conversation, buffer, connected).await;
        }
        Err(e) => {
            error!("Debug capture failed: {}", e);
        }
    }
}

async fn handle_mcq_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => {
            process_mcq_screenshot(image_data, groq, &tx, conversation, buffer, connected).await;
        }
        Err(e) => {
            error!("MCQ capture failed: {}", e);
        }
    }
}

async fn start_screen_capture_hotkey(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, hotkey: String, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>, multi_capture_cancel: Arc<AtomicBool>) {
    let mut screen_capture = ScreenCapture::new(hotkey);
    let mut multi_buffer: Vec<String> = Vec::new();
    
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        if multi_capture_cancel.swap(false, Ordering::SeqCst) {
            if !multi_buffer.is_empty() {
                multi_buffer.clear();
            }
            let _ = tx.send(serde_json::json!({
                "type": "multi_capture_update",
                "count": 0
            }).to_string());
            info!("Multi-capture buffer cleared (UI cancel).");
        }
        
        if let Some(image_data) = screen_capture.check_multi_capture() {
            multi_buffer.push(image_data);
            let _ = tx.send(serde_json::json!({
                "type": "multi_capture_update",
                "count": multi_buffer.len()
            }).to_string());
            info!("Multi-capture chunk buffered ({}).", multi_buffer.len());
        }

        if screen_capture.check_cancel() {
            if !multi_buffer.is_empty() {
                multi_buffer.clear();
                let _ = tx.send(serde_json::json!({
                    "type": "multi_capture_update",
                    "count": 0
                }).to_string());
            }
            info!("Multi-capture buffer cleared.");
        }
        
        if let Some(image_data) = screen_capture.check_capture() {
            if !multi_buffer.is_empty() {
                multi_buffer.push(image_data);
                let images = std::mem::take(&mut multi_buffer);
                let _ = tx.send(serde_json::json!({
                    "type": "multi_capture_update",
                    "count": 0
                }).to_string());
                process_screenshot(images, &groq, &tx, conversation.clone(), buffer.clone(), connected.clone()).await;
            } else {
                process_screenshot(vec![image_data], &groq, &tx, conversation.clone(), buffer.clone(), connected.clone()).await;
            }
        }
    }
}

async fn start_search_hotkey_listener(tx: broadcast::Sender<String>, search_hotkey: Arc<tokio::sync::Mutex<SearchHotkey>>) {
    loop {
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        let mut hotkey = search_hotkey.lock().await;
        
        if hotkey.check_triggered() {
            info!("Search hotkey triggered - sending to browser");
            let _ = tx.send(serde_json::json!({
                "type": "toggle_search"
            }).to_string());
        }
        
        if hotkey.check_backspace() {
            let _ = tx.send(serde_json::json!({
                "type": "search_backspace"
            }).to_string());
        }

        if hotkey.check_enter() {
            let _ = tx.send(serde_json::json!({
                "type": "search_enter"
            }).to_string());
        }

        let keystrokes = hotkey.get_keystrokes();
        if !keystrokes.is_empty() {
            let _ = tx.send(serde_json::json!({
                "type": "search_keystroke",
                "text": keystrokes
            }).to_string());
        }
    }
}

async fn start_debug_hotkey_listener(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    let mut debug_hotkey = DebugHotkey::new();
    
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if debug_hotkey.check_triggered() {
            info!("Debug hotkey triggered - capturing screen for error analysis");
            match ScreenCapture::capture_now() {
                Ok(image_data) => {
                    process_debug_screenshot(image_data, &groq, &tx, conversation.clone(), buffer.clone(), connected.clone()).await;
                }
                Err(e) => error!("Debug capture failed: {}", e),
            }
        }
    }
}

async fn start_mcq_hotkey_listener(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    let mut mcq_hotkey = McqHotkey::new();
    
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if mcq_hotkey.check_triggered() {
            info!("MCQ hotkey (Ctrl+Alt+Q) triggered - capturing screen for MCQ analysis");
            match ScreenCapture::capture_now() {
                Ok(image_data) => {
                    process_mcq_screenshot(image_data, groq.clone(), &tx, conversation.clone(), buffer.clone(), connected.clone()).await;
                }
                Err(e) => error!("MCQ capture failed: {}", e),
            }
        }
    }
}

fn clean_analysis_text(analysis: &str) -> String {
    analysis
        .trim()
        .strip_prefix("```json")
        .or_else(|| analysis.trim().strip_prefix("```"))
        .unwrap_or(analysis.trim())
        .strip_suffix("```")
        .unwrap_or(analysis.trim())
        .trim()
        .to_string()
}

fn push_unique(parts: &mut Vec<String>, value: Option<&str>) {
    if let Some(v) = value {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            return;
        }
        if !parts.iter().any(|p| p == trimmed) {
            parts.push(trimmed.to_string());
        }
    }
}

fn merge_cleaned_analyses(cleaned_list: &[String]) -> serde_json::Value {
    let mut parsed = Vec::new();
    let mut confidence_values = Vec::new();
    let mut recapture_reason: Option<String> = None;
    
    for raw in cleaned_list {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
            if value["needs_recapture"].as_bool().unwrap_or(false) {
                if recapture_reason.is_none() {
                    recapture_reason = value["reason"].as_str().map(|s| s.to_string());
                }
            }
            if let Some(c) = value["confidence"].as_f64() {
                confidence_values.push(c);
            }
            parsed.push(value);
        }
    }
    
    if let Some(reason) = recapture_reason {
        return serde_json::json!({
            "needs_recapture": true,
            "reason": reason
        });
    }
    
    if parsed.is_empty() {
        return serde_json::json!({
            "type": "GENERAL",
            "content": cleaned_list.join("\n")
        });
    }
    
    let has_dsa = parsed.iter().any(|v| v["type"].as_str() == Some("DSA_PROBLEM"));
    let has_system = parsed.iter().any(|v| v["type"].as_str() == Some("SYSTEM_DESIGN"));
    let has_debug = parsed.iter().any(|v| v["type"].as_str() == Some("DEBUG_ERROR"));
    let has_logical = parsed.iter().any(|v| v["type"].as_str() == Some("LOGICAL_PUZZLE"));
    
    let confidence = if confidence_values.is_empty() {
        0.9
    } else {
        confidence_values.iter().cloned().fold(1.0, f64::min)
    };
    
    if has_dsa {
        let sources: Vec<&serde_json::Value> = parsed
            .iter()
            .filter(|v| v["type"].as_str() == Some("DSA_PROBLEM"))
            .collect();
        
        let mut title = String::new();
        let mut descriptions = Vec::new();
        let mut input_formats = Vec::new();
        let mut output_formats = Vec::new();
        let mut constraints = HashSet::new();
        let mut examples = Vec::new();
        let mut example_keys = HashSet::new();
        
        for v in sources {
            if title.is_empty() {
                if let Some(t) = v["title"].as_str() {
                    title = t.to_string();
                }
            }
            push_unique(&mut descriptions, v["description"].as_str());
            push_unique(&mut input_formats, v["input_format"].as_str());
            push_unique(&mut output_formats, v["output_format"].as_str());
            
            if let Some(arr) = v["constraints"].as_array() {
                for c in arr {
                    if let Some(s) = c.as_str() {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            constraints.insert(trimmed.to_string());
                        }
                    }
                }
            }
            
            if let Some(arr) = v["examples"].as_array() {
                for ex in arr {
                    let input = ex["input"].as_str().unwrap_or("").trim();
                    let output = ex["output"].as_str().unwrap_or("").trim();
                    let key = format!("{}||{}", input, output);
                    if !input.is_empty() || !output.is_empty() {
                        if example_keys.insert(key) {
                            examples.push(ex.clone());
                        }
                    }
                }
            }
        }
        
        if title.is_empty() {
            title = "Coding Problem".to_string();
        }
        
        let constraints_vec: Vec<serde_json::Value> = constraints
            .into_iter()
            .map(serde_json::Value::String)
            .collect();
        
        return serde_json::json!({
            "type": "DSA_PROBLEM",
            "title": title,
            "description": descriptions.join("\n"),
            "input_format": input_formats.join("\n"),
            "output_format": output_formats.join("\n"),
            "constraints": constraints_vec,
            "examples": examples,
            "confidence": confidence
        });
    }
    
    if has_system {
        let sources: Vec<&serde_json::Value> = parsed
            .iter()
            .filter(|v| v["type"].as_str() == Some("SYSTEM_DESIGN"))
            .collect();
        
        let mut questions = Vec::new();
        let mut requirements = HashSet::new();
        
        for v in sources {
            push_unique(&mut questions, v["question"].as_str().or_else(|| v["content"].as_str()));
            if let Some(arr) = v["requirements"].as_array() {
                for r in arr {
                    if let Some(s) = r.as_str() {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() {
                            requirements.insert(trimmed.to_string());
                        }
                    }
                }
            }
        }
        
        let req_vec: Vec<serde_json::Value> = requirements
            .into_iter()
            .map(serde_json::Value::String)
            .collect();
        
        return serde_json::json!({
            "type": "SYSTEM_DESIGN",
            "question": questions.join("\n"),
            "requirements": req_vec,
            "confidence": confidence
        });
    }
    
    if has_debug {
        let sources: Vec<&serde_json::Value> = parsed
            .iter()
            .filter(|v| v["type"].as_str() == Some("DEBUG_ERROR"))
            .collect();
        
        let mut error_category = String::new();
        let mut codes = Vec::new();
        let mut error_messages = Vec::new();
        let mut descriptions = Vec::new();
        
        for v in sources {
            if error_category.is_empty() {
                if let Some(cat) = v["error_category"].as_str() {
                    error_category = cat.to_string();
                }
            }
            push_unique(&mut codes, v["code"].as_str());
            push_unique(&mut error_messages, v["error_message"].as_str());
            push_unique(&mut descriptions, v["error_description"].as_str().or_else(|| v["description"].as_str()).or_else(|| v["content"].as_str()));
        }
        
        return serde_json::json!({
            "type": "DEBUG_ERROR",
            "error_category": error_category,
            "code": codes.join("\n"),
            "error_message": error_messages.join("\n"),
            "error_description": descriptions.join("\n"),
            "confidence": confidence
        });
    }
    
    let sources: Vec<&serde_json::Value> = parsed
        .iter()
        .filter(|v| v["type"].as_str() == Some("LOGICAL_PUZZLE"))
        .collect();
    
    if has_logical && !sources.is_empty() {
        let mut contents = Vec::new();
        for v in sources {
            push_unique(&mut contents, v["content"].as_str().or_else(|| v["question"].as_str()).or_else(|| v["description"].as_str()));
        }
        return serde_json::json!({
            "type": "LOGICAL_PUZZLE",
            "content": contents.join("\n"),
            "confidence": confidence
        });
    }
    
    let mut contents = Vec::new();
    for v in &parsed {
        push_unique(&mut contents, v["content"].as_str().or_else(|| v["question"].as_str()).or_else(|| v["description"].as_str()));
    }
    
    serde_json::json!({
        "type": "GENERAL",
        "content": contents.join("\n"),
        "confidence": confidence
    })
}

async fn process_debug_screenshot(image_data: String, groq: &GroqClient, tx: &broadcast::Sender<String>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    info!("Processing debug screenshot...");
    
    send_or_buffer(tx, serde_json::json!({
        "type": "transcription",
        "text": "Analyzing code error from screenshot..."
    }).to_string(), &buffer, &connected).await;
    
    // Get conversation history for context
    let history = conversation.read().await.clone();
    
    // Analyze the error screenshot with debugging context
    match groq.debug_code_error(&image_data, &history).await {
        Ok(fix_suggestion) => {
            info!("✓ Debug analysis complete");
            
            conversation.write().await.push(ConversationMessage {
                role: "user".to_string(),
                content: "[Screenshot: Code Error/Bug]".to_string(),
            });
            
            conversation.write().await.push(ConversationMessage {
                role: "assistant".to_string(),
                content: fix_suggestion.clone(),
            });
            
            send_or_buffer(tx, serde_json::json!({
                "type": "answer",
                "text": fix_suggestion
            }).to_string(), &buffer, &connected).await;
        }
        Err(e) => {
            error!("Debug analysis error: {}", e);
            send_or_buffer(tx, serde_json::json!({
                "type": "answer",
                "text": format!("Error analyzing code: {}", e)
            }).to_string(), &buffer, &connected).await;
        }
    }
}

async fn process_mcq_screenshot(image_data: String, groq: Arc<GroqClient>, tx: &broadcast::Sender<String>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    info!("Processing MCQ screenshot...");
    
    send_or_buffer(tx, serde_json::json!({
        "type": "transcription",
        "text": "Analyzing MCQ from screenshot..."
    }).to_string(), &buffer, &connected).await;
    
    // Create MCQ handler (no conversation history needed for MCQ)
    let mcq_handler = McqHandler::new(groq);
    
    // Step 1: Analyze MCQ screenshot using Scout model
    match mcq_handler.analyze_mcq_image(&image_data).await {
        Ok((mcq_data, _is_coding)) => {
            info!("✓ MCQ analysis complete from Scout");
            
            // Check if recapture is needed
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&mcq_data) {
                if parsed["needs_recapture"].as_bool().unwrap_or(false) {
                    let reason = parsed["reason"].as_str().unwrap_or("Low confidence. Please recapture.");
                    send_or_buffer(tx, serde_json::json!({
                        "type": "recapture_needed",
                        "message": reason
                    }).to_string(), &buffer, &connected).await;
                    return;
                }
            }
            
            // Step 2: Solve MCQ using OSS-120B with tools (no conversation history)
            match mcq_handler.solve_mcq(&mcq_data).await {
                Ok(solution) => {
                    info!("✓ MCQ solution complete from OSS-120B");
                    
                    // Don't add to conversation history as per requirement
                    send_or_buffer(tx, serde_json::json!({
                        "type": "answer",
                        "text": solution
                    }).to_string(), &buffer, &connected).await;
                }
                Err(e) => {
                    error!("MCQ solving error: {}", e);
                    send_or_buffer(tx, serde_json::json!({
                        "type": "answer",
                        "text": format!("Error solving MCQ: {}", e)
                    }).to_string(), &buffer, &connected).await;
                }
            }
        }
        Err(e) => {
            error!("MCQ analysis error: {}", e);
            send_or_buffer(tx, serde_json::json!({
                "type": "answer",
                "text": format!("Error analyzing MCQ: {}", e)
            }).to_string(), &buffer, &connected).await;
        }
    }
}

async fn process_screenshot(image_data_list: Vec<String>, groq: &GroqClient, tx: &broadcast::Sender<String>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    if image_data_list.is_empty() {
        return;
    }
    
    let capture_count = image_data_list.len();
    let status_text = if capture_count > 1 {
        format!("Screenshots ({}) captured - analyzing...", capture_count)
    } else {
        "Screenshot captured - analyzing...".to_string()
    };
    
    info!("Processing screenshot{}...", if capture_count > 1 { "s" } else { "" });
    
    send_or_buffer(tx, serde_json::json!({
        "type": "transcription",
        "text": status_text
    }).to_string(), &buffer, &connected).await;
    
    // Get conversation history for context
    let history = conversation.read().await.clone();
    
    let analysis_result: Result<String, String> = if capture_count == 1 {
        groq.analyze_image(&image_data_list[0], &history)
            .await
            .map(|(analysis, _)| analysis)
    } else {
        let mut analyses = Vec::new();
        for image in &image_data_list {
            match groq.analyze_image(image, &history).await {
                Ok((analysis, _)) => analyses.push(analysis),
                Err(e) => {
                    error!("Screenshot analysis error: {}", e);
                    send_or_buffer(tx, serde_json::json!({
                        "type": "answer",
                        "text": format!("Error analyzing screenshot: {}", e)
                    }).to_string(), &buffer, &connected).await;
                    return;
                }
            }
        }
        
        let cleaned_list: Vec<String> = analyses.iter().map(|a| clean_analysis_text(a)).collect();
        let merged = merge_cleaned_analyses(&cleaned_list);
        Ok(merged.to_string())
    };
    
    match analysis_result {
        Ok(analysis) => {
            info!("Raw analysis from Scout: {}", &analysis[..analysis.len().min(200)]);
            
            let cleaned_analysis = clean_analysis_text(&analysis);
            
            info!("Cleaned analysis (first 300 chars): {}", &cleaned_analysis[..cleaned_analysis.len().min(300)]);
            
            let mut is_coding = false;
            let mut problem_type_str = String::from("GENERAL");
            
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&cleaned_analysis) {
                info!("Successfully parsed JSON");
                if parsed["needs_recapture"].as_bool().unwrap_or(false) {
                    let reason = parsed["reason"].as_str().unwrap_or("Low confidence. Please recapture.");
                    send_or_buffer(tx, serde_json::json!({
                        "type": "recapture_needed",
                        "message": reason
                    }).to_string(), &buffer, &connected).await;
                    return;
                }
                
                problem_type_str = parsed["type"].as_str().unwrap_or("GENERAL").to_string();
                info!("Problem type from JSON: {}", problem_type_str);
                is_coding = problem_type_str == "DSA_PROBLEM";
            } else {
                info!("Failed to parse as JSON, checking if contains DSA_PROBLEM type");
                if cleaned_analysis.contains("\"type\": \"DSA_PROBLEM\"") || cleaned_analysis.contains("'type': 'DSA_PROBLEM'") {
                    info!("Found DSA_PROBLEM in text, treating as coding problem");
                    is_coding = true;
                    problem_type_str = String::from("DSA_PROBLEM");
                } else if cleaned_analysis.contains("\"type\": \"SYSTEM_DESIGN\"") {
                    problem_type_str = String::from("SYSTEM_DESIGN");
                }
            }
            
            info!("✓ Screenshot analyzed - Problem type: {}", problem_type_str);
            
            let final_answer = if is_coding {
                info!("Detected DSA_PROBLEM, formatting data for OSS-120B...");
                let problem_data = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&cleaned_analysis) {
                    let mut formatted = String::new();
                    
                    // Add predefined code structure if present
                    if let Some(predefined) = parsed["predefined_code"].as_str() {
                        if !predefined.is_empty() {
                            formatted.push_str(&format!("Predefined Code Structure (use this EXACT structure):\n{}\n\n", predefined));
                        }
                    }
                    
                    formatted.push_str(&format!("Problem: {}\n\n", parsed["title"].as_str().unwrap_or("Coding Problem")));
                    formatted.push_str(&format!("Description:\n{}\n\n", parsed["description"].as_str().unwrap_or("")));
                    formatted.push_str(&format!("Input Format: {}\n", parsed["input_format"].as_str().unwrap_or("")));
                    formatted.push_str(&format!("Output Format: {}\n\n", parsed["output_format"].as_str().unwrap_or("")));
                    if let Some(constraints) = parsed["constraints"].as_array() {
                        formatted.push_str("Constraints:\n");
                        for c in constraints {
                            if let Some(s) = c.as_str() {
                                formatted.push_str(&format!("- {}\n", s));
                            }
                        }
                        formatted.push_str("\n");
                    }
                    if let Some(examples) = parsed["examples"].as_array() {
                        formatted.push_str("Examples:\n");
                        for (i, ex) in examples.iter().enumerate() {
                            formatted.push_str(&format!("Example {}:\n", i + 1));
                            formatted.push_str(&format!("Input: {}\n", ex["input"].as_str().unwrap_or("")));
                            formatted.push_str(&format!("Output: {}\n", ex["output"].as_str().unwrap_or("")));
                            if let Some(exp) = ex["explanation"].as_str() {
                                formatted.push_str(&format!("Explanation: {}\n", exp));
                            }
                            formatted.push_str("\n");
                        }
                    }
                    info!("Formatted problem data: {} chars", formatted.len());
                    formatted
                } else {
                    info!("Failed to parse JSON, using raw analysis");
                    analysis.strip_prefix("CODING_PROBLEM:").unwrap_or(&analysis).trim().to_string()
                };
                
                conversation.write().await.push(ConversationMessage {
                    role: "user".to_string(),
                    content: format!("[Screenshot: Coding Problem]\n{}", problem_data),
                });
                
                info!("Calling solve_coding_problem with GPT-OSS-120B...");
                let history = conversation.read().await.clone();
                match groq.solve_coding_problem(&problem_data, &history).await {
                    Ok(solution) => {
                        info!("✓ Solution received: {} chars", solution.len());
                        conversation.write().await.push(ConversationMessage {
                            role: "assistant".to_string(),
                            content: solution.clone(),
                        });
                        solution
                    }
                    Err(e) => {
                        error!("Coding solution error: {}", e);
                        format!("Error solving problem: {}", e)
                    }
                }
            } else {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&cleaned_analysis) {
                    let problem_type = parsed["type"].as_str().unwrap_or("GENERAL");
                    
                    match problem_type {
                        "SYSTEM_DESIGN" => {
                            let question = parsed["question"].as_str().unwrap_or("System design question");
                            let requirements = if let Some(reqs) = parsed["requirements"].as_array() {
                                reqs.iter().filter_map(|r| r.as_str()).collect::<Vec<_>>().join("\n- ")
                            } else {
                                String::new()
                            };
                            
                            let formatted = format!("System Design Question:\n{}\n\nRequirements:\n- {}\n", question, requirements);
                            
                            conversation.write().await.push(ConversationMessage {
                                role: "user".to_string(),
                                content: format!("[Screenshot: System Design]\n{}", formatted),
                            });
                            
                            info!("Answering system design question with GPT-OSS-120B...");
                            let history = conversation.read().await.clone();
                            match groq.solve_coding_problem(&formatted, &history).await {
                                Ok(answer) => {
                                    conversation.write().await.push(ConversationMessage {
                                        role: "assistant".to_string(),
                                        content: answer.clone(),
                                    });
                                    answer
                                }
                                Err(e) => format!("Error: {}", e)
                            }
                        }
                        "DEBUG_ERROR" => {
                            info!("🐛 Detected DEBUG_ERROR, sending to OSS-120B for debugging...");
                            let error_content = parsed["error_description"].as_str()
                                .or_else(|| parsed["content"].as_str())
                                .or_else(|| parsed["description"].as_str())
                                .unwrap_or("Code debugging request");
                            
                            let code_content = parsed["code"].as_str().unwrap_or("");
                            let error_message = parsed["error_message"].as_str().unwrap_or("");
                            
                            let formatted = if !code_content.is_empty() && !error_message.is_empty() {
                                format!("Debug this code error:\n\nCode:\n```\n{}\n```\n\nError:\n{}\n\nDescription: {}", 
                                    code_content, error_message, error_content)
                            } else {
                                format!("Debug Error Analysis:\n{}", error_content)
                            };
                            
                            conversation.write().await.push(ConversationMessage {
                                role: "user".to_string(),
                                content: format!("[Screenshot: Debug Error]\n{}", formatted),
                            });
                            
                            info!("🐛 Sending debug request to GPT-OSS-120B...");
                            let history = conversation.read().await.clone();
                            match groq.solve_coding_problem(&formatted, &history).await {
                                Ok(answer) => {
                                    info!("✓ Debug solution received: {} chars", answer.len());
                                    conversation.write().await.push(ConversationMessage {
                                        role: "assistant".to_string(),
                                        content: answer.clone(),
                                    });
                                    answer
                                }
                                Err(e) => {
                                    error!("Debug solution error: {}", e);
                                    format!("Error debugging code: {}", e)
                                }
                            }
                        }
                        "LOGICAL_PUZZLE" | "GENERAL" => {
                            let content = parsed["content"].as_str()
                                .or_else(|| parsed["question"].as_str())
                                .or_else(|| parsed["description"].as_str())
                                .unwrap_or("General question");
                            
                            conversation.write().await.push(ConversationMessage {
                                role: "user".to_string(),
                                content: format!("[Screenshot: {}]\n{}", problem_type, content),
                            });
                            
                            info!("Answering {} with GPT-OSS-120B...", problem_type);
                            let history = conversation.read().await.clone();
                            match groq.solve_coding_problem(content, &history).await {
                                Ok(answer) => {
                                    conversation.write().await.push(ConversationMessage {
                                        role: "assistant".to_string(),
                                        content: answer.clone(),
                                    });
                                    answer
                                }
                                Err(e) => format!("Error: {}", e)
                            }
                        }
                        _ => {
                            conversation.write().await.push(ConversationMessage {
                                role: "user".to_string(),
                                content: "[Screenshot captured]".to_string(),
                            });
                            conversation.write().await.push(ConversationMessage {
                                role: "assistant".to_string(),
                                content: cleaned_analysis.to_string(),
                            });
                            cleaned_analysis.to_string()
                        }
                    }
                } else {
                    conversation.write().await.push(ConversationMessage {
                        role: "user".to_string(),
                        content: "[Screenshot captured]".to_string(),
                    });
                    conversation.write().await.push(ConversationMessage {
                        role: "assistant".to_string(),
                        content: analysis.clone(),
                    });
                    analysis
                }
            };
            
            send_or_buffer(tx, serde_json::json!({
                "type": "answer",
                "text": final_answer
            }).to_string(), &buffer, &connected).await;
        }
        Err(e) => {
            error!("Screenshot analysis error: {}", e);
            send_or_buffer(tx, serde_json::json!({
                "type": "answer",
                "text": format!("Error analyzing screenshot: {}", e)
            }).to_string(), &buffer, &connected).await;
        }
    }
}


