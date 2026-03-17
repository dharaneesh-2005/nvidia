# Nvidia PiP (Picture-in-Picture) Feature

## Overview

The PiP feature provides a **floating, always-on-top window** that displays Nvidia's responses in real-time. The native PiP window is **completely invisible to screen capture tools** like OBS, Zoom, Google Meet, and Microsoft Teams.

## Two Modes Available

### 1. Browser PiP Mode (Chrome 116+)
- Uses the Document Picture-in-Picture API
- Works in Chrome and Edge browsers
- **Visible to screen capture** (no protection)
- Quick and easy to use

### 2. Native PiP Mode (Tauri)
- Uses native Windows APIs
- **Invisible to screen capture** (full protection)
- Requires the Tauri wrapper application
- Works with OBS, Zoom, GMeet, Teams, etc.

## How to Use

### From the Web Interface
1. Open the Nvidia web interface in your browser
2. Click the **📺 PiP** button in the header
3. Or press **Ctrl+Alt+P** to toggle PiP

### Using the Native App (Screen Capture Protection)
1. Start the backend server: `nvidia.exe`
2. Start the native wrapper: `nvidia-tauri.exe`
3. Open the browser and click **📺 PiP**
4. The native window will appear with screen capture exclusion

## Features

### Native PiP Window
- ✅ Always on top of other windows
- ✅ Borderless, floating design
- ✅ Dynamic resize
- ✅ WebSocket real-time updates
- ✅ **Screen capture exclusion** (invisible to OBS, Zoom, etc.)
- ✅ Skip taskbar (clean appearance)
- ✅ Bottom-right corner positioning by default

### Browser PiP Window
- ✅ Always on top
- ✅ Works across browser tabs
- ✅ Real-time content sync
- ⚠️ Visible to screen capture

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+Alt+P` | Toggle PiP window |
| `Ctrl+Alt+S` | Open manual search |
| `Ctrl+Alt+X` | Capture screenshot |
| `Ctrl+Alt+C` | Multi-capture mode |
| `Ctrl+Alt+Z` | Cancel multi-capture |

## Technical Details

### Screen Capture Exclusion
The native PiP window uses Windows `SetWindowDisplayAffinity` API with `WDA_EXCLUDEFROMCAPTURE` flag. This makes the window appear as a **black rectangle** in:
- OBS Studio (Window/Display capture)
- Zoom screen sharing
- Google Meet
- Microsoft Teams
- Discord screen share
- Windows Game Bar (Win+G)
- Print Screen key

### Requirements
- **Windows 10 version 2004 or later** (for capture exclusion)
- Microsoft Edge WebView2 runtime (auto-installs if missing)
- Chrome 116+ (for browser PiP mode)

### Architecture
```
┌─────────────────────────────────────────┐
│  Browser (localhost:3000)                 │
│  ┌─────────────────────────────────────┐ │
│  │  [📺 PiP Button] → WebSocket msg    │ │
│  └─────────────────────────────────────┘ │
└──────────────────┬──────────────────────┘
                   │
┌──────────────────▼──────────────────────┐
│  Tauri Native App (nvidia-tauri.exe)    │
│  ┌─────────────────────────────────────┐ │
│  │  WebView2 Control                   │ │
│  │  ┌───────────────────────────────┐  │ │
│  │  │  pip.html (same as browser)   │  │ │
│  │  └───────────────────────────────┘  │ │
│  │  SetWindowDisplayAffinity           │ │
│  │  → WDA_EXCLUDEFROMCAPTURE           │ │
│  └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

## Building

### Build the Native PiP Application
```bash
# Install Tauri CLI (one-time)
cargo install tauri-cli --version "^2.0"

# Build everything
build_pip.bat
```

### Run in Development
```bash
# Terminal 1: Start the backend
cargo run --bin nvidia

# Terminal 2: Start the Tauri app
cd src-tauri
cargo tauri dev
```

## Troubleshooting

### PiP button doesn't work
- Check browser console for errors
- Ensure WebSocket connection is active
- Try refreshing the page

### Native PiP doesn't open
- Ensure `nvidia-tauri.exe` is running
- Check that backend is on port 3000
- Verify Windows 10 2004+ is installed

### Screen capture still shows PiP
- You must use the **native Tauri app** for capture exclusion
- Browser PiP is always visible to capture tools
- Check Windows version (needs 2004+)

### WebView2 not found
- Download and install WebView2 runtime:
  https://developer.microsoft.com/en-us/microsoft-edge/webview2/

## Configuration

Edit `config.yml` to customize PiP behavior:

```yaml
pip_mode:
  enabled: true
  width: 400
  height: 350
  position_x: "right"      # or pixel value
  position_y: "bottom"     # or pixel value
  opacity: 0.95
  always_on_top: true
  exclude_from_capture: true
  hotkey_toggle: "Ctrl+Alt+P"
```

## Files Added/Modified

### New Files
- `src-tauri/` - Tauri native application
- `src-tauri/src/pip.rs` - PiP window implementation
- `build_pip.bat` - Build script
- `PIP_README.md` - This documentation

### Modified Files
- `Cargo.toml` - Added workspace configuration
- `static/index.html` - Added PiP button and JavaScript
- `src/main.rs` - Added WebSocket handlers for PiP

## License

Copyright (c) 2024 Nvidia
