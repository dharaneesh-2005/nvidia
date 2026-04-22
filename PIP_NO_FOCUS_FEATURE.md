# PiP Window - No Focus Stealing Feature

## Overview
The PiP (Picture-in-Picture) window has been enhanced to prevent focus stealing. When you click on the PiP window, your browser or other active application will remain focused and won't blur.

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

## Usage

1. Open PiP window (click 📺 PiP button or press Ctrl+Alt+P)
2. Position it anywhere on screen
3. Click on PiP to scroll or read - your browser stays focused!
4. Continue working in your browser without interruption

## Technical Implementation

### Code Changes

**File: `src-tauri/src/pip.rs`**

```rust
// Added .focused(false) to window builder
.focused(false)  // Don't steal focus when created

// Applied WS_EX_NOACTIVATE after window creation
apply_no_activate_windows(&pip_window)?;
```

**New Function:**
```rust
fn apply_no_activate_windows(window: &tauri::WebviewWindow) -> Result<(), String> {
    // Gets current window extended styles
    // Adds WS_EX_NOACTIVATE flag
    // Prevents window activation on click
}
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

## Testing

To verify the feature works:

1. Open your browser (Chrome, Edge, etc.)
2. Open the PiP window
3. Click in your browser address bar or text field
4. Click on the PiP window content area
5. ✅ Browser should remain focused (no blur)
6. ✅ You should still be able to type in browser

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
