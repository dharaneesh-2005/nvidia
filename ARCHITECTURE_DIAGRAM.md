# Architecture Diagram - SEB Stealth Mode

## System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         USER'S COMPUTER                              │
│                                                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                  Safe Exam Browser (SEB)                        │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  SafeExamBrowser.exe (Trusted Process)                   │  │ │
│  │  │                                                            │  │ │
│  │  │  ┌──────────────────────────────────────────────────────┐│  │ │
│  │  │  │  payload.dll (Injected via CreateRemoteThread)       ││  │ │
│  │  │  │                                                        ││  │ │
│  │  │  │  ┌─────────────────────────────────────────────────┐ ││  │ │
│  │  │  │  │  DllMain()                                      │ ││  │ │
│  │  │  │  │  ├─ Spawn nvidia.exe (CREATE_NO_WINDOW)         │ ││  │ │
│  │  │  │  │  ├─ Create MonitorThread()                      │ ││  │ │
│  │  │  │  │  └─ Log to C:\temp\seb_injection.log            │ ││  │ │
│  │  │  │  └─────────────────────────────────────────────────┘ ││  │ │
│  │  │  │                                                        ││  │ │
│  │  │  │  ┌─────────────────────────────────────────────────┐ ││  │ │
│  │  │  │  │  MonitorThread() [Runs continuously]            │ ││  │ │
│  │  │  │  │  ├─ EnumWindows() every 1 second                │ ││  │ │
│  │  │  │  │  ├─ Find nvidia.exe windows                     │ ││  │ │
│  │  │  │  │  ├─ SetWindowDisplayAffinity(WDA_EXCLUDE...)    │ ││  │ │
│  │  │  │  │  └─ SetWindowPos(HWND_TOPMOST)                  │ ││  │ │
│  │  │  │  └─────────────────────────────────────────────────┘ ││  │ │
│  │  │  └──────────────────────────────────────────────────────┘│  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                                                                  │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  SEB Browser (Chromium-based)                            │  │ │
│  │  │  ├─ URL: http://localhost:5000                           │  │ │
│  │  │  ├─ WebSocket connection to backend                      │  │ │
│  │  │  └─ Displays interview helper UI                         │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  nvidia.exe (Interview Helper Backend)                         │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  Axum Web Server (localhost:5000)                        │  │ │
│  │  │  ├─ HTTP: Serves static files (HTML/CSS/JS)             │  │ │
│  │  │  └─ WebSocket: Real-time bidirectional communication    │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                                                                  │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  Audio Capture Module (WASAPI Loopback)                  │  │ │
│  │  │  ├─ Captures system audio (interview questions)          │  │ │
│  │  │  ├─ Voice Activity Detection (VAD)                       │  │ │
│  │  │  ├─ Downsamples to 16kHz mono                            │  │ │
│  │  │  └─ Sends to Groq Whisper API                            │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                                                                  │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  Screen Capture Module (Ctrl+Alt+X)                      │  │ │
│  │  │  ├─ Captures screenshot via screenshots crate            │  │ │
│  │  │  ├─ Encodes to PNG → Base64                              │  │ │
│  │  │  └─ Sends to Groq Scout API                              │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                                                                  │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  Hotkey Listeners (Windows API)                          │  │ │
│  │  │  ├─ Ctrl+Alt+X: Screenshot capture                       │  │ │
│  │  │  ├─ Ctrl+Alt+C: Multi-capture mode                       │  │ │
│  │  │  ├─ Ctrl+Alt+Z: Cancel multi-capture                     │  │ │
│  │  │  ├─ Ctrl+Alt+S: Manual search modal                      │  │ │
│  │  │  └─ Ctrl+Alt+D: Debug mode                               │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                                                                  │ │
│  │  ┌──────────────────────────────────────────────────────────┐  │ │
│  │  │  Conversation History (Arc<RwLock<Vec<Message>>>)        │  │ │
│  │  │  ├─ Stores last 20-25 messages                           │  │ │
│  │  │  ├─ Provides context to AI models                        │  │ │
│  │  │  └─ Thread-safe access                                   │  │ │
│  │  └──────────────────────────────────────────────────────────┘  │ │
│  │                                                                  │ │
│  │  Windows: INVISIBLE to screen capture (WDA_EXCLUDEFROMCAPTURE) │ │
│  │           VISIBLE on physical display                           │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                       │
└───────────────────────────────┬───────────────────────────────────────┘
                                │
                                │ HTTPS (TLS 1.3)
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         GROQ API (Cloud)                             │
│                                                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Whisper Large V3 (Voice Transcription)                        │ │
│  │  ├─ Input: WAV audio (16kHz mono)                              │ │
│  │  ├─ Output: Transcribed text                                   │ │
│  │  └─ Latency: ~1-2 seconds                                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  Scout (Llama-4-Scout-17b) - Visual Analysis                   │ │
│  │  ├─ Input: Base64 PNG image                                    │ │
│  │  ├─ Output: Structured JSON (problem type, details)            │ │
│  │  └─ Latency: ~2-3 seconds                                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  GPT-OSS-120B (Coding Solutions)                               │ │
│  │  ├─ Input: Problem description + conversation history          │ │
│  │  ├─ Output: Detailed solution (brute + optimal)                │ │
│  │  └─ Latency: ~3-5 seconds                                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                       │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  GPT-OSS-20B (Conversational Q&A)                              │ │
│  │  ├─ Input: Question + conversation history + profile           │ │
│  │  ├─ Output: Natural conversational answer                      │ │
│  │  └─ Latency: ~1-2 seconds                                      │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                       │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow Diagrams

### Voice Question Flow

```
User speaks question
        ↓
WASAPI Loopback captures system audio
        ↓
Audio Processor (VAD + downsampling)
        ↓
Groq Whisper API (transcription)
        ↓
Transcribed text → Conversation History
        ↓
Groq GPT-OSS-20B (with profile + history)
        ↓
Answer → Conversation History
        ↓
WebSocket → Browser UI
        ↓
User sees answer
```

### Screen Capture Flow

```
User presses Ctrl+Alt+X
        ↓
Screen Capture Module (screenshots crate)
        ↓
PNG → Base64 encoding
        ↓
Groq Scout API (visual analysis)
        ↓
Structured JSON (problem type, details)
        ↓
If DSA_PROBLEM:
    ↓
    Groq GPT-OSS-120B (coding solution)
    ↓
    Detailed solution (brute + optimal)
        ↓
Solution → Conversation History
        ↓
WebSocket → Browser UI
        ↓
User sees solution with code
```

### Manual Search Flow

```
User presses Ctrl+Alt+S
        ↓
Search modal opens (JavaScript)
        ↓
User types question
        ↓
Enter key → WebSocket message
        ↓
Backend receives "manual_question"
        ↓
Question → Conversation History
        ↓
Groq GPT-OSS-20B (with profile + history)
        ↓
Answer → Conversation History
        ↓
WebSocket → Browser UI
        ↓
User sees answer
```

---

## Injection Flow

```
User runs injector.exe
        ↓
Find SEB process (CreateToolhelp32Snapshot)
        ↓
Open process (OpenProcess with PROCESS_VM_WRITE)
        ↓
Allocate memory (VirtualAllocEx)
        ↓
Write DLL path (WriteProcessMemory)
        ↓
Create remote thread (CreateRemoteThread → LoadLibraryW)
        ↓
payload.dll loads in SEB's process space
        ↓
DllMain() executes
        ↓
Spawn nvidia.exe (CreateProcessW with stealth flags)
        ↓
Create MonitorThread()
        ↓
MonitorThread continuously applies stealth:
    ├─ EnumWindows() every 1 second
    ├─ Find nvidia.exe windows
    ├─ SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)
    └─ SetWindowPos(HWND_TOPMOST)
        ↓
nvidia.exe runs invisibly
```

---

## Stealth Mechanism

```
┌─────────────────────────────────────────────────────────────┐
│                    Physical Display                          │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                                                          │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │  SEB Browser (Visible)                           │  │ │
│  │  │  ├─ Exam questions                               │  │ │
│  │  │  └─ Answer input                                 │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  │                                                          │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │  Interview Helper UI (Visible)                   │  │ │
│  │  │  ├─ Chat interface                               │  │ │
│  │  │  ├─ AI responses                                 │  │ │
│  │  │  └─ Code solutions                               │  │ │
│  │  │                                                    │  │ │
│  │  │  WDA_EXCLUDEFROMCAPTURE applied ✓                │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  │                                                          │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ User sees both windows
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              Screen Capture (BitBlt/PrintWindow)             │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                                                          │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │  SEB Browser (Captured)                          │  │ │
│  │  │  ├─ Exam questions                               │  │ │
│  │  │  └─ Answer input                                 │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  │                                                          │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │  [BLACK RECTANGLE]                               │  │ │
│  │  │  Interview Helper NOT captured                   │  │ │
│  │  │  WDA_EXCLUDEFROMCAPTURE blocks capture           │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  │                                                          │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                               │
│  SEB's monitoring sees only the exam browser                 │
│  Interview helper is INVISIBLE                               │
└─────────────────────────────────────────────────────────────┘
```

---

## Component Interaction

```
┌──────────────┐
│  injector.exe│
└──────┬───────┘
       │ Injects
       ▼
┌──────────────────────────────────────────────────────────┐
│  SafeExamBrowser.exe (SEB Process)                       │
│  ┌────────────────────────────────────────────────────┐  │
│  │  payload.dll                                       │  │
│  │  ├─ Spawns ──────────────────────────────────────┐ │  │
│  │  └─ Monitors ────────────────────────────────────┐│ │  │
│  └──────────────────────────────────────────────────┘│ │  │
└────────────────────────────────────────────────────────┼─┼──┘
                                                         │ │
                                                         ▼ │
┌──────────────────────────────────────────────────────────┼──┐
│  nvidia.exe (Interview Helper)                           │  │
│  ┌────────────────────────────────────────────────────┐  │  │
│  │  Axum Server                                       │  │  │
│  │  ├─ HTTP (static files)                           │  │  │
│  │  └─ WebSocket (real-time)                         │  │  │
│  └────────────────────────────────────────────────────┘  │  │
│                                                           │  │
│  ┌────────────────────────────────────────────────────┐  │  │
│  │  Audio Capture (WASAPI)                           │  │  │
│  └────────────────────────────────────────────────────┘  │  │
│                                                           │  │
│  ┌────────────────────────────────────────────────────┐  │  │
│  │  Screen Capture (Hotkeys)                         │  │  │
│  └────────────────────────────────────────────────────┘  │  │
│                                                           │  │
│  ┌────────────────────────────────────────────────────┐  │  │
│  │  Groq API Client                                   │  │  │
│  └────────────────────────────────────────────────────┘  │  │
│                                                           │  │
│  Windows: INVISIBLE to capture ◄─────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
       │
       │ WebSocket
       ▼
┌──────────────────────────────────────────────────────────┐
│  Browser (in SEB)                                         │
│  ┌────────────────────────────────────────────────────┐  │
│  │  http://localhost:5000                            │  │
│  │  ├─ Chat UI                                       │  │
│  │  ├─ Voice indicator                               │  │
│  │  ├─ Capture buttons                               │  │
│  │  └─ AI responses                                  │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

---

## Thread Architecture

```
nvidia.exe Process
├─ Main Thread
│  ├─ Axum HTTP Server
│  ├─ WebSocket Handler
│  └─ Configuration Loading
│
├─ Audio Capture Thread
│  ├─ WASAPI Loopback
│  ├─ Voice Activity Detection
│  └─ Transcription Pipeline
│
├─ Screen Capture Hotkey Thread
│  ├─ RegisterHotKey (Ctrl+Alt+X)
│  ├─ RegisterHotKey (Ctrl+Alt+C)
│  └─ RegisterHotKey (Ctrl+Alt+Z)
│
├─ Search Hotkey Thread
│  └─ RegisterHotKey (Ctrl+Alt+S)
│
├─ Debug Hotkey Thread
│  └─ RegisterHotKey (Ctrl+Alt+D)
│
└─ Tokio Runtime Threads (async tasks)
   ├─ WebSocket connections
   ├─ HTTP requests to Groq API
   └─ Conversation history management

payload.dll (in SEB process)
├─ DllMain Thread
│  └─ Spawns nvidia.exe
│
└─ Monitor Thread
   ├─ EnumWindows() loop
   ├─ SetWindowDisplayAffinity()
   └─ SetWindowPos()
```

---

## File System Layout

```
deploy/
├─ nvidia.exe                    ← Backend server (Rust)
├─ config.json                   ← Configuration (API keys, ports)
├─ profile.json                  ← User profile (for personalized answers)
├─ start_stealth.bat             ← Auto-start script
│
├─ dll-injection/
│  ├─ injector.exe               ← DLL injector (C++)
│  ├─ payload.dll                ← Basic stealth payload (C++)
│  └─ advanced_payload.dll       ← Advanced stealth (C++)
│
├─ static/
│  ├─ index.html                 ← Main UI
│  ├─ pip.html                   ← PiP window UI
│  ├─ css/
│  │  └─ search.css              ← Search modal styles
│  └─ js/
│     └─ search.js               ← Search modal logic
│
└─ docs/
   ├─ README_SEB.md              ← Project overview
   ├─ QUICK_START_SEB.md         ← 5-minute guide
   ├─ SEB_DEPLOYMENT_GUIDE.md    ← Complete guide
   └─ SEB_TECHNICAL_OVERVIEW.md  ← Technical details
```

---

## Network Communication

```
Browser (localhost:5000)
    │
    │ HTTP GET /
    ├──────────────────────────► Axum Server
    │◄────────────────────────── index.html
    │
    │ WebSocket /ws
    ├──────────────────────────► WebSocket Handler
    │◄────────────────────────── Connected
    │
    │ {"type": "capture_screen"}
    ├──────────────────────────► Screen Capture Module
    │                                    │
    │                                    │ HTTPS
    │                                    ├──────────► Groq Scout API
    │                                    │◄────────── JSON response
    │                                    │
    │                                    │ HTTPS
    │                                    ├──────────► Groq GPT-OSS-120B
    │                                    │◄────────── Solution
    │◄────────────────────────── {"type": "answer", "text": "..."}
    │
    │ Voice detected
    │◄────────────────────────── {"type": "transcription", "text": "..."}
    │                                    │
    │                                    │ HTTPS
    │                                    ├──────────► Groq Whisper API
    │                                    │◄────────── Transcribed text
    │                                    │
    │                                    │ HTTPS
    │                                    ├──────────► Groq GPT-OSS-20B
    │                                    │◄────────── Answer
    │◄────────────────────────── {"type": "answer", "text": "..."}
```

---

## Security Layers

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: Process Trust                                      │
│  ├─ nvidia.exe spawned from SEB (trusted parent)            │
│  └─ Appears as legitimate subprocess                         │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: DLL Invisibility                                   │
│  ├─ payload.dll runs inside SEB's process                   │
│  └─ SEB cannot see DLLs loaded in its own process           │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: Screen Capture Bypass                              │
│  ├─ SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)        │
│  └─ Windows are invisible to BitBlt/PrintWindow             │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 4: Continuous Maintenance                             │
│  ├─ Monitor thread reapplies stealth every 1 second         │
│  └─ Handles new windows automatically                        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 5: Network Stealth                                    │
│  ├─ Direct socket access (bypasses SEB's URL filter)        │
│  └─ HTTPS encryption (TLS 1.3)                              │
└─────────────────────────────────────────────────────────────┘
```

---

**Diagram Version**: 1.0  
**Last Updated**: 2024  
**Format**: ASCII Art (for maximum compatibility)
