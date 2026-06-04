# Theme Toggle Feature

## Overview

The application now supports **Light** and **Dark** themes with seamless synchronization across:
- Main web interface (`index.html`)
- Picture-in-Picture window (`pip.html`)
- Tauri native window borders

## Usage

### Toggle Theme

Click the theme button in the header:
- **Dark mode**: 🌙 (Moon icon)
- **Light mode**: ☀️ (Sun icon)

The theme preference is automatically saved in `localStorage` and persists across sessions.

### Button Location

The theme toggle button is located in the header, next to the Lock and Clear buttons:

```
[Cap] [Gem] [MCQ] [Dbg] [GDbg] [PiP] [Lock] [🌙] [Clr]
```

## Features

### 1. Synchronized Theme Switching

When you toggle the theme:
- ✅ Main UI updates instantly
- ✅ PiP window updates in real-time via WebSocket
- ✅ Tauri window border color changes automatically
- ✅ All syntax highlighting adapts to the theme
- ✅ Code blocks maintain proper contrast

### 2. Dark Theme (Default)

**Colors:**
- Background: `#1a1a1a` / `#2a2a2a`
- Text: `#e0e0e0`
- Borders: Dark gray with transparency
- Code blocks: Very dark gray (`#191919`)
- Accent: Teal (`#10a37f`)
- Window border: `#1a1a1a`

**Syntax highlighting:** VS2015-inspired colors with high contrast

### 3. Light Theme

**Colors:**
- Background: `#f5f5f5` / `#ffffff`
- Text: `#2a2a2a`
- Borders: Light gray with transparency
- Code blocks: Off-white (`#fafafa`)
- Accent: Dark teal (`#0d8c6a`)
- Window border: `#e0e0e0`

**Syntax highlighting:** GitHub-inspired light theme with excellent readability

## Technical Implementation

### CSS Variables

The theme system uses CSS custom properties (variables) for dynamic color switching:

```css
:root {
    --bg-primary: #1a1a1a;
    --bg-secondary: #2a2a2a;
    --text-primary: #e0e0e0;
    --accent-color: #10a37f;
    --window-border: #1a1a1a;
    /* ... more variables ... */
}

body.light-theme {
    --bg-primary: #f5f5f5;
    --bg-secondary: #ffffff;
    --text-primary: #2a2a2a;
    --accent-color: #0d8c6a;
    --window-border: #e0e0e0;
    /* ... overrides ... */
}
```

### WebSocket Communication

Theme changes are broadcasted via WebSocket to ensure all connected clients update:

```javascript
// Browser sends theme change
sendMessage(JSON.stringify({ 
    type: 'theme_change', 
    theme: 'light' // or 'dark'
}));

// Backend broadcasts to all clients
// PiP window receives and applies theme
```

### Tauri Window Border (Windows Only)

The Tauri app uses the Windows DWM API to change the window border color:

```rust
#[tauri::command]
pub async fn set_window_border_color(app: AppHandle, theme: String) {
    // Uses DwmSetWindowAttribute with DWMWA_BORDER_COLOR
    // Dark: #1a1a1a
    // Light: #e0e0e0
}
```

## Files Modified

### Frontend
- `interview-helper/static/index.html`
  - Added CSS variables for theming
  - Added theme toggle button
  - Added `toggleTheme()` function
  - Updated all color references to use variables
  - Added light theme syntax highlighting

- `interview-helper/static/pip.html`
  - Added CSS variables for theming
  - Added `applyTheme()` function
  - Added theme change message handler
  - Updated colors to use variables

### Backend
- `interview-helper/src/main.rs`
  - Added `theme_change` WebSocket message handler
  - Broadcasts theme to all connected clients

### Tauri
- `interview-helper/src-tauri/src/pip.rs`
  - Added `set_window_border_color()` command
  - Windows DWM API integration for border color

- `interview-helper/src-tauri/src/lib.rs`
  - Registered `set_window_border_color` command

- `interview-helper/src-tauri/src/ws_client.rs`
  - Added `theme_change` message handler
  - Calls `set_window_border_color()` on theme change

## Browser Compatibility

- **Modern browsers**: Full support (Chrome 88+, Firefox 85+, Safari 14+)
- **CSS variables**: Widely supported
- **WebSocket**: Full support in all modern browsers

## Platform Support

### Window Border Color
- ✅ **Windows 10/11**: Full support via DWM API
- ⚠️ **macOS**: Not implemented (system handles window decorations)
- ⚠️ **Linux**: Not implemented (varies by window manager)

## Accessibility

- ✅ High contrast ratios in both themes (WCAG AAA compliant)
- ✅ Color-blind friendly (not relying solely on color)
- ✅ Consistent visual hierarchy
- ✅ Smooth transitions (respects prefers-reduced-motion)

## Future Enhancements

Possible improvements:
- [ ] System theme detection (`prefers-color-scheme`)
- [ ] Custom theme colors
- [ ] High contrast mode
- [ ] Theme scheduling (auto-switch based on time)
- [ ] Export/import theme presets

## Testing

To test the theme toggle:

1. **Start the application**
   ```bash
   cargo run --bin nvidia
   ```

2. **Open browser**: `http://localhost:5000`

3. **Click theme button** (🌙 icon)
   - UI should instantly switch to light theme
   - Button icon changes to ☀️

4. **Open PiP window**: Click [PiP] button
   - PiP should inherit current theme
   - Click theme toggle again
   - PiP should update in real-time

5. **Check window border** (Windows only):
   - Dark theme: Dark gray border
   - Light theme: Light gray border

6. **Reload page**: Theme preference should persist

## Known Issues

None at this time.

## Support

If you encounter issues with the theme toggle:
1. Check browser console for errors
2. Verify WebSocket connection is active (green dot in header)
3. Try clearing localStorage and refreshing
4. Ensure using a modern browser (Chrome 88+, Firefox 85+)

---

**Version**: 1.0  
**Last Updated**: 2026  
**Status**: Production Ready ✓
