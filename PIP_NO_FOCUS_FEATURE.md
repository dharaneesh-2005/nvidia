# PiP Window - Enhanced Features

## Overview
The PiP (Picture-in-Picture) window is a borderless, always-on-top window with advanced features:
- **No Focus Stealing**: Click the window without losing focus in your browser
- **Screen Capture Exclusion**: Invisible to OBS, Zoom, GMeet, etc.
- **Transparency Control**: Adjustable opacity from 10% to 100%
- **Smooth Toggle**: Ctrl+Alt+P properly recreates window with all flags intact

## Key Features

### 1. Transparency Control
Adjust window opacity with a smooth slider in the status bar.

**Features:**
- Range: 10% (nearly transparent) to 100% (fully opaque)
- Smooth GPU-accelerated transitions
- Saved to localStorage (persists across sessions)
- Real-time preview as you drag the slider

**Usage:**
- Use the "Opacity" slider in the status bar
- Drag left for more transparency, right for less
- Perfect for overlaying content while seeing through

### 2. Borderless Design
Clean, modern borderless window with custom controls.

**Features:**
- No Windows title bar or borders
- Custom drag handle (green "🔒 Nvidia PiP" bar)
- Minimize button in drag handle
- Close button in status bar (red ×)
- Resizable by dragging edges

**Controls:**
- **Drag**: Click and drag the green title bar
- **Minimize**: Click − button in title bar
- **Close**: Click × button in status bar (next to opacity slider)
- **Resize**: Drag window edges

### 3. Proper Toggle Behavior
Ctrl+Alt+P now correctly maintains all Windows flags.

**How It Works:**
- First press: Creates window with all security flags
- Second press: **Closes** window (not just hide)
- Third press: **Recreates** window with flags intact

**Why This Matters:**
- Windows loses `WDA_EXCLUDEFROMCAPTURE` when hiding/showing
- Windows loses `WS_EX_NOACTIVATE` when hiding/showing
- Closing and recreating ensures flags are always applied
- Screen capture exclusion works every time
- No focus stealing works every time

### 4. Centered Positioning
Window opens in the center of your screen (not bottom-right corner).

**Default Position:**
- Centered horizontally and vertically
- 400×350 pixels
- Can be moved anywhere after opening

## How It Works

### Windows Implementation
The PiP window uses the `WS_EX_NOACTIVATE` extended window style, which prevents the window from being activated when clicked.

**Technical Details:**
- **WS_EX_NOACTIVATE**: Windows extended style that prevents window activation
- Applied using `SetWindowLongPtrW` Win32 API
- Window remains always-on-top but doesn't steal focus
- You can still interact with the window (scroll, read content)
- The window won't activate when clicked

### Benefits

1. **No Browser Blur**: Your browser stays focused when clicking PiP
2. **Seamless Workflow**: Continue typing/working while viewing PiP
3. **Always Visible**: Window stays on top without interrupting your work
4. **Screen Capture Invisible**: Still hidden from OBS, Zoom, GMeet, etc.

## Usage Guide

### Opening the PiP Window
1. Click the 📺 PiP button in the web UI, OR
2. Press **Ctrl+Alt+P** (global hotkey)
3. Window opens centered on your screen

### Adjusting Transparency
1. Locate the "Opacity" slider in the status bar
2. Drag left for more transparency (10% minimum)
3. Drag right for less transparency (100% maximum)
4. Setting is saved automatically

### Moving the Window
1. Click and hold the green "🔒 Nvidia PiP" title bar
2. Drag to desired position
3. Release to drop

### Resizing the Window
1. Hover over any window edge
2. Cursor changes to resize arrows
3. Click and drag to resize

### Minimizing
- Click the **−** button in the title bar

### Closing
- Click the **×** button in the status bar (red button next to opacity slider), OR
- Press **Ctrl+Alt+P** again to toggle off

## UI Layout

```
┌─────────────────────────────────────────┐
│ 🔒 Nvidia PiP  Hidden from capture    − │ ← Drag handle + Minimize
├─────────────────────────────────────────┤
│ ● Connected  Opacity [====] 100%  ×    │ ← Status bar + Controls
├─────────────────────────────────────────┤
│                                         │
│         Chat messages appear here       │
│                                         │
├─────────────────────────────────────────┤
│              🗑️ Clear                   │ ← Footer
└─────────────────────────────────────────┘
```

## Technical Implementation

### Transparency Control

**Rust Command (`pip.rs`):**
```rust
#[tauri::command]
pub async fn set_pip_opacity(app: AppHandle, opacity: f64) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("pip") {
        let clamped_opacity = opacity.max(0.1).min(1.0);
        window.set_opacity(clamped_opacity)
            .map_err(|e| format!("Failed to set opacity: {}", e))?;
        Ok(())
    } else {
        Err("PiP window not found".to_string())
    }
}
```

**JavaScript (`pip.html`):**
```javascript
function initOpacityControl() {
    const slider = document.getElementById('opacitySlider');
    const savedOpacity = localStorage.getItem('pipOpacity');
    
    slider.addEventListener('input', (e) => {
        const value = e.target.value;
        setOpacity(parseInt(value) / 100);
        localStorage.setItem('pipOpacity', value);
    });
}
```

### Toggle Behavior Fix

**Before (Broken):**
```rust
// Used hide/show - lost Windows flags
window.hide()  // ❌ Loses WDA_EXCLUDEFROMCAPTURE
window.show()  // ❌ Loses WS_EX_NOACTIVATE
```

**After (Fixed):**
```rust
// Close and recreate - preserves all flags
window.close()  // ✅ Properly destroys window
open_pip_window()  // ✅ Recreates with all flags
```

### Borderless Window

**Window Builder:**
```rust
WebviewWindowBuilder::new(&app, "pip", url)
    .decorations(false)  // Borderless
    .always_on_top(true)
    .focused(false)      // No focus stealing
    .skip_taskbar(true)
```

### Centered Positioning

**Position Calculation:**
```rust
let window_x = (monitor_width - window_width) / 2.0;
let window_y = (monitor_height - window_height) / 2.0;
```

## Behavior

### Before Fix:
- Click PiP → Browser loses focus and blurs
- Need to click back to browser to continue working
- Interrupts workflow

### After Fix:
- Click PiP → Browser stays focused
- No blur effect on browser
- Seamless multitasking
- Can scroll PiP content while typing in browser

## Platform Support

- ✅ **Windows**: Fully supported via WS_EX_NOACTIVATE
- ⚠️ **macOS**: Not implemented (macOS handles focus differently)
- ⚠️ **Linux**: Not implemented

## Limitations

1. **Title Bar Interaction**: Clicking the title bar or window borders may still activate the window (Windows behavior)
2. **Dragging**: Dragging the window will temporarily activate it
3. **Content Area**: Clicking the content area (chat messages) won't steal focus

## Testing Checklist

### Transparency
- [ ] Slider moves smoothly from 10% to 100%
- [ ] Window opacity changes in real-time
- [ ] Setting persists after closing and reopening
- [ ] No visual glitches during opacity changes

### Toggle Behavior
- [ ] First Ctrl+Alt+P: Window opens centered
- [ ] Second Ctrl+Alt+P: Window closes completely
- [ ] Third Ctrl+Alt+P: Window reopens with screen capture exclusion working
- [ ] Third Ctrl+Alt+P: Window reopens with no focus stealing working
- [ ] No black screen on reopen

### Drag and Move
- [ ] Can drag window by green title bar
- [ ] Window moves smoothly without lag
- [ ] Can drop window anywhere on screen
- [ ] Dragging doesn't cause focus issues

### Window Controls
- [ ] Minimize button (−) works
- [ ] Close button (×) works
- [ ] Can resize by dragging edges
- [ ] Window stays borderless

### Screen Capture Exclusion
- [ ] Window invisible in OBS
- [ ] Window invisible in Zoom screen share
- [ ] Window invisible in GMeet screen share
- [ ] Works after toggle (close and reopen)

### No Focus Stealing
- [ ] Clicking PiP doesn't blur browser
- [ ] Can type in browser while PiP is visible
- [ ] Can scroll PiP content without losing browser focus
- [ ] Works after toggle (close and reopen)

## Console Output

When PiP window opens, you'll see:
```
[PiP] ✓ WS_EX_NOACTIVATE applied - window won't steal focus
[PiP] You can click on the PiP window without losing focus on your browser
```

## Related Features

- **Screen Capture Exclusion**: PiP is invisible to screen recording
- **Always On Top**: PiP stays above all windows
- **Draggable**: Can be moved anywhere on screen
- **Resizable**: Can be resized as needed

## Future Enhancements

Potential improvements:
- macOS support using NSPanel or similar
- Linux support using X11/Wayland properties
- Optional click-through mode (completely transparent to clicks)
- Configurable focus behavior


## Platform Support

- ✅ **Windows 10/11**: Fully supported
  - Screen capture exclusion via `WDA_EXCLUDEFROMCAPTURE`
  - No focus stealing via `WS_EX_NOACTIVATE`
  - Transparency via `set_opacity()`
- ⚠️ **macOS**: Partial support (screen capture exclusion only)
- ⚠️ **Linux**: Not implemented

## Known Limitations

1. **Minimum Opacity**: Set to 10% to prevent fully invisible window
2. **Drag Handle Required**: Must use title bar to drag (not content area)
3. **Platform Specific**: Some features Windows-only

## Console Output

When PiP window opens:
```
[PiP] Creating PiP window at (960, 540) with size 400x350
[PiP] Borderless window created with custom drag handle
[PiP] ✓ Screen capture exclusion applied (SetWindowDisplayAffinity)
[PiP] Window is now invisible to: OBS, Zoom, GMeet, Teams, etc.
[PiP] ✓ WS_EX_NOACTIVATE applied - window won't steal focus
[PiP] You can click on the PiP window without losing focus on your browser
```

When opacity changes:
```
[PiP] Opacity set to 0.50
```

When toggling:
```
[PiP] Window closed (toggle off)
[PiP] Creating new window (toggle on)
```

## Files Modified

### Rust Files
- `src-tauri/src/pip.rs`: Added `set_pip_opacity()` command, fixed `toggle_pip_window()`, centered positioning
- `src-tauri/src/lib.rs`: Registered `set_pip_opacity` command

### HTML/JavaScript Files
- `static/pip.html`: Added opacity slider UI, fixed drag functionality, added close button in status bar

## Summary

The PiP window now provides a complete, polished experience:
- ✅ Borderless modern design
- ✅ Smooth transparency control (10-100%)
- ✅ Proper toggle behavior (no flag loss)
- ✅ Centered positioning
- ✅ Working drag handle
- ✅ Visible close button
- ✅ Screen capture exclusion maintained
- ✅ No focus stealing maintained
