# Keyboard Shortcuts

## Screen Capture Hotkeys

| Hotkey | Function | Description |
|--------|----------|-------------|
| **Ctrl+Alt+X** | Finalize Capture | Capture screen and send for DSA problem analysis |
| **Ctrl+Shift+X** | Finalize Capture (Alt) | Alternative hotkey for finalizing capture |
| **Ctrl+Alt+C** | Multi-Capture | Capture screen chunk for multi-part questions |
| **Ctrl+Alt+Z** | Cancel Multi-Capture | Clear multi-capture buffer |
| **Ctrl+Alt+Q** | MCQ Capture | Capture and analyze Multiple Choice Questions |

## Other Hotkeys

| Hotkey | Function | Description |
|--------|----------|-------------|
| **Ctrl+Alt+S** | Manual Search | Open manual question input modal |
| **Ctrl+Alt+P** | Toggle PiP | Open/close Picture-in-Picture window |
| **Ctrl+Alt+D** | Debug Code | Capture screen for code error analysis |

## Usage Notes

### DSA Problem Capture (Ctrl+Alt+X)
- Use for coding problems (LeetCode, HackerRank style)
- Supports multi-capture with Ctrl+Alt+C
- Provides detailed solutions with multiple approaches
- Adds to conversation history

### MCQ Capture (Ctrl+Alt+Q)
- Use for Multiple Choice Questions
- Automatically detects code vs theory questions
- Uses code_interpreter for code execution
- Uses browser_search for fact verification
- Does NOT add to conversation history

### Multi-Capture Workflow
1. Press **Ctrl+Alt+C** to capture first part
2. Press **Ctrl+Alt+C** again for additional parts
3. Press **Ctrl+Alt+X** to finalize and analyze all parts
4. Press **Ctrl+Alt+Z** to cancel and clear buffer

### Debug Code (Ctrl+Alt+D)
- Captures screen showing code errors
- Analyzes compilation/runtime errors
- Provides fix suggestions
- Shows corrected code

## Tips

- All hotkeys work globally (even when app is in background)
- Hotkeys are registered on app startup
- If a hotkey doesn't work, another app may be using it
- Check console logs for hotkey registration status
