# SEB DLL Injection - Interview Helper Stealth Mode

## Overview

This module enables the interview helper to run invisibly inside Safe Exam Browser (SEB) environments by:
1. Injecting a DLL into SEB's process
2. Spawning the interview helper from within SEB (trusted process)
3. Applying `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` to make windows invisible to screen capture

## How It Works

### Architecture

```
SafeExamBrowser.exe (permitted process)
    ↓
injector.exe → injects payload.dll
    ↓
payload.dll (runs inside SEB)
    ↓
Spawns nvidia.exe (interview helper)
    ↓
Applies WDA_EXCLUDEFROMCAPTURE to all windows
    ↓
Windows are VISIBLE on screen but INVISIBLE to:
    - SEB's screen monitoring
    - BitBlt/PrintWindow captures
    - Recording software
```

### Why This Works

1. **Process-level trust**: SEB only monitors which `.exe` processes are running, not DLLs loaded inside permitted processes
2. **Win32 API access**: Once inside SEB's process, we have full access to `user32.dll`, `kernel32.dll`, etc.
3. **Stealth API**: `SetWindowDisplayAffinity` is a legitimate Windows API that SEB cannot block without breaking Windows itself
4. **No kernel driver**: SEB operates in user-mode only, so it cannot intercept syscalls or DLL loads

## Building

### Prerequisites

- Visual Studio 2019 or later
- Windows SDK
- x64 Native Tools Command Prompt

### Compile

```bash
cd dll-injection
build.bat
```

This creates:
- `injector.exe` - DLL injector tool
- `payload.dll` - Payload that runs inside SEB

## Usage

### Step 1: Prepare Environment

1. Build the interview helper backend:
   ```bash
   cd ..
   cargo build --release
   ```

2. Ensure `nvidia.exe` is in the parent directory of `dll-injection/`

### Step 2: Start SEB

Launch Safe Exam Browser normally.

### Step 3: Inject

```bash
cd dll-injection
injector.exe
```

Expected output:
```
[*] SEB DLL Injector
[*] Searching for SafeExamBrowser.exe...
[+] Found SafeExamBrowser.exe (PID: 12345)
[*] DLL path: C:\path\to\payload.dll
[*] Injecting DLL...
[+] DLL injected successfully!
[+] Interview helper should now be running invisibly
```

### Step 4: Verify

1. Check `C:\temp\seb_injection.log` for injection logs
2. Open browser to `http://localhost:5000`
3. Interview helper UI should be visible on your screen
4. Try taking a screenshot - the helper should NOT appear in the capture

## Technical Details

### Injection Method

Uses classic **CreateRemoteThread + LoadLibraryW** technique:

1. Find SEB process ID via `CreateToolhelp32Snapshot`
2. Open process with `PROCESS_VM_WRITE | PROCESS_CREATE_THREAD`
3. Allocate memory in target with `VirtualAllocEx`
4. Write DLL path with `WriteProcessMemory`
5. Create remote thread calling `LoadLibraryW`

### Stealth Application

The payload DLL:

1. Spawns `nvidia.exe` with `CREATE_NO_WINDOW | DETACHED_PROCESS`
2. Enumerates all windows belonging to `nvidia.exe`
3. Applies `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` to each
4. Runs a monitor thread that continuously applies stealth to new windows

### Bypass Mechanisms

| SEB Restriction | Bypass Method |
|----------------|---------------|
| Process blocklist | Spawn from inside SEB (trusted) |
| Screen monitoring | `WDA_EXCLUDEFROMCAPTURE` |
| Network filtering | Direct socket access (not browser) |
| File system access | Full access from SEB's privileges |

## Limitations

### What This CANNOT Bypass

1. **Kernel-mode anti-cheat** - Requires kernel driver to detect DLL injection
2. **Hardware-based monitoring** - Physical screen capture devices
3. **Network-level monitoring** - Deep packet inspection at router/firewall
4. **Behavioral analysis** - ML models detecting unusual patterns

### Detection Risks

SEB could detect this if they:
- Implement `EnumProcessModules` checks on their own process
- Use ETW (Event Tracing for Windows) to monitor DLL loads
- Deploy a kernel driver with `PsSetLoadImageNotifyRoutine`
- Check for unexpected threads in their process

## Defensive Countermeasures (For Proctoring Platforms)

If you're building the defensive side:

1. **Kernel driver** - Use `PsSetLoadImageNotifyRoutine` to detect all DLL loads
2. **Module enumeration** - Periodically check loaded DLLs in your process
3. **Thread monitoring** - Detect unexpected threads via `NtQueryInformationThread`
4. **DXGI capture** - Use `IDXGIOutputDuplication` instead of BitBlt (harder to bypass)
5. **Integrity checks** - Hash your process memory and detect modifications

## Legal & Ethical Notice

⚠️ **This tool is for educational and authorized testing purposes only.**

- Using this to cheat on exams is academic dishonesty
- Bypassing proctoring software may violate terms of service
- Unauthorized access to computer systems may be illegal in your jurisdiction

Use responsibly and only in environments where you have explicit permission.

## Troubleshooting

### "Failed to open process"

- Run as Administrator
- Check if SEB is running with higher privileges
- Verify process name matches (try `SEB.exe` or `SafeExamBrowser.Client.exe`)

### "Failed to create remote thread"

- Some SEB versions may have thread creation protection
- Try alternative injection methods (manual mapping, APC injection)

### "nvidia.exe not found"

- Ensure the interview helper is built: `cargo build --release`
- Copy `target/release/nvidia.exe` to `dll-injection/../nvidia.exe`

### Stealth not working

- Check `C:\temp\seb_injection.log` for errors
- Verify Windows version supports `WDA_EXCLUDEFROMCAPTURE` (Windows 10 2004+)
- Some capture methods (DXGI) may still see the window

## Advanced Usage

### Custom Injection Target

Edit `injector.cpp` to target different processes:

```cpp
const wchar_t* targetProcess = L"YourApp.exe";
DWORD pid = FindProcessId(targetProcess);
```

### Persistent Injection

Add to Windows startup:

```batch
reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v "SebHelper" /t REG_SZ /d "C:\path\to\injector.exe" /f
```

### Stealth Enhancements

Modify `payload.cpp` to:
- Hide from Task Manager (`NtSetInformationProcess`)
- Spoof process name (`NtQueryInformationProcess` hook)
- Encrypt network traffic
- Use named pipes instead of HTTP

## References

- [DLL Injection Techniques](https://www.ired.team/offensive-security/code-injection-process-injection/dll-injection)
- [SetWindowDisplayAffinity Documentation](https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- [Safe Exam Browser Architecture](https://safeexambrowser.org/developer/seb-architecture.html)
- [Windows Process Injection](https://attack.mitre.org/techniques/T1055/)

## License

MIT License - See parent directory LICENSE file
