mod modules;

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::VecDeque;
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
    client_connected: Arc<RwLock<bool>>,
    search_hotkey: Arc<tokio::sync::Mutex<SearchHotkey>>,
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

async fn send_or_buffer(tx: &broadcast::Sender<String>, msg: String, buffer: &MessageBuffer, connected: &Arc<RwLock<bool>>) {
    if *connected.read().await {
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
    tokio::spawn(start_search_hotkey_listener(tx.clone(), search_hotkey.clone()));
    
    let state = AppState {
        tx: tx.clone(),
        groq: groq.clone(),
        code_manager: code_manager.clone(),
        conversation: Arc::new(RwLock::new(Vec::new())),
        message_buffer: Arc::new(RwLock::new(VecDeque::new())),
        client_connected: Arc::new(RwLock::new(false)),
        search_hotkey: search_hotkey,
    };
    
    // Start audio capture with proper streaming
    tokio::spawn(start_audio_capture(tx.clone(), groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connected.clone()));
    
    // Start screen capture hotkey listener
    tokio::spawn(start_screen_capture_hotkey(tx.clone(), groq.clone(), config.hotkey.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connected.clone()));
    
    // Start debug hotkey listener
    tokio::spawn(start_debug_hotkey_listener(tx.clone(), groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connected.clone()));
    
    // Build web server
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .route("/api/code/files", get(get_files))
        .route("/api/code/content", post(get_file_content))
        .route("/api/code/query", post(query_code))
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

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();
    
    // Mark client as connected
    *state.client_connected.write().await = true;
    
    // Send buffered messages first
    {
        let mut buffer = state.message_buffer.write().await;
        while let Some(msg) = buffer.pop_front() {
            if socket.send(axum::extract::ws::Message::Text(msg)).await.is_err() {
                *state.client_connected.write().await = false;
                return;
            }
        }
    }
    
    loop {
        tokio::select! {
            msg = rx.recv() => {
                if let Ok(msg) = msg {
                    if socket.send(axum::extract::ws::Message::Text(msg.clone())).await.is_err() {
                        *state.client_connected.write().await = false;
                        break;
                    }
                }
            }
            msg = socket.recv() => {
                if let Some(Ok(axum::extract::ws::Message::Text(text))) = msg {
                    if text == "capture_screen" {
                        tokio::spawn(handle_screen_capture(state.tx.clone(), state.groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connected.clone()));
                    } else if text == "debug_code" {
                        tokio::spawn(handle_debug_code(state.tx.clone(), state.groq.clone(), state.conversation.clone(), state.message_buffer.clone(), state.client_connected.clone()));
                    } else if text == "search_closed" {
                        let hotkey = state.search_hotkey.lock().await;
                        hotkey.deactivate();
                    } else if text.starts_with("{") {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                            let msg_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            if msg_type == "manual_question" {
                                let question = json.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                info!("🔍 [SEARCH LOG] Received manual_question: '{}'", question);
                                
                                if !question.is_empty() {
                                    let tx_clone = state.tx.clone();
                                    let groq = state.groq.clone();
                                    let conversation = state.conversation.clone();
                                    let buffer = state.message_buffer.clone();
                                    let connected = state.client_connected.clone();
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
                                        match groq.chat_with_history(&question, &history).await {
                                            Ok(answer) => {
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
                                            },
                                            Err(e) => {
                                                error!("🔍 [SEARCH LOG] AI request failed: {}", e);
                                                send_or_buffer(&tx_clone, serde_json::json!({
                                                    "type": "answer",
                                                    "text": format!("Error: {}", e)
                                                }).to_string(), &buffer, &connected).await;
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
                                    let connected = state.client_connected.clone();
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
                                                            send_or_buffer(&tx_clone, serde_json::json!({
                                                                "type": "transcription",
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
                    *state.client_connected.write().await = false;
                    break;
                }
            }
        }
    }
    
    *state.client_connected.write().await = false;
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

async fn start_audio_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<RwLock<bool>>) {
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



async fn handle_screen_capture(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<RwLock<bool>>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => {
            process_screenshot(image_data, &groq, &tx, conversation, buffer, connected).await;
        }
        Err(e) => {
            error!("Screen capture failed: {}", e);
        }
    }
}

async fn handle_debug_code(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<RwLock<bool>>) {
    match ScreenCapture::capture_now() {
        Ok(image_data) => {
            process_debug_screenshot(image_data, &groq, &tx, conversation, buffer, connected).await;
        }
        Err(e) => {
            error!("Debug capture failed: {}", e);
        }
    }
}

async fn start_screen_capture_hotkey(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, hotkey: String, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<RwLock<bool>>) {
    let mut screen_capture = ScreenCapture::new(hotkey);
    
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Some(image_data) = screen_capture.check_capture() {
            process_screenshot(image_data, &groq, &tx, conversation.clone(), buffer.clone(), connected.clone()).await;
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

async fn start_debug_hotkey_listener(tx: broadcast::Sender<String>, groq: Arc<GroqClient>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<RwLock<bool>>) {
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

async fn process_debug_screenshot(image_data: String, groq: &GroqClient, tx: &broadcast::Sender<String>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<RwLock<bool>>) {
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

async fn process_screenshot(image_data: String, groq: &GroqClient, tx: &broadcast::Sender<String>, conversation: ConversationHistory, buffer: MessageBuffer, connected: Arc<RwLock<bool>>) {
    info!("Processing screenshot...");
    
    send_or_buffer(tx, serde_json::json!({
        "type": "transcription",
        "text": "Screenshot captured - analyzing..."
    }).to_string(), &buffer, &connected).await;
    
    // Get conversation history for context
    let history = conversation.read().await.clone();
    
    match groq.analyze_image(&image_data, &history).await {
        Ok((analysis, _is_coding)) => {
            info!("Raw analysis from Scout: {}", &analysis[..analysis.len().min(200)]);
            
            let cleaned_analysis = analysis
                .trim()
                .strip_prefix("```json")
                .or_else(|| analysis.trim().strip_prefix("```"))
                .unwrap_or(analysis.trim())
                .strip_suffix("```")
                .unwrap_or(analysis.trim())
                .trim();
            
            info!("Cleaned analysis (first 300 chars): {}", &cleaned_analysis[..cleaned_analysis.len().min(300)]);
            
            let mut is_coding = false;
            let mut problem_type_str = String::from("GENERAL");
            
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(cleaned_analysis) {
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
                let problem_data = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(cleaned_analysis) {
                    let mut formatted = String::new();
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
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(cleaned_analysis) {
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
