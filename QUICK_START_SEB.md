# Quick Start Guide - SEB Stealth Mode

## 5-Minute Setup

### Prerequisites
- Windows 10 2004+ 
- Administrator access
- Safe Exam Browser installed

---

## Step 1: Build (One-Time)

Open **x64 Native Tools Command Prompt for VS** as Administrator:

```batch
cd interview-helper
build_complete_seb.bat
```

Wait for build to complete (~2-5 minutes).

---

## Step 2: Deploy

Copy the `deploy` folder to your target location:

```batch
# Example: Copy to USB drive
xcopy /E /I deploy E:\interview-helper

# Or use the portable ZIP
# deploy.zip contains everything
```

---

## Step 3: Run

### Option A: Automatic (Recommended)

```batch
cd deploy
start_stealth.bat
```

This script:
- ✅ Detects if SEB is running
- ✅ Injects automatically if SEB found
- ✅ Starts normally if SEB not found
- ✅ Opens on `http://localhost:5000`

### Option B: Manual

1. Start Safe Exam Browser first
2. Run injector:
   ```batch
   cd deploy\dll-injection
   injector.exe
   ```
3. Open browser to `http://localhost:5000`

---

## Step 4: Verify

### Test Stealth Mode

1. Open the helper UI at `http://localhost:5000`
2. Take a screenshot (Win+Shift+S)
3. **Expected**: Helper should NOT appear in screenshot

### Test Functionality

- **Voice**: Speak a question → Should transcribe and answer
- **Screen**: Press `Ctrl+Alt+X` → Should capture and analyze
- **Manual**: Press `Ctrl+Alt+S` → Should open search modal

---

## Troubleshooting

### "Failed to open process"
→ Run as Administrator

### "DLL not found"
→ Ensure `payload.dll` is in `dll-injection` folder

### Stealth not working
→ Check Windows version: `winver` (must be 10 2004+)

### Backend not starting
→ Check port 5000: `netstat -ano | findstr :5000`

---

## Hotkeys

| Key | Action |
|-----|--------|
| `Ctrl+Alt+X` | Capture screenshot |
| `Ctrl+Alt+C` | Multi-capture mode |
| `Ctrl+Alt+S` | Manual search |
| `Ctrl+Alt+D` | Debug mode |
| `Ctrl+Alt+P` | Toggle PiP |

---

## Files Overview

```
deploy/
├── nvidia.exe              ← Backend server
├── config.json             ← Configuration
├── start_stealth.bat       ← Auto-start script
├── dll-injection/
│   ├── injector.exe        ← DLL injector
│   └── payload.dll         ← Stealth payload
└── static/                 ← Web UI
```

---

## Support

Check logs:
```batch
type C:\temp\seb_injection.log
```

Full documentation:
```
deploy\SEB_DEPLOYMENT_GUIDE.md
```

---

## Quick Commands

```batch
# Build everything
build_complete_seb.bat

# Start with auto-detection
cd deploy && start_stealth.bat

# Manual injection
cd deploy\dll-injection && injector.exe

# Check status
tasklist | findstr nvidia

# Stop
taskkill /IM nvidia.exe /F
```

---

**That's it!** You're ready to use the interview helper in SEB environments.

For advanced configuration and troubleshooting, see `SEB_DEPLOYMENT_GUIDE.md`.
