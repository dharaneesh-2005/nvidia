# System Architecture

## Overview

Interview Helper is a stealth interview assistance system that runs entirely on your PC with no visible UI, displaying everything on your phone via a web interface.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         PC (Windows)                             │
│                                                                   │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │              Rust Service (No UI/Taskbar)                   │ │
│  │                                                              │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌─────────────────┐  │ │
│  │  │ Audio Module │  │Screen Module │  │  Code Module    │  │ │
│  │  │              │  │              │  │                 │  │ │
│  │  │ - cpal       │  │ - screenshots│  │ - File indexing │  │ │
│  │  │ - WASAPI     │  │ - hotkey     │  │ - AST parsing   │  │ │
│  │  │ - WAV encode │  │ - PNG encode │  │ - Search        │  │ │
│  │  └──────┬───────┘  └──────┬───────┘  └────────┬────────┘  │ │
│  │         │                  │                    │           │ │
│  │         └──────────────────┼────────────────────┘           │ │
│  │                            ▼                                │ │
│  │                  ┌──────────────────┐                       │ │
│  │                  │  Groq API Client │                       │ │
│  │                  │                  │                       │ │
│  │                  │ - Whisper v3     │                       │ │
│  │                  │ - Llama 3.3 70B  │                       │ │
│  │                  │ - Llama 3.2 90B  │                       │ │
│  │                  │   Vision         │                       │ │
│  │                  └──────────────────┘                       │ │
│  │                            │                                │ │
│  │                            ▼                                │ │
│  │                  ┌──────────────────┐                       │ │
│  │                  │  Axum Web Server │                       │ │
│  │                  │                  │                       │ │
│  │                  │ - HTTP Server    │                       │ │
│  │                  │ - WebSocket      │                       │ │
│  │                  │ - Static files   │                       │ │
│  │                  │ - REST API       │                       │ │
│  │                  └──────────────────┘                       │ │
│  │                            │                                │ │
│  └────────────────────────────┼────────────────────────────────┘ │
│                               │                                  │
│                               │ localhost:5000                   │
│                               │ (exposed to network)             │
└───────────────────────────────┼──────────────────────────────────┘
                                │
                                │ WiFi Network
                                │
┌───────────────────────────────▼──────────────────────────────────┐
│                      Phone (Web Browser)                          │
│                                                                   │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    Web Interface                            │ │
│  │                                                              │ │
│  │  ┌─────────────────────────────────────────────────────┐  │ │
│  │  │ Live Tab                                             │  │ │
│  │  │  - Real-time transcription display                   │  │ │
│  │  │  - AI-generated answers                              │  │ │
│  │  │  - Screenshot viewer                                 │  │ │
│  │  │  - Vision analysis results                           │  │ │
│  │  └─────────────────────────────────────────────────────┘  │ │
│  │                                                              │ │
│  │  ┌─────────────────────────────────────────────────────┐  │ │
│  │  │ Code Tab                                             │  │ │
│  │  │  - File tree browser                                 │  │ │
│  │  │  - Code viewer with syntax highlighting              │  │ │
│  │  │  - AI query interface                                │  │ │
│  │  │  - Code explanations                                 │  │ │
│  │  └─────────────────────────────────────────────────────┘  │ │
│  │                                                              │ │
│  │  WebSocket Connection (real-time updates)                   │ │
│  └────────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Audio Capture Flow

```
System Audio
    ↓
WASAPI Loopback (cpal)
    ↓
Buffer (3 second chunks)
    ↓
WAV Encoding (16kHz, mono)
    ↓
Groq Whisper API
    ↓
Transcription Text
    ↓
WebSocket → Phone Display
    ↓
Groq Llama 3.3 70B
    ↓
AI Answer
    ↓
WebSocket → Phone Display
```

### 2. Screen Capture Flow

```
User presses Ctrl+Shift+F12
    ↓
Global Hotkey Listener
    ↓
Screenshot Capture (screenshots crate)
    ↓
PNG Encoding
    ↓
Base64 Encoding
    ↓
WebSocket → Phone Display
    ↓
Groq Llama 3.2 90B Vision
    ↓
Image Analysis
    ↓
WebSocket → Phone Display
```

### 3. Code Query Flow

```
User asks question on phone
    ↓
HTTP POST /api/code/query
    ↓
Code Manager searches project
    ↓
Relevant files extracted
    ↓
Context + Query → Groq Llama 3.3 70B
    ↓
AI Response
    ↓
JSON Response → Phone Display
```

## Component Details

### Audio Module (`src/modules/audio.rs`)

**Responsibilities:**
- Capture system audio using WASAPI loopback
- Buffer audio in 3-second chunks
- Convert to WAV format (16kHz, mono)
- Thread-safe buffer management

**Key Technologies:**
- `cpal`: Cross-platform audio library
- `hound`: WAV encoding
- `Arc<Mutex<>>`: Thread-safe buffer

### Screen Module (`src/modules/screen.rs`)

**Responsibilities:**
- Register global hotkey (Ctrl+Shift+F12)
- Capture screen on hotkey press
- Encode to PNG and Base64
- Thread-safe capture queue

**Key Technologies:**
- `screenshots`: Screen capture
- `global-hotkey`: System-wide hotkey registration
- `image`: Image processing

### Groq Module (`src/modules/groq.rs`)

**Responsibilities:**
- Interface with Groq API
- Audio transcription (Whisper)
- Text generation (Llama 3.3 70B)
- Vision analysis (Llama 3.2 90B)

**API Endpoints:**
- `POST /openai/v1/audio/transcriptions`
- `POST /openai/v1/chat/completions`

### Code Module (`src/modules/code.rs`)

**Responsibilities:**
- Index project files
- Generate file tree
- Read file contents
- Search for relevant code context
- Filter code files by extension

**Supported Extensions:**
- Languages: rs, py, js, ts, java, c, cpp, go, rb, php, cs, swift, kt, scala
- Config: json, yaml, toml, xml
- Docs: md, html, css

### Web Server (Axum)

**Endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` | Serve main HTML |
| GET | `/ws` | WebSocket connection |
| GET | `/api/code/files` | Get file tree |
| POST | `/api/code/content` | Get file content |
| POST | `/api/code/query` | Query code with AI |
| GET | `/static/*` | Static assets |

**WebSocket Messages:**

```json
// Transcription
{
  "type": "transcription",
  "text": "What is a binary search tree?"
}

// AI Answer
{
  "type": "answer",
  "text": "A binary search tree is..."
}

// Screenshot
{
  "type": "screenshot",
  "image": "base64_encoded_png"
}

// Analysis
{
  "type": "analysis",
  "text": "This code implements..."
}
```

## Concurrency Model

### Async Runtime: Tokio

- Main thread: Axum web server
- Spawn task: Audio capture loop
- Spawn task: Screen capture listener
- WebSocket: Per-connection async task

### Thread Safety

- Audio buffer: `Arc<Mutex<Vec<f32>>>`
- Screen capture queue: `Arc<Mutex<Option<String>>>`
- Broadcast channel: `tokio::sync::broadcast`

## Performance Characteristics

### Latency

- Audio capture: ~3 seconds (configurable)
- Whisper transcription: ~1-2 seconds
- Text generation: ~2-3 seconds
- Screen capture: <100ms
- Vision analysis: ~3-4 seconds
- WebSocket latency: <50ms (local network)

### Resource Usage

- Memory: ~50-100 MB
- CPU: <5% idle, ~20% during processing
- Network: ~1-5 KB/s (audio), ~100-500 KB (screenshot)

## Security Considerations

### Network Security

- Default: localhost only
- Production: Bind to 0.0.0.0 (all interfaces)
- Recommendation: Use firewall rules to restrict access
- No authentication (local network trust model)

### Data Privacy

- All processing on local PC
- Only API calls to Groq servers
- No data stored permanently
- Audio/screenshots not saved to disk

### API Key Security

- Stored in config.json
- Not committed to git (.gitignore)
- Loaded at startup only
- Not exposed via API

## Deployment Options

### 1. Development Mode

```bash
cargo run
```

- Console output visible
- Easy debugging
- Manual start/stop

### 2. Release Binary

```bash
cargo build --release
target\release\interview_helper.exe
```

- Optimized performance
- No console window (with proper build flags)
- Manual start/stop

### 3. Windows Service (NSSM)

```bash
nssm install InterviewHelper
```

- Auto-start on boot
- Runs in background
- No console window
- Service management via Windows

### 4. Windows Service (Native)

- Use `windows-service` crate
- Compile as Windows service
- Full Windows integration
- Most complex setup

## Extensibility

### Adding New AI Models

1. Add model name to `config.json`
2. Update `GroqClient` methods
3. Handle new response formats

### Adding New Capture Sources

1. Create new module in `src/modules/`
2. Implement capture logic
3. Spawn task in `main.rs`
4. Send via broadcast channel

### Adding New UI Features

1. Update `static/index.html`
2. Add CSS in `static/css/style.css`
3. Handle in `static/js/app.js`
4. Add API endpoint if needed

## Error Handling

### Audio Capture Errors

- Device not found: Log and retry
- Buffer overflow: Clear and continue
- Encoding error: Skip chunk

### Screen Capture Errors

- Hotkey conflict: Log warning
- Capture failure: Skip and wait for next

### API Errors

- Network error: Retry with backoff
- Rate limit: Queue and retry
- Invalid response: Log and skip

### WebSocket Errors

- Connection lost: Client auto-reconnects
- Send failure: Drop message
- Parse error: Log and ignore

## Testing Strategy

### Unit Tests

- Audio encoding/decoding
- File tree generation
- Code search logic

### Integration Tests

- Groq API calls (with mock)
- WebSocket communication
- File operations

### Manual Tests

- Audio capture quality
- Hotkey responsiveness
- UI on different devices
- Network connectivity

## Future Enhancements

1. **Local AI Models**: Run Whisper/Llama locally
2. **Multi-monitor**: Capture specific screen
3. **Recording**: Save sessions for review
4. **Authentication**: Secure phone access
5. **Code Editing**: AI-powered code modifications
6. **Voice Commands**: Control via speech
7. **Mobile App**: Native iOS/Android app
8. **Cloud Sync**: Optional cloud backup
