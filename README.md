# Interview Helper - AI-Powered Interview Assistant

Stealth interview assistant that captures audio, processes with Groq AI, and displays answers on your phone via web browser.

## Features

- 🎤 **Real-time Audio Capture** - Captures system audio via Stereo Mix
- 🤖 **AI Processing** - Groq Whisper for transcription, GPT-OSS-120B for answers
- 📱 **Phone Display** - View answers on phone browser (stealth mode)
- 📸 **Screenshot Analysis** - Capture coding problems with Ctrl+Shift+S
- 🐛 **Debug Mode** - Analyze code errors with Ctrl+Shift+F
- 🔍 **Manual Search** - Type questions with Ctrl+Alt+S
- 💾 **Chat History** - Persistent conversation history
- 🔄 **Auto-Reconnect** - WebSocket reconnection with message buffering
- 📁 **Codebase Analysis** - Auto-index repos for Bug Bash/Integration rounds

## Quick Start

### 1. Enable Stereo Mix (Windows)
1. Right-click speaker icon → Sound settings
2. Sound Control Panel → Recording tab
3. Right-click → Show Disabled Devices
4. Enable "Stereo Mix" → Set as default

### 2. Configure
Edit `config.json`:
```json
{
  "groq_api_key": "your_groq_api_key_here",
  "server": {
    "host": "0.0.0.0",
    "port": 5000
  },
  "hotkey": "Ctrl+Shift+S",
  "interview_repo_path": "C:\\interview\\repo"
}
```

### 3. Run
```bash
cargo run --release
```

### 4. Access on Phone
Open browser: `http://YOUR_PC_IP:5000`

## Hotkeys

| Hotkey | Action |
|--------|--------|
| Ctrl+Shift+S | Capture screenshot for analysis |
| Ctrl+Shift+F | Debug code error |
| Ctrl+Alt+S | Open manual search |
| Ctrl+Shift+C | Clear chat history |

## Codebase Analysis (Bug Bash/Integration)

1. Clone interview repo to `C:\interview\repo`
2. Press Ctrl+Alt+S
3. Check "🔍 Auto-index codebase"
4. Ask: "Where is the webhook validation bug?"
5. AI analyzes entire codebase with GPT-OSS-120B

## Architecture

- **Backend**: Rust (Axum + Tokio)
- **Frontend**: HTML/CSS/JS with WebSocket
- **AI**: Groq API (Whisper, GPT-OSS-120B, Llama-4-Scout)
- **Audio**: WASAPI loopback (Stereo Mix)

## Build Installer

```bash
cargo build --release
iscc installer.iss
```

## Interview Usage

### HackerRank Assessment
- Audio capture works perfectly (no screen sharing)
- Use for algorithm hints, complexity analysis

### Coding Round
- Quick syntax lookups
- Edge case suggestions
- Time/space complexity reminders

### Bug Bash
- Auto-index codebase from `C:\interview\repo`
- Ask specific bug location questions
- Get file paths and fixes

### Integration Round
- Analyze architecture patterns
- API integration suggestions
- Boilerplate code generation

## Configuration

### Audio Settings
- Sample rate: 16000 Hz
- Channels: Mono
- Chunk duration: 2000ms

### AI Models
- Transcription: whisper-large-v3
- Text: openai/gpt-oss-20b
- Vision: meta-llama/llama-4-scout-17b-16e-instruct
- Codebase: openai/gpt-oss-120b

### Timing
- Silence timeout: 1000ms
- Energy drop timeout: 350ms
- Max audio duration: 30s

## Troubleshooting

### No Audio Capture
- Ensure Stereo Mix is enabled and set as default
- Check if audio is playing through speakers (not headphones)

### WebSocket Disconnects
- Auto-reconnect with exponential backoff (1s → 30s)
- Messages buffered during disconnect

### Codebase Not Found
- Verify repo cloned to `C:\interview\repo`
- Check path in `config.json`

## Security Notes

- API key stored in `config.json` (don't commit)
- Screen capture excluded from recording
- No data sent to external servers except Groq API

## License

MIT License - Use at your own risk

## Disclaimer

This tool is for educational purposes. Using AI assistance during interviews may violate interview policies. Use responsibly.
