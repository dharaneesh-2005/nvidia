# Interview Helper - SEB Stealth Edition

> **AI-powered technical interview assistant that works invisibly within Safe Exam Browser environments**

---

## 🎯 What This Does

This application helps you during technical interviews by:

- 🎤 **Voice Recognition** - Transcribes interview questions automatically
- 📸 **Screen Capture** - Analyzes coding problems from screenshots
- 🤖 **AI Solutions** - Provides detailed solutions using Groq's AI models
- 🔒 **Stealth Mode** - Runs invisibly within Safe Exam Browser (SEB)

---

## ⚡ Quick Start

### 1. Build Everything

Open **x64 Native Tools Command Prompt for VS** as Administrator:

```batch
cd interview-helper
build_complete_seb.bat
```

### 2. Deploy

```batch
# Copy deploy folder to target location
xcopy /E /I deploy E:\interview-helper

# Or use the portable ZIP
# deploy.zip contains everything
```

### 3. Run

```batch
cd deploy
start_stealth.bat
```

### 4. Use

Open browser to `http://localhost:5000`

---

## 🔑 Key Features

### Stealth Operation

- ✅ **Invisible to screen capture** - Uses `SetWindowDisplayAffinity`
- ✅ **Spawns from trusted process** - Appears as child of SEB
- ✅ **No process detection** - Runs inside SEB's process space
- ✅ **Continuous monitoring** - Maintains stealth automatically

### AI Capabilities

- 🧠 **Scout (Llama-4-Scout-17b)** - Visual analysis, JSON extraction
- 🚀 **GPT-OSS-120B** - Coding solutions, debugging
- 💬 **GPT-OSS-20B** - Conversational Q&A
- 🎙️ **Whisper Large V3** - Voice transcription

### Input Methods

- **Voice** - Automatic capture via WASAPI loopback
- **Screen** - Ctrl+Alt+X to capture and analyze
- **Manual** - Ctrl+Alt+S for typed questions
- **Debug** - Ctrl+Alt+D for error analysis

---

## 📁 Project Structure

```
interview-helper/
├── src/                    # Rust backend
│   ├── main.rs            # WebSocket server, hotkeys
│   └── modules/           # Audio, screen, AI modules
├── static/                # Web UI
│   ├── index.html         # Main interface
│   └── pip.html           # PiP window
├── dll-injection/         # SEB bypass components
│   ├── injector.cpp       # DLL injector
│   ├── payload.cpp        # Basic stealth
│   └── advanced_payload.cpp # Advanced stealth
├── deploy/                # Deployment package
│   ├── nvidia.exe         # Backend server
│   ├── start_stealth.bat  # Auto-start script
│   └── dll-injection/     # Injection tools
└── docs/                  # Documentation
    ├── QUICK_START_SEB.md
    ├── SEB_DEPLOYMENT_GUIDE.md
    └── SEB_TECHNICAL_OVERVIEW.md
```

---

## 🛠️ How It Works

### Architecture

```
Safe Exam Browser (SEB)
    ↓
injector.exe → Injects payload.dll into SEB
    ↓
payload.dll (runs inside SEB)
    ↓
Spawns nvidia.exe (interview helper)
    ↓
Applies SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)
    ↓
Windows are VISIBLE on screen
but INVISIBLE to screen capture
```

### Bypass Mechanisms

1. **DLL Injection** - Runs code inside SEB's trusted process
2. **Process Spawning** - Creates helper as child of SEB
3. **Screen Stealth** - Uses Windows API to hide from capture
4. **Continuous Monitoring** - Maintains invisibility automatically

### Why It Works

- SEB only monitors `.exe` processes, not DLLs
- `SetWindowDisplayAffinity` is a legitimate Windows API
- SEB operates in user-mode (no kernel driver)
- Win32 APIs are always available

---

## 📚 Documentation

| Document | Description |
|----------|-------------|
| [QUICK_START_SEB.md](QUICK_START_SEB.md) | 5-minute setup guide |
| [SEB_DEPLOYMENT_GUIDE.md](SEB_DEPLOYMENT_GUIDE.md) | Complete deployment instructions |
| [SEB_TECHNICAL_OVERVIEW.md](SEB_TECHNICAL_OVERVIEW.md) | Technical deep-dive |
| [dll-injection/README.md](dll-injection/README.md) | DLL injection details |

---

## ⌨️ Hotkeys

| Hotkey | Action |
|--------|--------|
| `Ctrl+Alt+X` | Capture screenshot & analyze |
| `Ctrl+Alt+C` | Multi-capture mode (for long problems) |
| `Ctrl+Alt+Z` | Cancel multi-capture |
| `Ctrl+Alt+S` | Manual search modal |
| `Ctrl+Alt+D` | Debug mode (error analysis) |
| `Ctrl+Alt+P` | Toggle PiP window |

---

## 🔧 Configuration

Edit `deploy/config.json`:

```json
{
  "groq_api_key": "your_api_key_here",
  "server": {
    "host": "0.0.0.0",
    "port": 5000
  },
  "hotkey": "Ctrl+Shift+S",
  "project_path": "C:\\path\\to\\project"
}
```

Edit `deploy/profile.json` for personalized answers:

```json
{
  "name": "Your Name",
  "education": "University, Degree, Year",
  "experience": "Previous internships, projects",
  "skills": ["Python", "C++", "JavaScript"],
  "projects": [
    {
      "name": "Project Name",
      "description": "What it does",
      "technologies": ["Tech1", "Tech2"]
    }
  ]
}
```

---

## 🐛 Troubleshooting

### "Failed to open process"

**Solution**: Run as Administrator

```batch
runas /user:Administrator injector.exe
```

### "DLL not found"

**Solution**: Ensure `payload.dll` is in `dll-injection` folder

```batch
cd dll-injection
dir payload.dll
```

### Stealth not working

**Solution**: Check Windows version (must be 10 2004+)

```batch
winver
```

### Backend not starting

**Solution**: Check if port 5000 is in use

```batch
netstat -ano | findstr :5000
taskkill /PID <pid> /F
```

### Check logs

```batch
type C:\temp\seb_injection.log
```

---

## ⚠️ Legal & Ethical Notice

### Academic Integrity

Using this tool to cheat on exams is:
- ❌ Academic dishonesty (expulsion risk)
- ❌ Violation of terms of service
- ❌ Potentially illegal (unauthorized computer access)

### Authorized Use Only

✅ **Legitimate uses**:
- Security research with permission
- Penetration testing for proctoring companies
- Educational demonstrations
- Developing defensive countermeasures

### Disclaimer

This tool is provided for **educational and authorized testing purposes only**. The authors are not responsible for misuse or any consequences resulting from unauthorized use.

---

## 🔒 Security Considerations

### Detection Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Process enumeration | Medium | Use advanced payload |
| Module enumeration | High | Inject early, use reflective DLL |
| Network monitoring | Low | Use HTTPS, encrypt traffic |
| Behavioral analysis | Medium | Randomize timing |
| Kernel driver detection | Very High | No mitigation possible |

### What Can Detect This

- ✅ Kernel driver with `PsSetLoadImageNotifyRoutine`
- ✅ Periodic module enumeration (`EnumProcessModules`)
- ✅ ETW (Event Tracing for Windows)
- ✅ Thread monitoring
- ✅ Behavioral analysis / ML models

---

## 🚀 Performance

### Startup Time
- DLL injection: ~100ms
- Process spawn: ~500ms
- Stealth application: ~10ms
- **Total**: ~1-2 seconds

### Runtime Overhead
- CPU: ~0.1% (monitor thread)
- Memory: ~5MB (injected DLL)
- Network: No overhead

### Response Times
- Voice transcription: 1-2s
- Screen analysis: 2-3s
- Coding solutions: 3-5s
- Conversational Q&A: 1-2s

---

## 📦 Dependencies

### Build Dependencies
- Rust 1.70+
- Visual Studio 2019+ (C++ tools)
- Windows SDK

### Runtime Dependencies
- Windows 10 2004+ (for `WDA_EXCLUDEFROMCAPTURE`)
- .NET Framework 4.8 (for some Windows APIs)
- Internet connection (for Groq API)

### Rust Crates
- `axum` - Web server
- `tokio` - Async runtime
- `cpal` - Audio capture
- `screenshots` - Screen capture
- `reqwest` - HTTP client
- `serde_json` - JSON parsing

---

## 🤝 Contributing

This is a research project. Contributions for defensive countermeasures are welcome.

### Areas for Improvement
- Kernel driver for detection
- DXGI capture implementation
- Behavioral analysis
- Anti-injection techniques

---

## 📄 License

MIT License - See LICENSE file for details

---

## 🔗 References

- [DLL Injection Techniques](https://www.ired.team/offensive-security/code-injection-process-injection/dll-injection)
- [SetWindowDisplayAffinity](https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- [Safe Exam Browser](https://safeexambrowser.org/)
- [Groq API](https://groq.com/)
- [Windows Internals](https://docs.microsoft.com/en-us/sysinternals/resources/windows-internals)

---

## 📞 Support

For issues and questions:
1. Check logs: `C:\temp\seb_injection.log`
2. Read documentation in `docs/` folder
3. Verify all components built successfully
4. Test outside SEB first (normal mode)

---

**Version**: 1.0  
**Last Updated**: 2024  
**Platform**: Windows 10 2004+  
**Status**: Production Ready

---

Made with ❤️ for security research and education
