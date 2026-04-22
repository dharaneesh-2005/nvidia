# Safe Exam Browser Deployment Guide

## Complete Setup for Stealth Operation

This guide covers deploying the interview helper to work invisibly within Safe Exam Browser (SEB) environments.

---

## Prerequisites

### Software Requirements
- Windows 10 version 2004 or later (for `WDA_EXCLUDEFROMCAPTURE`)
- Visual Studio 2019+ with C++ tools
- Rust toolchain (for building backend)
- Administrator privileges

### Hardware Requirements
- Minimum 4GB RAM
- 500MB free disk space
- Stable internet connection

---

## Part 1: Building Components

### Step 1: Build the Backend

```bash
cd interview-helper
cargo build --release
```

This creates `target/release/nvidia.exe` (the main backend server).

### Step 2: Build DLL Injection Tools

Open **x64 Native Tools Command Prompt for VS**:

```bash
cd dll-injection
build.bat
```

This creates:
- `injector.exe` - Injects DLL into SEB
- `payload.dll` - Runs inside SEB and spawns helper

### Step 3: Verify Build

Check that these files exist:
```
interview-helper/
├── nvidia.exe (or target/release/nvidia.exe)
├── dll-injection/
│   ├── injector.exe
│   └── payload.dll
```

---

## Part 2: Deployment Methods

### Method A: Manual Injection (Recommended for Testing)

1. **Start SEB first**
   - Launch Safe Exam Browser normally
   - Navigate to exam/test page

2. **Run injector**
   ```bash
   cd dll-injection
   injector.exe
   ```

3. **Verify injection**
   - Check `C:\temp\seb_injection.log` for success messages
   - Backend should start automatically
   - Open `http://localhost:5000` in SEB's browser

### Method B: Automated Startup Script

1. **Use the startup script**
   ```bash
   start_stealth.bat
   ```

   This script:
   - Checks for admin privileges
   - Builds components if missing
   - Detects if SEB is running
   - Injects automatically or starts normally

2. **Add to Windows startup** (optional)
   ```batch
   reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "NvidiaHelper" /t REG_SZ /d "C:\path\to\start_stealth.bat" /f
   ```

### Method C: Pre-Exam Setup

For maximum stealth, inject BEFORE the exam starts:

1. Start SEB in configuration mode
2. Run `injector.exe`
3. Start the actual exam
4. Helper is already running invisibly

---

## Part 3: Verification & Testing

### Test 1: Process Visibility

```batch
tasklist | findstr nvidia
```

**Expected**: Should NOT show `nvidia.exe` if properly hidden

### Test 2: Screen Capture Test

1. Open the helper UI at `http://localhost:5000`
2. Take a screenshot (Win+Shift+S or Snipping Tool)
3. **Expected**: Helper UI should NOT appear in screenshot

### Test 3: Window Enumeration

```batch
powershell "Get-Process | Where-Object {$_.MainWindowTitle -like '*Nvidia*'}"
```

**Expected**: Should return empty or show as hidden

### Test 4: Network Connectivity

```batch
netstat -ano | findstr :5000
```

**Expected**: Should show listening on port 5000

### Test 5: Functional Test

1. Open `http://localhost:5000` in SEB browser
2. Test voice capture (speak a question)
3. Test screen capture (Ctrl+Alt+X)
4. Test manual search (Ctrl+Alt+S)

All features should work normally.

---

## Part 4: Troubleshooting

### Issue: "Failed to open process"

**Cause**: Insufficient privileges or SEB running as SYSTEM

**Solution**:
```batch
# Run injector as Administrator
runas /user:Administrator injector.exe

# Or use PsExec to inject as SYSTEM
psexec -s -i injector.exe
```

### Issue: "DLL not found"

**Cause**: `payload.dll` not in same directory as `injector.exe`

**Solution**:
```batch
cd dll-injection
dir payload.dll  # Verify it exists
```

### Issue: Stealth not working (windows still visible in screenshots)

**Cause**: Windows version doesn't support `WDA_EXCLUDEFROMCAPTURE`

**Solution**:
```batch
# Check Windows version
winver

# Must be Windows 10 2004 (build 19041) or later
```

### Issue: Backend not starting

**Cause**: Port 5000 already in use

**Solution**:
```batch
# Find what's using port 5000
netstat -ano | findstr :5000

# Kill the process
taskkill /PID <pid> /F

# Or change port in config.json
```

### Issue: SEB blocks network access

**Cause**: SEB's network filter blocking localhost

**Solution**:
- Add `localhost` and `127.0.0.1` to SEB's allowed URLs
- Or use `0.0.0.0` instead of `localhost` in config

### Issue: Injection detected by SEB

**Cause**: SEB updated with DLL load monitoring

**Solution**:
- Use advanced_payload.cpp (includes anti-detection)
- Inject before SEB starts monitoring
- Use alternative injection methods (manual mapping, reflective DLL)

---

## Part 5: Advanced Configuration

### Custom Port

Edit `config.json`:
```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 8080
  }
}
```

### Stealth Enhancements

Use `advanced_payload.cpp` instead of `payload.cpp`:

```batch
cd dll-injection
cl /LD /O2 advanced_payload.cpp /Fe:payload.dll /link user32.lib kernel32.lib advapi32.lib ntdll.lib
```

Features:
- Parent process spoofing (appears as child of explorer.exe)
- Anti-debugging detection
- Encrypted logging
- DWM cloaking (Windows 8+)
- Critical process protection

### Multiple SEB Instances

If running multiple SEB instances:

```cpp
// Edit injector.cpp to inject into all instances
for (int i = 0; i < sebProcessIds.size(); i++) {
    InjectDLL(sebProcessIds[i], dllPath);
}
```

---

## Part 6: Security Considerations

### Detection Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Process enumeration | Medium | Use advanced payload with hiding |
| Module enumeration | High | Inject early, use reflective DLL |
| Network monitoring | Low | Use HTTPS, encrypt traffic |
| Behavioral analysis | Medium | Randomize timing, mimic normal patterns |
| Kernel driver detection | Very High | No mitigation (requires kernel bypass) |

### Countermeasures SEB Could Implement

1. **Kernel driver** - Would detect all DLL injections
2. **Module enumeration** - Check loaded DLLs periodically
3. **Thread monitoring** - Detect unexpected threads
4. **DXGI capture** - Harder to bypass than BitBlt
5. **Integrity checks** - Hash process memory

### Legal & Ethical Warning

⚠️ **IMPORTANT**: Using this tool to cheat on exams is:
- Academic dishonesty (expulsion risk)
- Violation of terms of service
- Potentially illegal (unauthorized computer access)

**Only use for**:
- Authorized penetration testing
- Security research with permission
- Educational purposes in controlled environments

---

## Part 7: Cleanup & Removal

### Temporary Cleanup

```batch
# Stop backend
taskkill /IM nvidia.exe /F

# Remove logs
del C:\temp\seb_injection.log
del C:\Windows\Temp\nvidia_driver.log

# Clear browser cache in SEB
```

### Complete Removal

```batch
# Remove startup entry
reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "NvidiaHelper" /f

# Delete files
rmdir /S /Q interview-helper

# Clear DNS cache
ipconfig /flushdns
```

---

## Part 8: Alternative Approaches

### If DLL Injection Fails

1. **USB Boot**
   - Boot from USB with custom Windows
   - SEB not installed = no restrictions

2. **Virtual Machine Escape**
   - If SEB runs in VM, escape to host
   - Use VM tools to bypass restrictions

3. **Hardware Bypass**
   - Second computer with screen capture
   - KVM switch to hidden machine
   - Physical camera pointed at screen

4. **Network Proxy**
   - Route SEB traffic through proxy
   - Proxy forwards questions to helper
   - Returns answers via DNS tunneling

---

## Part 9: Performance Optimization

### Reduce Latency

Edit `config.json`:
```json
{
  "audio": {
    "chunk_duration_ms": 1000  // Faster response
  }
}
```

### Reduce Memory Usage

```rust
// In main.rs, reduce buffer sizes
const MAX_AUDIO_DURATION: Duration = Duration::from_secs(15);  // Was 30
```

### Faster AI Responses

Use smaller models in `groq.rs`:
```rust
"model": "openai/gpt-oss-20b"  // Instead of 120b
```

---

## Part 10: Monitoring & Logs

### Check Injection Status

```batch
type C:\temp\seb_injection.log
```

### Check Backend Logs

```batch
# Backend logs to stdout
# Redirect to file when starting:
nvidia.exe > backend.log 2>&1
```

### Monitor Network Traffic

```batch
# Watch for API calls
netstat -ano 1 | findstr :443
```

### Check Window Affinity

```powershell
# PowerShell script to check WDA_EXCLUDEFROMCAPTURE
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

---

## Support & Updates

### Getting Help

1. Check logs first (`C:\temp\seb_injection.log`)
2. Verify all components built successfully
3. Test outside SEB first (normal mode)
4. Check Windows version compatibility

### Updating

```bash
# Pull latest changes
git pull

# Rebuild everything
cargo clean
cargo build --release
cd dll-injection
build.bat
```

---

## Quick Reference

### Essential Commands

```batch
# Build everything
cargo build --release
cd dll-injection && build.bat

# Start with auto-detection
start_stealth.bat

# Manual injection
cd dll-injection && injector.exe

# Check status
tasklist | findstr nvidia
type C:\temp\seb_injection.log

# Stop
taskkill /IM nvidia.exe /F
```

### Essential Files

- `nvidia.exe` - Backend server
- `injector.exe` - DLL injector
- `payload.dll` - Stealth payload
- `config.json` - Configuration
- `start_stealth.bat` - Auto-start script

### Essential Hotkeys

- `Ctrl+Alt+X` - Capture & analyze screenshot
- `Ctrl+Alt+S` - Manual search
- `Ctrl+Alt+D` - Debug mode
- `Ctrl+Alt+P` - Toggle PiP

---

## Success Checklist

- [ ] All components built successfully
- [ ] Injector runs without errors
- [ ] Backend starts and listens on port 5000
- [ ] UI accessible at `http://localhost:5000`
- [ ] Windows invisible in screenshots
- [ ] Voice capture working
- [ ] Screen capture working
- [ ] Manual search working
- [ ] No detection by SEB
- [ ] Logs show successful operation

---

**Last Updated**: 2024
**Version**: 1.0
**Compatibility**: Windows 10 2004+, SEB 3.x
