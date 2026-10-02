mod modules;

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use std::collections::VecDeque;
use tokio::sync::{broadcast, RwLock};
use tower_http::services::ServeDir;
use tracing::{info, error};

use modules::{
    audio::AudioCapture,
    audio_processor::AudioProcessor,
    mic_capture::MicCapture,
    mic_processor::MicProcessor,
    screen::ScreenCapture,
    groq::{GroqClient, ConversationMessage, NamedKey},
    code::CodeManager,
    config::Config,
    search::SearchHotkey,
    debug::DebugHotkey,
    mcq::McqHandler,
    mcq_hotkey::McqHotkey,
};

type ConversationHistory = Arc<RwLock<Vec<ConversationMessage>>>;
type MessageBuffer = Arc<RwLock<VecDeque<String>>>;
type CandidateContext = Arc<RwLock<VecDeque<String>>>;

#[derive(Clone)]
struct AppState {
    tx: broadcast::Sender<String>,
    groq: Arc<GroqClient>,
    code_manager: Arc<CodeManager>,
    conversation: ConversationHistory,
    candidate_context: CandidateContext,
    message_buffer: MessageBuffer,
    client_connections: Arc<AtomicUsize>,
    search_hotkey: Arc<tokio::sync::Mutex<SearchHotkey>>,
    multi_capture_cancel: Arc<AtomicBool>,
    ui_chunks: Arc<RwLock<Vec<String>>>,
}

const SILENCE_TIMEOUT: Duration = Duration::from_secs(2);
const MIN_AUDIO_DURATION: Duration = Duration::from_millis(1000);
const MIN_AVERAGE_ENERGY: f32 = 0.025;
const ENERGY_DROP_THRESHOLD: f32 = 0.55;
const ENERGY_DROP_TIMEOUT: Duration = Duration::from_millis(800);
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
        if buf.len() > 100 { buf.pop_front(); }
    }
}

/// Trim conversation history to max 30 messages (15 Q&A pairs)
async fn trim_conversation(conversation: &ConversationHistory) {
    let mut conv = conversation.write().await;
    if conv.len() > 30 {
        let drain_count = conv.len() - 30;
        conv.drain(..drain_count);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Starting Nvidia...");

    let config = Config::load().expect("Failed to load config");
    let (tx, _rx) = broadcast::channel(100);

    // Build Gemini named keys from config
    let gemini_named_keys: Vec<NamedKey> = config.gemini_keys.iter()
        .filter(|k| !k.key.is_empty())
        .map(|k| NamedKey { name: k.name.clone(), key: k.key.clone() })
        .collect();

    info!("[Gemini] {} key(s) configured", gemini_named_keys.len());

    let mut groq_client = GroqClient::new_with_gemini(config.groq_api_key.clone(), gemini_named_keys);
    groq_client.set_broadcast(tx.clone());
    let groq = Arc::new(groq_client);
    let code_manager = Arc::new(CodeManager::new(config.project_path.clone()));

    let search_hotkey = Arc::new(tokio::sync::Mutex::new(SearchHotkey::new()));
    let multi_capture_cancel = Arc::new(AtomicBool::new(false));
    tokio::spawn(start_search_hotkey_listener(tx.clone(), search_hotkey.clone()));

    let mic_recording = Arc::new(AtomicBool::new(false));

    let state = AppState {
        tx: tx.clone(),
        groq: groq.clone(),
        code_manager: code_manager.clone(),
        conversation: Arc::new(RwLock::new(Vec::new())),
        candidate_context: Arc::new(RwLock::new(VecDeque::new())),
        message_buffer: Arc::new(RwLock::new(VecDeque::new())),
        client_connections: Arc::new(AtomicUsize::new(0)),
        search_hotkey,
        multi_capture_cancel: multi_capture_cancel.clone(),
        ui_chunks: Arc::new(RwLock::new(Vec::new())),
    };

    // Start audio capture (interviewer system audio)
    tokio::spawn(start_audio_capture(
        tx.clone(), groq.clone(),
        state.conversation.clone(), state.candidate_context.clone(),
        state.message_buffer.clone(), state.client_connections.clone(),
    ));

    // Start mic capture (candidate answers)
    tokio::spawn(start_mic_capture(
        groq.clone(), state.candidate_context.clone(), tx.clone(),
        state.message_buffer.clone(), state.client_connections.clone(), mic_recording.clone(),
    ));

    // Mic recording hotkey (Ctrl+Left toggle)
    tokio::spawn(start_mic_recording_hotkey(tx.clone(), mic_recording.clone()));

    // Screen capture hotkey (Ctrl+Shift+S) → Gemini vision
    tokio::spawn(start_screen_capture_hotkey(
        tx.clone(), groq.clone(), config.hotkey.clone(),
        state.conversation.clone(), state.candidate_context.clone(),
        state.message_buffer.clone(), state.client_connections.clone(),
        multi_capture_cancel.clone(), state.ui_chunks.clone(),
    ));

    // Debug hotkey (Ctrl+D) → Gemini vision debug
    tokio::spawn(start_debug_hotkey_listener(
        tx.clone(), groq.clone(),
        state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone(),
    ));

    // MCQ hotkey (Ctrl+Alt+Q) → Gemini vision MCQ
    tokio::spawn(start_mcq_hotkey_listener(
        tx.clone(), groq.clone(),
        state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone(),
    ));

    // Gemini capture hotkey (Ctrl+Alt+A) → Gemini vision solve
    tokio::spawn(start_gemini_capture_hotkey(
        tx.clone(), groq.clone(),
        state.conversation.clone(), state.candidate_context.clone(),
        state.message_buffer.clone(), state.client_connections.clone(),
        state.ui_chunks.clone(),
    ));

    // Gemini Debug hotkey (Ctrl+Alt+F) → Gemini vision debug
    tokio::spawn(start_gemini_debug_hotkey(
        tx.clone(), groq.clone(),
        state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone(),
    ));

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .route("/api/microphones", get(list_microphones))
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

async fn list_microphones() -> Json<serde_json::Value> {
    let devices = MicCapture::list_devices();
    Json(serde_json::json!({ "devices": devices }))
}

async fn pip_toggle_handler(State(state): State<AppState>) -> impl IntoResponse {
    info!("[PiP] HTTP toggle request received");
    let _ = state.tx.send(serde_json::json!({ "type": "toggle_pip_window" }).to_string());
    let _ = state.tx.send(serde_json::json!({ "type": "get_pip_state" }).to_string());
    axum::response::Json(serde_json::json!({ "status": "ok", "message": "PiP toggle broadcasted" }))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

struct ConnectionGuard { count: Arc<AtomicUsize> }
impl ConnectionGuard {
    fn new(count: Arc<AtomicUsize>) -> Self { count.fetch_add(1, Ordering::SeqCst); Self { count } }
}
impl Drop for ConnectionGuard {
    fn drop(&mut self) { self.count.fetch_sub(1, Ordering::SeqCst); }
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    let _conn_guard = ConnectionGuard::new(state.client_connections.clone());

    // Drain buffered messages to new client
    {
        let mut buffer = state.message_buffer.write().await;
        while let Some(msg) = buffer.pop_front() {
            if socket.send(axum::extract::ws::Message::Text(msg)).await.is_err() { return; }
        }
    }

    loop {
        tokio::select! {
            msg = rx.recv() => {
                if let Ok(msg) = msg {
                    if socket.send(axum::extract::ws::Message::Text(msg)).await.is_err() { break; }
                }
            }
            msg = socket.recv() => {
                if let Some(Ok(axum::extract::ws::Message::Text(text))) = msg {
                    if text == "capture_screen" {
                        // Capture with any buffered chunks
                        let mut chunks = state.ui_chunks.write().await;
                        if !chunks.is_empty() {
                            match ScreenCapture::capture_now() {
                                Ok(image_data) => {
                                    chunks.push(image_data);
                                    let all_images = std::mem::take(&mut *chunks);
                                    drop(chunks);
                                    let _ = state.tx.send(serde_json::json!({
                                        "type": "multi_capture_update", "count": 0
                                    }).to_string());
                                    let groq_c = state.groq.clone();
                                    let tx_c = state.tx.clone();
                                    let conv_c = state.conversation.clone();
                                    let ctx_c = state.candidate_context.clone();
                                    let buf_c = state.message_buffer.clone();
                                    let conn_c = state.client_connections.clone();
                                    tokio::spawn(async move {
                                        process_screenshot(all_images, &groq_c, &tx_c, conv_c, ctx_c, buf_c, conn_c).await;
                                    });
                                }
                                Err(e) => { drop(chunks); error!("Screen capture failed: {}", e); }
                            }
                        } else {
                            drop(chunks);
                            tokio::spawn(handle_screen_capture(
                                state.tx.clone(), state.groq.clone(),
                                state.conversation.clone(), state.candidate_context.clone(),
                                state.message_buffer.clone(), state.client_connections.clone(),
                            ));
                        }
                    } else if text == "capture_chunk" {
                        match ScreenCapture::capture_now() {
                            Ok(image_data) => {
                                let mut chunks = state.ui_chunks.write().await;
                                chunks.push(image_data);
                                let count = chunks.len();
                                drop(chunks);
                                info!("[UI] Chunk captured via + button ({} total)", count);
                                let _ = state.tx.send(serde_json::json!({
                                    "type": "multi_capture_update", "count": count
                                }).to_string());
                            }
                            Err(e) => error!("Chunk capture failed: {}", e),
                        }
                    } else if text == "capture_mcq" {
                        tokio::spawn(handle_mcq_capture(
                            state.tx.clone(), state.groq.clone(),
                            state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone(),
                        ));
                    } else if text == "capture_ring" {
                        // Gemini capture via UI button
                        let groq_c = state.groq.clone();
                        let tx_c = state.tx.clone();
                        let conv_c = state.conversation.clone();
                        let ctx_c = state.candidate_context.clone();
                        let buf_c = state.message_buffer.clone();
                        let conn_c = state.client_connections.clone();
                        tokio::spawn(async move {
                            handle_gemini_capture(tx_c, groq_c, conv_c, ctx_c, buf_c, conn_c).await;
                        });
                    } else if text == "debug_code" {
                        tokio::spawn(handle_debug_code(
                            state.tx.clone(), state.groq.clone(),
                            state.conversation.clone(), state.message_buffer.clone(), state.client_connections.clone(),
                        ));
                    } else if text == "debug_ring" {
                        let tx_c = state.tx.clone();
                        let groq_c = state.groq.clone();
                        let conv_c = state.conversation.clone();
                        let buf_c = state.message_buffer.clone();
                        let conn_c = state.client_connections.clone();
                        tokio::spawn(async move {
                            handle_debug_code_gemini(tx_c, groq_c, conv_c, buf_c, conn_c).await;
                        });
                    } else if text == "search_closed" {
                        let hotkey = state.search_hotkey.lock().await;
                        hotkey.deactivate();
                    } else if text == "multi_capture_cancel" {
                        state.multi_capture_cancel.store(true, Ordering::SeqCst);
                    } else if text.starts_with('{') {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");

                            match msg_type {
                                "open_pip_window" => { let _ = state.tx.send(serde_json::json!({ "type": "open_pip_window" }).to_string()); }
                                "close_pip_window" => { let _ = state.tx.send(serde_json::json!({ "type": "close_pip_window" }).to_string()); }
                                "set_pip_opacity" => {
                                    let opacity = json.get("opacity").and_then(|v| v.as_f64()).unwrap_or(1.0);
                                    let _ = state.tx.send(serde_json::json!({ "type": "set_pip_opacity", "opacity": opacity }).to_string());
                                }
                                "set_pip_resizable" => {
                                    let resizable = json.get("resizable").and_then(|v| v.as_bool()).unwrap_or(true);
                                    let _ = state.tx.send(serde_json::json!({ "type": "set_pip_resizable", "resizable": resizable }).to_string());
                                }
                                "theme_change" => {
                                    let theme = json.get("theme").and_then(|v| v.as_str()).unwrap_or("dark").to_string();
                                    let _ = state.tx.send(serde_json::json!({ "type": "theme_change", "theme": theme }).to_string());
                                    info!("[Theme] Broadcasted theme change: {}", theme);
                                }
                                "solve_problem" => {
                                    let question = json.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    if !question.is_empty() {
                                        let tx_clone = state.tx.clone();
                                        let groq = state.groq.clone();
                                        let conversation = state.conversation.clone();
                                        let candidate_ctx = state.candidate_context.clone();
                                        let buffer = state.message_buffer.clone();
                                        let connected = state.client_connections.clone();
                                        tokio::spawn(async move {
                                            let history = conversation.read().await.clone();
                                            let ctx: Vec<String> = candidate_ctx.read().await.iter().cloned().collect();
                                            match groq.solve_coding_problem(&question, &history, &ctx).await {
                                                Ok(solution) => {
                                                    conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: question.clone() });
                                                    conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: solution.clone() });
                                                    send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": solution }).to_string(), &buffer, &connected).await;
                                                }
                                                Err(e) => {
                                                    send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": format!("Error: {}", e) }).to_string(), &buffer, &connected).await;
                                                }
                                            }
                                        });
                                    }
                                }
                                "manual_question" => {
                                    let question = json.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    info!("🔍 [SEARCH LOG] Received manual_question: '{}'", question);
                                    if !question.is_empty() {
                                        let tx_clone = state.tx.clone();
                                        let groq = state.groq.clone();
                                        let conversation = state.conversation.clone();
                                        let candidate_ctx = state.candidate_context.clone();
                                        let buffer = state.message_buffer.clone();
                                        let connected = state.client_connections.clone();
                                        tokio::spawn(async move {
                                            send_or_buffer(&tx_clone, serde_json::json!({ "type": "transcription", "text": question }).to_string(), &buffer, &connected).await;
                                            let history = conversation.read().await.clone();
                                            let candidate_context_vec: Vec<String> = candidate_ctx.read().await.iter().cloned().collect();

                                            let mut attempt = 1;
                                            let max_attempts = 2;
                                            loop {
                                                let result = tokio::time::timeout(
                                                    Duration::from_secs(15),
                                                    groq.chat_with_context(&question, &history, &candidate_context_vec)
                                                ).await;
                                                match result {
                                                    Ok(Ok(answer)) => {
                                                        conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: question.clone() });
                                                        conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: answer.clone() });
                                                        send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": answer }).to_string(), &buffer, &connected).await;
                                                        break;
                                                    }
                                                    Ok(Err(e)) => {
                                                        error!("🔍 [SEARCH LOG] AI request failed: {}", e);
                                                        if attempt < max_attempts { attempt += 1; continue; }
                                                        send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": format!("Error: {}", e) }).to_string(), &buffer, &connected).await;
                                                        break;
                                                    }
                                                    Err(_) => {
                                                        error!("🔍 [SEARCH LOG] AI request timed out");
                                                        if attempt < max_attempts { attempt += 1; continue; }
                                                        send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": "Request timed out. Please try again." }).to_string(), &buffer, &connected).await;
                                                        break;
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                                "mic_audio" => {
                                    let audio_base64 = json.get("audio").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    if !audio_base64.is_empty() {
                                        let tx_clone = state.tx.clone();
                                        let groq = state.groq.clone();
                                        let conversation = state.conversation.clone();
                                        let buffer = state.message_buffer.clone();
                                        let connected = state.client_connections.clone();
                                        tokio::spawn(async move {
                                            send_or_buffer(&tx_clone, serde_json::json!({ "type": "transcription", "text": "Processing microphone audio..." }).to_string(), &buffer, &connected).await;
                                            match base64::decode(&audio_base64) {
                                                Ok(audio_data) => {
                                                    match groq.transcribe(&audio_data).await {
                                                        Ok(transcription) => {
                                                            let text = transcription.trim();
                                                            if !text.is_empty() {
                                                                send_or_buffer(&tx_clone, serde_json::json!({ "type": "update_transcription", "text": text }).to_string(), &buffer, &connected).await;
                                                                let history = conversation.read().await.clone();
                                                                match groq.chat_with_history(text, &history).await {
                                                                    Ok(answer) => {
                                                                        conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: text.to_string() });
                                                                        conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: answer.clone() });
                                                                        send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": answer }).to_string(), &buffer, &connected).await;
                                                                    }
                                                                    Err(e) => { send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": format!("Error: {}", e) }).to_string(), &buffer, &connected).await; }
                                                                }
                                                            }
                                                        }
                                                        Err(e) => { send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": format!("Transcription error: {}", e) }).to_string(), &buffer, &connected).await; }
                                                    }
                                                }
                                                Err(e) => { send_or_buffer(&tx_clone, serde_json::json!({ "type": "answer", "text": format!("Audio decode error: {}", e) }).to_string(), &buffer, &connected).await; }
                                            }
                                        });
                                    }
                                }
                                _ => {}
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

async fn get_file_content(State(state): State<AppState>, Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let path = payload["path"].as_str().unwrap_or("");
    let content = state.code_manager.get_file_content(path);
    Json(serde_json::json!({ "content": content }))
}

async fn query_code(State(state): State<AppState>, Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let query = payload["query"].as_str().unwrap_or("");
    let context = state.code_manager.get_relevant_context(query);
    let response = state.groq.chat(&format!("Context:\n{}\n\nQuestion: {}", context, query))
        .await.unwrap_or_else(|_| "Error processing query".to_string());
    Json(serde_json::json!({ "response": response }))
}

// ── Audio capture tasks ────────────────────────────────────────────────────────

async fn start_audio_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, candidate_context: CandidateContext, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    let (_audio_capture, audio_receiver) = AudioCapture::new();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut processor = AudioProcessor::new(audio_receiver, groq, tx, conversation, candidate_context, buffer, connected);
            processor.run().await;
        });
    });
}

async fn start_mic_capture(groq: Arc<GroqClient>, candidate_context: CandidateContext, tx: broadcast::Sender<String>, buffer: MessageBuffer, connected: Arc<AtomicUsize>, recording_flag: Arc<AtomicBool>) {
    let (_mic_capture, mic_receiver) = MicCapture::new();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut processor = MicProcessor::new(mic_receiver, groq, tx, candidate_context, buffer, connected, recording_flag);
            processor.run().await;
        });
    });
}

/// Mic recording hotkey: Ctrl+Left toggle
async fn start_mic_recording_hotkey(tx: broadcast::Sender<String>, recording_flag: Arc<AtomicBool>) {
    use winapi::um::winuser::{GetAsyncKeyState, VK_CONTROL, VK_LEFT};
    let mut was_pressed = false;
    loop {
        tokio::time::sleep(Duration::from_millis(80)).await;
        let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL) } < 0;
        let left = unsafe { GetAsyncKeyState(VK_LEFT) } < 0;
        let is_pressed = ctrl && left;
        if is_pressed && !was_pressed {
            let now_recording = !recording_flag.load(Ordering::SeqCst);
            recording_flag.store(now_recording, Ordering::SeqCst);
            let _ = tx.send(serde_json::json!({ "type": "mic_recording", "recording": now_recording }).to_string());
            info!("[Mic Hotkey] Ctrl+Left — Recording {}", if now_recording { "STARTED" } else { "STOPPED" });
        }
        was_pressed = is_pressed;
    }
}

// ── Screenshot handlers (all use Gemini vision now) ───────────────────────────

async fn handle_screen_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, candidate_context: CandidateContext, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => process_screenshot(vec![image_data], &groq, &tx, conversation, candidate_context, buffer, connected).await,
        Err(e) => error!("Screen capture failed: {}", e),
    }
}

/// Main screenshot processor — Gemini vision, no Scout intermediate step
async fn process_screenshot(
    image_data_list: Vec<String>,
    groq: &GroqClient,
    tx: &broadcast::Sender<String>,
    conversation: ConversationHistory,
    candidate_context: CandidateContext,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
) {
    if image_data_list.is_empty() { return; }
    let capture_count = image_data_list.len();

    info!("Processing {} screenshot(s) with Gemini vision...", capture_count);
    send_or_buffer(tx, serde_json::json!({
        "type": "transcription",
        "text": format!("📸 {} screenshot(s) captured — solving with Gemini...", capture_count)
    }).to_string(), &buffer, &connected).await;

    let history = conversation.read().await.clone();
    let ctx: Vec<String> = candidate_context.read().await.iter().cloned().collect();

    match groq.solve_with_gemini_vision(&image_data_list, &history, &ctx).await {
        Ok(solution) => {
            info!("✓ Gemini vision solution: {} chars", solution.len());
            conversation.write().await.push(ConversationMessage {
                role: "user".to_string(),
                content: format!("[Screenshot: {} image(s)]", capture_count),
            });
            conversation.write().await.push(ConversationMessage {
                role: "assistant".to_string(),
                content: solution.clone(),
            });
            trim_conversation(&conversation).await;
            send_or_buffer(tx, serde_json::json!({ "type": "answer", "text": solution }).to_string(), &buffer, &connected).await;
        }
        Err(e) => {
            error!("Screenshot solve error: {}", e);
            send_or_buffer(tx, serde_json::json!({ "type": "answer", "text": format!("Error: {}", e) }).to_string(), &buffer, &connected).await;
        }
    }
}

/// Debug screenshot processor — Gemini vision debug
async fn process_debug_screenshot(
    image_data: String,
    groq: &GroqClient,
    tx: &broadcast::Sender<String>,
    conversation: ConversationHistory,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
) {
    info!("Processing debug screenshot with Gemini vision...");
    send_or_buffer(tx, serde_json::json!({
        "type": "transcription",
        "text": "🐛 Analyzing code error with Gemini..."
    }).to_string(), &buffer, &connected).await;

    let history = conversation.read().await.clone();

    match groq.debug_with_gemini(&image_data, &history).await {
        Ok(fix_suggestion) => {
            info!("✓ Debug analysis complete: {} chars", fix_suggestion.len());
            conversation.write().await.push(ConversationMessage {
                role: "user".to_string(),
                content: "[Screenshot: Code Error/Bug]".to_string(),
            });
            conversation.write().await.push(ConversationMessage {
                role: "assistant".to_string(),
                content: fix_suggestion.clone(),
            });
            trim_conversation(&conversation).await;
            send_or_buffer(tx, serde_json::json!({ "type": "answer", "text": fix_suggestion }).to_string(), &buffer, &connected).await;
        }
        Err(e) => {
            error!("Debug analysis error: {}", e);
            send_or_buffer(tx, serde_json::json!({ "type": "answer", "text": format!("Error analyzing code: {}", e) }).to_string(), &buffer, &connected).await;
        }
    }
}

/// MCQ screenshot processor — Gemini vision MCQ
async fn process_mcq_screenshot(
    image_data: String,
    groq: Arc<GroqClient>,
    tx: &broadcast::Sender<String>,
    _conversation: ConversationHistory,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
) {
    info!("Processing MCQ screenshot with Gemini vision...");
    send_or_buffer(tx, serde_json::json!({
        "type": "transcription",
        "text": "📝 Analyzing MCQ with Gemini..."
    }).to_string(), &buffer, &connected).await;

    let mcq_handler = McqHandler::new(groq);
    match mcq_handler.solve_mcq_from_image(&image_data).await {
        Ok(solution) => {
            info!("✓ MCQ solution complete: {} chars", solution.len());
            // MCQ does NOT add to conversation history (standalone answer)
            send_or_buffer(tx, serde_json::json!({ "type": "answer", "text": solution }).to_string(), &buffer, &connected).await;
        }
        Err(e) => {
            error!("MCQ solving error: {}", e);
            send_or_buffer(tx, serde_json::json!({ "type": "answer", "text": format!("Error solving MCQ: {}", e) }).to_string(), &buffer, &connected).await;
        }
    }
}

async fn handle_debug_code(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => process_debug_screenshot(image_data, &groq, &tx, conversation, buffer, connected).await,
        Err(e) => error!("Debug capture failed: {}", e),
    }
}

async fn handle_debug_code_gemini(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    // Same as handle_debug_code — both use Gemini vision now
    match ScreenCapture::capture_now() {
        Ok(image_data) => {
            info!("[Gemini Debug] Screenshot captured...");
            send_or_buffer(&tx, serde_json::json!({
                "type": "transcription",
                "text": "💎 Gemini Debug: Analyzing error..."
            }).to_string(), &buffer, &connected).await;
            let history = conversation.read().await.clone();
            match groq.debug_with_gemini(&image_data, &history).await {
                Ok(fix) => {
                    info!("[Gemini Debug] ✓ Fix received: {} chars", fix.len());
                    conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: "[Gemini Debug: Code Error]".to_string() });
                    conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: fix.clone() });
                    trim_conversation(&conversation).await;
                    send_or_buffer(&tx, serde_json::json!({ "type": "answer", "text": fix }).to_string(), &buffer, &connected).await;
                }
                Err(e) => {
                    error!("[Gemini Debug] Error: {}", e);
                    send_or_buffer(&tx, serde_json::json!({ "type": "answer", "text": format!("Gemini Debug Error: {}", e) }).to_string(), &buffer, &connected).await;
                }
            }
        }
        Err(e) => error!("[Gemini Debug] Capture failed: {}", e),
    }
}

async fn handle_mcq_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => process_mcq_screenshot(image_data, groq, &tx, conversation, buffer, connected).await,
        Err(e) => error!("MCQ capture failed: {}", e),
    }
}

/// Gemini capture (Ctrl+Alt+A) — single or multi-image, direct Gemini vision solve
async fn handle_gemini_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, candidate_context: CandidateContext, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => {
            let images = vec![image_data];
            let image_count = images.len();
            info!("[Gemini] Captured {} image(s), solving...", image_count);
            send_or_buffer(&tx, serde_json::json!({
                "type": "transcription",
                "text": format!("💎 Gemini: Solving from {} screenshot(s)...", image_count)
            }).to_string(), &buffer, &connected).await;
            let history = conversation.read().await.clone();
            let ctx: Vec<String> = candidate_context.read().await.iter().cloned().collect();
            match groq.solve_with_gemini_vision(&images, &history, &ctx).await {
                Ok(solution) => {
                    info!("[Gemini] ✓ Solution: {} chars", solution.len());
                    conversation.write().await.push(ConversationMessage { role: "user".to_string(), content: format!("[Gemini: {} screenshot(s)]", image_count) });
                    conversation.write().await.push(ConversationMessage { role: "assistant".to_string(), content: solution.clone() });
                    trim_conversation(&conversation).await;
                    send_or_buffer(&tx, serde_json::json!({ "type": "answer", "text": solution }).to_string(), &buffer, &connected).await;
                }
                Err(e) => {
                    error!("[Gemini] Error: {}", e);
                    send_or_buffer(&tx, serde_json::json!({ "type": "answer", "text": format!("Gemini Error: {}", e) }).to_string(), &buffer, &connected).await;
                }
            }
        }
        Err(e) => error!("[Gemini] Screen capture failed: {}", e),
    }
}

async fn handle_gemini_capture_multi(
    tx: broadcast::Sender<String>,
    groq: Arc<GroqClient>,
    conversation: ConversationHistory,
    candidate_context: CandidateContext,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    images: Vec<String>,
) {
    let image_count = images.len();
    info!("[Gemini] Processing {} image(s)...", image_count);
    send_or_buffer(&tx, serde_json::json!({
        "type": "transcription",
        "text": format!("💎 Gemini: Solving from {} screenshot(s)...", image_count)
    }).to_string(), &buffer, &connected).await;

    let history = conversation.read().await.clone();
    let ctx: Vec<String> = candidate_context.read().await.iter().cloned().collect();

    match groq.solve_with_gemini_vision(&images, &history, &ctx).await {
        Ok(solution) => {
            info!("[Gemini] ✓ Solution: {} chars", solution.len());
            conversation.write().await.push(ConversationMessage {
                role: "user".to_string(),
                content: format!("[Gemini: {} screenshot(s)]", image_count),
            });
            conversation.write().await.push(ConversationMessage {
                role: "assistant".to_string(),
                content: solution.clone(),
            });
            trim_conversation(&conversation).await;
            send_or_buffer(&tx, serde_json::json!({ "type": "answer", "text": solution }).to_string(), &buffer, &connected).await;
        }
        Err(e) => {
            error!("[Gemini] Error: {}", e);
            send_or_buffer(&tx, serde_json::json!({ "type": "answer", "text": format!("Gemini Error: {}", e) }).to_string(), &buffer, &connected).await;
        }
    }
}

// ── Hotkey listeners ──────────────────────────────────────────────────────────

async fn start_screen_capture_hotkey(
    tx: broadcast::Sender<String>,
    groq: Arc<GroqClient>,
    hotkey: String,
    conversation: ConversationHistory,
    candidate_context: CandidateContext,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    multi_capture_cancel: Arc<AtomicBool>,
    ui_chunks: Arc<RwLock<Vec<String>>>,
) {
    let mut screen_capture = ScreenCapture::new(hotkey);
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;

        if multi_capture_cancel.swap(false, Ordering::SeqCst) {
            let mut chunks = ui_chunks.write().await;
            if !chunks.is_empty() { chunks.clear(); }
            drop(chunks);
            let _ = tx.send(serde_json::json!({ "type": "multi_capture_update", "count": 0 }).to_string());
            info!("Multi-capture buffer cleared.");
        }

        if let Some(image_data) = screen_capture.check_multi_capture() {
            let mut chunks = ui_chunks.write().await;
            chunks.push(image_data);
            let count = chunks.len();
            drop(chunks);
            let _ = tx.send(serde_json::json!({ "type": "multi_capture_update", "count": count }).to_string());
            info!("Multi-capture chunk buffered ({}).", count);
        }

        if screen_capture.check_cancel() {
            let mut chunks = ui_chunks.write().await;
            if !chunks.is_empty() {
                chunks.clear();
                drop(chunks);
                let _ = tx.send(serde_json::json!({ "type": "multi_capture_update", "count": 0 }).to_string());
            }
            info!("Multi-capture buffer cleared.");
        }

        if let Some(image_data) = screen_capture.check_capture() {
            let mut chunks = ui_chunks.write().await;
            if !chunks.is_empty() {
                chunks.push(image_data);
                let images = std::mem::take(&mut *chunks);
                drop(chunks);
                let _ = tx.send(serde_json::json!({ "type": "multi_capture_update", "count": 0 }).to_string());
                let groq_c = groq.clone();
                let tx_c = tx.clone();
                let conv_c = conversation.clone();
                let ctx_c = candidate_context.clone();
                let buf_c = buffer.clone();
                let conn_c = connected.clone();
                tokio::spawn(async move {
                    process_screenshot(images, &groq_c, &tx_c, conv_c, ctx_c, buf_c, conn_c).await;
                });
            } else {
                drop(chunks);
                let groq_c = groq.clone();
                let tx_c = tx.clone();
                let conv_c = conversation.clone();
                let ctx_c = candidate_context.clone();
                let buf_c = buffer.clone();
                let conn_c = connected.clone();
                tokio::spawn(async move {
                    process_screenshot(vec![image_data], &groq_c, &tx_c, conv_c, ctx_c, buf_c, conn_c).await;
                });
            }
        }
    }
}

async fn start_search_hotkey_listener(tx: broadcast::Sender<String>, search_hotkey: Arc<tokio::sync::Mutex<SearchHotkey>>) {
    loop {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut hotkey = search_hotkey.lock().await;
        if hotkey.check_triggered() {
            let _ = tx.send(serde_json::json!({ "type": "toggle_search" }).to_string());
        }
        if hotkey.check_backspace() {
            let _ = tx.send(serde_json::json!({ "type": "search_backspace" }).to_string());
        }
        if hotkey.check_enter() {
            let _ = tx.send(serde_json::json!({ "type": "search_enter" }).to_string());
        }
        let keystrokes = hotkey.get_keystrokes();
        if !keystrokes.is_empty() {
            let _ = tx.send(serde_json::json!({ "type": "search_keystroke", "text": keystrokes }).to_string());
        }
    }
}

async fn start_debug_hotkey_listener(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<AtomicUsize>) {
    let mut debug_hotkey = DebugHotkey::new();
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if debug_hotkey.check_triggered() {
            info!("Debug hotkey triggered — capturing screen for Gemini debug");
            match ScreenCapture::capture_now() {
                Ok(image_data) => process_debug_screenshot(image_data, &groq, &tx, conversation.clone(), buffer.clone(), connected.clone()).await,
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
            info!("MCQ hotkey (Ctrl+Alt+Q) triggered — Gemini MCQ vision");
            match ScreenCapture::capture_now() {
                Ok(image_data) => process_mcq_screenshot(image_data, groq.clone(), &tx, conversation.clone(), buffer.clone(), connected.clone()).await,
                Err(e) => error!("MCQ capture failed: {}", e),
            }
        }
    }
}

/// Gemini capture hotkey (Ctrl+Alt+A) — vision solve with multi-image support
async fn start_gemini_capture_hotkey(
    tx: broadcast::Sender<String>,
    groq: Arc<GroqClient>,
    conversation: ConversationHistory,
    candidate_context: CandidateContext,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
    ui_chunks: Arc<RwLock<Vec<String>>>,
) {
    use winapi::um::winuser::{GetAsyncKeyState, VK_CONTROL, VK_MENU};
    let mut was_pressed = false;
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL) } < 0;
        let alt = unsafe { GetAsyncKeyState(VK_MENU) } < 0;
        let a_key = unsafe { GetAsyncKeyState(0x41) } < 0;
        let is_pressed = ctrl && alt && a_key;
        if is_pressed && !was_pressed {
            info!("[Gemini] Ctrl+Alt+A pressed!");
            let current_image = ScreenCapture::capture_now().ok();
            let mut chunks = ui_chunks.write().await;
            let mut all_images: Vec<String> = std::mem::take(&mut *chunks);
            drop(chunks);
            if let Some(img) = current_image { all_images.push(img); }
            if all_images.is_empty() { was_pressed = is_pressed; continue; }
            let _ = tx.send(serde_json::json!({ "type": "multi_capture_update", "count": 0 }).to_string());
            let tx_c = tx.clone();
            let groq_c = groq.clone();
            let conv_c = conversation.clone();
            let ctx_c = candidate_context.clone();
            let buf_c = buffer.clone();
            let conn_c = connected.clone();
            tokio::spawn(async move {
                handle_gemini_capture_multi(tx_c, groq_c, conv_c, ctx_c, buf_c, conn_c, all_images).await;
            });
        }
        was_pressed = is_pressed;
    }
}

/// Gemini debug hotkey (Ctrl+Alt+F)
async fn start_gemini_debug_hotkey(
    tx: broadcast::Sender<String>,
    groq: Arc<GroqClient>,
    conversation: ConversationHistory,
    buffer: MessageBuffer,
    connected: Arc<AtomicUsize>,
) {
    use winapi::um::winuser::{GetAsyncKeyState, VK_CONTROL, VK_MENU};
    let mut was_pressed = false;
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL) } < 0;
        let alt = unsafe { GetAsyncKeyState(VK_MENU) } < 0;
        let f_key = unsafe { GetAsyncKeyState(0x46) } < 0;
        let is_pressed = ctrl && alt && f_key;
        if is_pressed && !was_pressed {
            info!("[Gemini Debug] Ctrl+Alt+F pressed!");
            let tx_c = tx.clone();
            let groq_c = groq.clone();
            let conv_c = conversation.clone();
            let buf_c = buffer.clone();
            let conn_c = connected.clone();
            tokio::spawn(async move {
                handle_debug_code_gemini(tx_c, groq_c, conv_c, buf_c, conn_c).await;
            });
        }
        was_pressed = is_pressed;
    }
}
