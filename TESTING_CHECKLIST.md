# Testing Checklist - SEB Stealth Mode

## Pre-Deployment Testing

### Build Verification

- [ ] Rust backend compiles without errors
  ```batch
  cargo build --release
  ```

- [ ] DLL injection components compile without errors
  ```batch
  cd dll-injection
  build.bat
  ```

- [ ] All files present in `deploy` folder:
  - [ ] `nvidia.exe`
  - [ ] `config.json`
  - [ ] `profile.json`
  - [ ] `start_stealth.bat`
  - [ ] `dll-injection/injector.exe`
  - [ ] `dll-injection/payload.dll`
  - [ ] `static/` folder with all files

- [ ] `deploy.zip` created successfully

---

## Functional Testing (Without SEB)

### Backend Server

- [ ] Backend starts without errors
  ```batch
  nvidia.exe
  ```

- [ ] Server listens on port 5000
  ```batch
  netstat -ano | findstr :5000
  ```

- [ ] Web UI accessible at `http://localhost:5000`

- [ ] WebSocket connection established (check browser console)

### Voice Capture

- [ ] Microphone detected and initialized
- [ ] Voice activity detection working
- [ ] Transcription successful (speak a test question)
- [ ] Answer received from AI
- [ ] Conversation history maintained

### Screen Capture

- [ ] Hotkey `Ctrl+Alt+X` triggers capture
- [ ] Screenshot analyzed successfully
- [ ] Problem type detected correctly (DSA/System Design/etc.)
- [ ] Solution provided by AI
- [ ] Multi-capture mode works (`Ctrl+Alt+C`)
- [ ] Cancel multi-capture works (`Ctrl+Alt+Z`)

### Manual Search

- [ ] Hotkey `Ctrl+Alt+S` opens modal
- [ ] Can type question
- [ ] Enter submits question
- [ ] Answer received
- [ ] Modal closes properly

### Debug Mode

- [ ] Hotkey `Ctrl+Alt+D` triggers debug capture
- [ ] Error screenshot analyzed
- [ ] Fix suggestions provided

### PiP Mode

- [ ] PiP button works
- [ ] PiP window opens
- [ ] Content syncs with main window
- [ ] PiP window stays on top

---

## Stealth Testing (Without SEB)

### Window Affinity

- [ ] Run backend
- [ ] Check window affinity:
  ```powershell
  Add-Type @"
  using System;
  using System.Runtime.InteropServices;
  public class WinAPI {
      [DllImport("user32.dll")]
      public static extern uint GetWindowDisplayAffinity(IntPtr hwnd, out uint affinity);
  }
  "@
  
  Get-Process nvidia | ForEach-Object {
      $affinity = 0
      [WinAPI]::GetWindowDisplayAffinity($_.MainWindowHandle, [ref]$affinity)
      Write-Host "Affinity: $affinity (0x11 = stealth)"
  }
  ```

- [ ] Affinity should be `0x11` (WDA_EXCLUDEFROMCAPTURE)

### Screenshot Test

- [ ] Open UI at `http://localhost:5000`
- [ ] Take screenshot with Snipping Tool
- [ ] **Expected**: UI should NOT appear in screenshot
- [ ] Take screenshot with Win+Shift+S
- [ ] **Expected**: UI should NOT appear in screenshot
- [ ] Take screenshot with PrintScreen
- [ ] **Expected**: UI should NOT appear in screenshot

---

## DLL Injection Testing (With SEB)

### Pre-Injection

- [ ] SEB installed and configured
- [ ] SEB starts without errors
- [ ] SEB browser accessible

### Injection Process

- [ ] Run `injector.exe` as Administrator
- [ ] Injection completes without errors
- [ ] Check injection log:
  ```batch
  type C:\temp\seb_injection.log
  ```

- [ ] Log shows:
  - [ ] "Payload DLL loaded into SEB"
  - [ ] "Spawning interview helper"
  - [ ] "Interview helper spawned successfully"
  - [ ] "Monitor thread started"
  - [ ] "Applied stealth to window"

### Post-Injection

- [ ] `nvidia.exe` process running
  ```batch
  tasklist | findstr nvidia
  ```

- [ ] Backend accessible at `http://localhost:5000` in SEB browser

- [ ] All features work within SEB:
  - [ ] Voice capture
  - [ ] Screen capture
  - [ ] Manual search
  - [ ] Debug mode

### Stealth Verification in SEB

- [ ] UI visible on physical screen
- [ ] UI NOT visible in SEB's monitoring screenshots
- [ ] UI NOT visible in screen recordings
- [ ] UI NOT visible in Alt+Tab
- [ ] UI NOT visible in Task Manager window list

---

## Automated Startup Testing

### start_stealth.bat

- [ ] Script detects admin privileges
- [ ] Script checks for components
- [ ] Script builds missing components
- [ ] Script detects SEB if running
- [ ] Script injects automatically if SEB found
- [ ] Script starts normally if SEB not found
- [ ] Script provides clear status messages

### Windows Startup

- [ ] Add to startup registry
  ```batch
  reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "NvidiaHelper" /t REG_SZ /d "C:\path\to\start_stealth.bat" /f
  ```

- [ ] Restart computer
- [ ] Script runs automatically on login
- [ ] Backend starts successfully
- [ ] No visible windows or prompts

---

## Performance Testing

### Response Times

- [ ] Voice transcription: < 2 seconds
- [ ] Screen analysis: < 3 seconds
- [ ] Coding solutions: < 5 seconds
- [ ] Conversational Q&A: < 2 seconds

### Resource Usage

- [ ] CPU usage: < 5% idle, < 20% active
- [ ] Memory usage: < 200MB
- [ ] Network usage: Minimal when idle
- [ ] Disk usage: No excessive writes

### Stability

- [ ] Run for 1 hour continuously
- [ ] No crashes or errors
- [ ] No memory leaks
- [ ] WebSocket stays connected
- [ ] Stealth maintained throughout

---

## Security Testing

### Process Visibility

- [ ] `nvidia.exe` not in SEB's process blocklist
- [ ] Process appears as child of SEB (if spawned from injection)
- [ ] Process not terminated by SEB

### Network Filtering

- [ ] Localhost connections allowed
- [ ] API calls to Groq successful
- [ ] WebSocket connections stable
- [ ] No network errors in logs

### File System Access

- [ ] Can read `config.json`
- [ ] Can read `profile.json`
- [ ] Can write logs to `C:\temp\`
- [ ] Can access `static/` files

---

## Edge Cases

### Multiple SEB Instances

- [ ] Injection works with multiple SEB processes
- [ ] Each instance gets its own helper
- [ ] No conflicts or crashes

### SEB Restart

- [ ] SEB closes and reopens
- [ ] Helper continues running
- [ ] Stealth maintained
- [ ] Reconnection successful

### Network Interruption

- [ ] Disconnect internet
- [ ] Backend continues running
- [ ] Reconnect internet
- [ ] API calls resume successfully

### Long Sessions

- [ ] Run for 3+ hours
- [ ] No performance degradation
- [ ] No memory leaks
- [ ] Stealth maintained

---

## Compatibility Testing

### Windows Versions

- [ ] Windows 10 2004 (build 19041)
- [ ] Windows 10 21H1
- [ ] Windows 10 21H2
- [ ] Windows 11 21H2
- [ ] Windows 11 22H2

### SEB Versions

- [ ] SEB 3.0
- [ ] SEB 3.1
- [ ] SEB 3.2
- [ ] SEB 3.3
- [ ] SEB 3.4 (latest)

### Browsers (for PiP)

- [ ] Chrome 116+
- [ ] Edge 116+
- [ ] Firefox (if supported)

---

## Failure Scenarios

### Injection Fails

- [ ] Error message displayed
- [ ] Log file created with error details
- [ ] Graceful fallback to normal mode
- [ ] User notified of failure

### Backend Crashes

- [ ] Crash logged to file
- [ ] Automatic restart attempted
- [ ] User notified if restart fails

### API Errors

- [ ] Retry logic works (3 attempts)
- [ ] Timeout handling works
- [ ] Error message displayed to user
- [ ] Conversation history preserved

### Stealth Breaks

- [ ] Monitor thread detects and reapplies
- [ ] Stealth restored within 1 second
- [ ] No visible flicker or glitch

---

## Cleanup Testing

### Temporary Files

- [ ] Logs created in correct location
- [ ] Logs don't grow excessively
- [ ] Old logs cleaned up automatically

### Process Termination

- [ ] `taskkill /IM nvidia.exe /F` works
- [ ] All child processes terminated
- [ ] No zombie processes left
- [ ] Ports released properly

### Complete Removal

- [ ] Delete `deploy` folder
- [ ] Remove startup registry entry
- [ ] Clear logs
- [ ] No traces left on system

---

## Documentation Testing

### README Accuracy

- [ ] All commands work as documented
- [ ] All file paths correct
- [ ] All screenshots up-to-date
- [ ] All links working

### Quick Start Guide

- [ ] Can complete setup in 5 minutes
- [ ] All steps clear and accurate
- [ ] No missing prerequisites
- [ ] Expected results match actual

### Troubleshooting Guide

- [ ] All common issues covered
- [ ] Solutions work as described
- [ ] Log locations correct
- [ ] Commands accurate

---

## Final Checklist

### Pre-Deployment

- [ ] All tests passed
- [ ] No critical bugs
- [ ] Performance acceptable
- [ ] Documentation complete
- [ ] Legal disclaimer included

### Deployment Package

- [ ] `deploy.zip` created
- [ ] All files included
- [ ] File sizes reasonable
- [ ] No debug symbols included
- [ ] No sensitive data included

### User Readiness

- [ ] User guide provided
- [ ] Quick start guide provided
- [ ] Troubleshooting guide provided
- [ ] Support contact provided
- [ ] Legal warnings acknowledged

---

## Sign-Off

**Tester Name**: ___________________  
**Date**: ___________________  
**Version Tested**: ___________________  
**Test Environment**: ___________________  
**Overall Status**: ☐ PASS  ☐ FAIL  ☐ CONDITIONAL PASS  

**Notes**:
```
[Add any additional notes, observations, or concerns here]
```

**Approval**: ☐ Approved for deployment  ☐ Requires fixes  

**Approver**: ___________________  
**Date**: ___________________  

---

## Continuous Testing

### Daily Checks

- [ ] Backend starts successfully
- [ ] API keys valid
- [ ] Network connectivity
- [ ] Logs reviewed

### Weekly Checks

- [ ] Full functional test
- [ ] Performance benchmarks
- [ ] Security scan
- [ ] Dependency updates

### Monthly Checks

- [ ] Compatibility testing
- [ ] Penetration testing
- [ ] Code review
- [ ] Documentation update

---

**Document Version**: 1.0  
**Last Updated**: 2024  
**Next Review**: [Date]
