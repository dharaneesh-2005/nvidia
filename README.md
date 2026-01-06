# Interview Helper

A hidden Rust service that captures system audio and screen, processes everything with Groq AI, and displays results on your phone via web browser.

## Features

- **System Audio Capture**: Real-time capture with Whisper v3 Large transcription
- **AI Answers**: Instant responses using Llama 3.3 70B
- **Screen Capture**: Hotkey-triggered (Ctrl+Shift+F12) with Llama 3.2 90B Vision analysis
- **Code Browser**: Navigate and query your project codebase with AI
- **Stealth Mode**: No UI on main system, runs as background service
- **Phone Display**: Access everything via web browser on your phone

## Tech Stack

- **Backend**: Rust (Axum web framework)
- **AI**: Groq API (Whisper, Llama 3.3, Llama 3.2 Vision)
- **Audio**: cpal + hound
- **Screen**: screenshots + global-hotkey
- **Frontend**: Vanilla HTML/CSS/JS with WebSocket

## Setup

### 1. Prerequisites

- Rust (install from https://rustup.rs/)
- Groq API key (get from https://console.groq.com/)

### 2. Configure

Edit `config.json`:

```json
{
  "groq_api_key": "YOUR_GROQ_API_KEY",
  "project_path": "E:\\your\\project\\path"
}
```

### 3. Build

```bash
cd interview-helper
cargo build --release
```

### 4. Run

```bash
cargo run --release
```

Or run the binary directly:
```bash
target\release\interview_helper.exe
```

### 5. Access from Phone

1. Find your PC's IP address: `ipconfig` (look for IPv4)
2. Open browser on phone: `http://YOUR_PC_IP:5000`
3. Keep the browser open during your interview

## Usage

### Live Tab
- **Audio**: Automatically captures and transcribes system audio
- **AI Answer**: Provides instant responses to questions
- **Screen**: Press `Ctrl+Shift+F12` to capture and analyze

### Code Tab
- Browse your project files
- View code with syntax highlighting
- Ask AI questions about your codebase
- Get explanations and suggestions

## Install as Windows Service

### Using NSSM (Recommended)

1. Download NSSM: https://nssm.cc/download
2. Install service:

```cmd
nssm install InterviewHelper "E:\project\newphonewrtc\interview-helper\target\release\interview_helper.exe"
nssm set InterviewHelper AppDirectory "E:\project\newphonewrtc\interview-helper"
nssm set InterviewHelper DisplayName "Interview Helper"
nssm set InterviewHelper Description "Hidden interview assistant service"
nssm set InterviewHelper Start SERVICE_AUTO_START
nssm start InterviewHelper
```

### Verify Service

```cmd
nssm status InterviewHelper
```

### Remove Service

```cmd
nssm stop InterviewHelper
nssm remove InterviewHelper confirm
```

## Groq API Models Used

- **whisper-large-v3**: Audio transcription (fastest, most accurate)
- **llama-3.3-70b-versatile**: Text generation and Q&A
- **llama-3.2-90b-vision-preview**: Image analysis

## Architecture

```
PC (Rust Service)
├── Audio Capture (cpal) → Groq Whisper → Transcription
├── Screen Capture (hotkey) → Groq Vision → Analysis
├── Code Manager → File indexing + AI queries
└── Axum Server (localhost:5000)
    └── WebSocket → Phone Browser
```

## Security Notes

- Service runs on localhost by default
- Change `server.host` to `0.0.0.0` to allow network access
- Use firewall rules to restrict access
- Groq API key is stored in config.json (keep secure)
- All processing happens on your PC
- Only API calls go to Groq servers

## Troubleshooting

### Audio not capturing
- Check Windows audio settings
- Ensure microphone/system audio is enabled
- Try running as administrator

### Hotkey not working
- Check if another app uses the same hotkey
- Change hotkey in config.json
- Run as administrator for global hotkey access

### Can't connect from phone
- Ensure PC and phone are on same WiFi
- Check Windows Firewall (allow port 5000)
- Verify PC IP address with `ipconfig`

### Groq API errors
- Verify API key in config.json
- Check Groq API status
- Ensure you have API credits

## Performance

- **Audio latency**: ~3 seconds (configurable)
- **Transcription**: ~1-2 seconds via Groq
- **AI response**: ~2-3 seconds
- **Screen capture**: Instant
- **Vision analysis**: ~3-4 seconds

## Development

Run in development mode:
```bash
cargo run
```

Build optimized release:
```bash
cargo build --release
```

Enable logging:
```bash
$env:RUST_LOG="info"
cargo run
```

## License

MIT
