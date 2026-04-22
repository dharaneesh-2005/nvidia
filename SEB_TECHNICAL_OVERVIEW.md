# Technical Overview: SEB Bypass Architecture

## Executive Summary

This document explains how the interview helper achieves complete invisibility within Safe Exam Browser (SEB) environments through DLL injection and Windows API manipulation.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Safe Exam Browser                         │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  SafeExamBrowser.exe (Permitted Process)               │ │
│  │  ┌──────────────────────────────────────────────────┐  │ │
│  │  │  payload.dll (Injected)                          │  │ │
│  │  │  ├─ Spawns nvidia.exe (Interview Helper)         │  │ │
│  │  │  ├─ Applies SetWindowDisplayAffinity             │  │ │
│  │  │  └─ Monitors & maintains stealth                 │  │ │
│  │  └──────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│              nvidia.exe (Interview Helper)                   │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Axum Web Server (localhost:5000)                      │ │
│  │  ├─ WebSocket for real-time communication             │ │
│  │  ├─ WASAPI Loopback (system audio capture)            │ │
│  │  ├─ Screen capture (Ctrl+Alt+X)                       │ │
│  │  └─ Groq API integration (AI models)                  │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  Windows: INVISIBLE to screen capture (WDA_EXCLUDEFROMCAPTURE)│
│           VISIBLE on physical display                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Bypass Mechanisms

### 1. DLL Injection

**Technique**: Classic CreateRemoteThread + LoadLibraryW

**Process**:
```cpp
1. Find SEB process ID via CreateToolhelp32Snapshot
2. Open process with PROCESS_VM_WRITE | PROCESS_CREATE_THREAD
3. Allocate memory in target: VirtualAllocEx
4. Write DLL path: WriteProcessMemory
5. Create remote thread: CreateRemoteThread(LoadLibraryW)
6. DLL loads and executes DllMain in SEB's context
```

**Why it works**:
- SEB only monitors `.exe` processes, not DLLs
- Once inside SEB's process, we inherit its permissions
- SEB cannot block DLL loads without breaking Windows

### 2. Process Spawning from Trusted Context

**Technique**: CreateProcess from within SEB

**Code**:
```cpp
STARTUPINFOW si = { sizeof(si) };
si.dwFlags = STARTF_USESHOWWINDOW;
si.wShowWindow = SW_HIDE;

CreateProcessW(
    L"nvidia.exe",
    NULL,
    NULL,
    NULL,
    FALSE,
    CREATE_NO_WINDOW | DETACHED_PROCESS,
    NULL,
    NULL,
    &si,
    &pi
);
```

**Why it works**:
- Process spawned from SEB appears as child of trusted process
- SEB's process monitor sees: `SafeExamBrowser.exe → nvidia.exe`
- Looks like legitimate subprocess, not external threat

### 3. Screen Capture Invisibility

**Technique**: SetWindowDisplayAffinity with WDA_EXCLUDEFROMCAPTURE

**Code**:
```cpp
HWND hwnd = FindWindow(NULL, L"Nvidia");
SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
```

**Effect**:
- Window is **physically visible** on monitor
- Window is **invisible** to:
  - BitBlt (used by most screen capture tools)
  - PrintWindow
  - Windows.Graphics.Capture API
  - OBS, Zoom, Teams screen sharing
  - SEB's monitoring screenshots

**Why it works**:
- `WDA_EXCLUDEFROMCAPTURE` is a legitimate Windows API
- SEB cannot block it without breaking Windows itself
- Operates at DWM (Desktop Window Manager) level
- No kernel driver needed

### 4. Continuous Stealth Maintenance

**Technique**: Monitor thread that continuously applies stealth

**Code**:
```cpp
DWORD WINAPI MonitorThread(LPVOID param) {
    while (true) {
        EnumWindows(EnumWindowsProc, targetPid);
        Sleep(1000);
    }
}

BOOL CALLBACK EnumWindowsProc(HWND hwnd, LPARAM lParam) {
    DWORD pid;
    GetWindowThreadProcessId(hwnd, &pid);
    if (pid == targetPid) {
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        SetWindowPos(hwnd, HWND_TOPMOST, ...);
    }
    return TRUE;
}
```

**Why it's needed**:
- New windows created after injection need stealth applied
- PiP window, modal dialogs, etc.
- Ensures persistent invisibility

---

## SEB's Limitations

### What SEB Can Block

✅ Unauthorized `.exe` processes (via process list monitoring)  
✅ Specific URLs in built-in browser (URL whitelist/blacklist)  
✅ Alt+Tab, Task Manager, Windows key (keyboard hooks)  
✅ External browsers (process termination)  

### What SEB Cannot Block

❌ **DLLs loaded into permitted processes** - No visibility  
❌ **Win32 API calls** - Always available (user32.dll, kernel32.dll)  
❌ **SetWindowDisplayAffinity** - Legitimate Windows API  
❌ **WASAPI loopback** - System-level audio capture  
❌ **Network sockets** - Direct socket access bypasses browser filter  
❌ **File system access** - Full access from SEB's privileges  

### Why SEB Cannot Detect This

1. **No kernel driver**: SEB operates entirely in user-mode
   - Cannot intercept syscalls (NtCreateSection, NtMapViewOfSection)
   - Cannot use kernel callbacks (PsSetLoadImageNotifyRoutine)
   - Cannot monitor DLL loads at kernel level

2. **Process-level monitoring only**: 
   - Checks running `.exe` files
   - Does not enumerate loaded DLLs in permitted processes
   - Does not monitor threads within processes

3. **No integrity checking**:
   - Does not hash process memory
   - Does not detect code injection
   - Does not monitor for unexpected threads

4. **API surface always available**:
   - `user32.dll`, `kernel32.dll`, `ntdll.dll` always loaded
   - Cannot block without breaking Windows
   - All Win32 APIs accessible from injected DLL

---

## Attack Surface Analysis

### Entry Points

1. **DLL Injection** (Primary)
   - CreateRemoteThread + LoadLibraryW
   - Manual mapping (more advanced)
   - APC injection
   - Thread hijacking

2. **Process Hollowing**
   - Create suspended SEB process
   - Unmap original code
   - Map malicious code
   - Resume execution

3. **COM Hijacking**
   - Register malicious COM object
   - SEB loads it automatically
   - Execute in SEB's context

### Persistence Mechanisms

1. **Windows Startup**
   ```batch
   reg add "HKCU\...\Run" /v "Helper" /d "injector.exe"
   ```

2. **Scheduled Task**
   ```batch
   schtasks /create /tn "NvidiaUpdate" /tr "injector.exe" /sc onlogon
   ```

3. **DLL Search Order Hijacking**
   - Place malicious DLL in SEB's directory
   - SEB loads it before system DLLs

---

## Defensive Countermeasures

### For Proctoring Platforms

To actually detect and prevent this bypass:

#### 1. Kernel Driver (Most Effective)

```cpp
// Kernel driver with image load notification
NTSTATUS DriverEntry(PDRIVER_OBJECT DriverObject, PUNICODE_STRING RegistryPath) {
    PsSetLoadImageNotifyRoutine(ImageLoadCallback);
    return STATUS_SUCCESS;
}

void ImageLoadCallback(
    PUNICODE_STRING FullImageName,
    HANDLE ProcessId,
    PIMAGE_INFO ImageInfo
) {
    // Check if DLL is being loaded into SEB process
    if (IsSEBProcess(ProcessId)) {
        if (!IsWhitelistedDLL(FullImageName)) {
            // Block the load or alert
            ImageInfo->ImageBase = NULL;
        }
    }
}
```

**Pros**: Catches all DLL injections at kernel level  
**Cons**: Requires driver signing, complex development

#### 2. Module Enumeration (User-Mode)

```cpp
// Periodically check loaded modules
HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, GetCurrentProcessId());
MODULE32W me32;
me32.dwSize = sizeof(MODULE32W);

if (Module32FirstW(hSnapshot, &me32)) {
    do {
        if (!IsWhitelistedModule(me32.szModule)) {
            // Unexpected DLL detected
            AlertAndTerminate();
        }
    } while (Module32NextW(hSnapshot, &me32));
}
```

**Pros**: Simple to implement  
**Cons**: Can be bypassed with timing, only detects after load

#### 3. Thread Monitoring

```cpp
// Detect unexpected threads
HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
THREADENTRY32 te32;
te32.dwSize = sizeof(THREADENTRY32);

if (Thread32First(hSnapshot, &te32)) {
    do {
        if (te32.th32OwnerProcessID == GetCurrentProcessId()) {
            // Check if thread start address is in known modules
            if (!IsKnownThreadStartAddress(te32.th32ThreadID)) {
                // Injected thread detected
                AlertAndTerminate();
            }
        }
    } while (Thread32Next(hSnapshot, &te32));
}
```

**Pros**: Detects CreateRemoteThread injection  
**Cons**: Can be bypassed with thread hijacking

#### 4. DXGI Desktop Duplication (Capture Bypass)

```cpp
// Use DXGI instead of BitBlt
IDXGIOutputDuplication* pDuplication;
pOutput->DuplicateOutput(pDevice, &pDuplication);

DXGI_OUTDUPL_FRAME_INFO frameInfo;
IDXGIResource* pResource;
pDuplication->AcquireNextFrame(INFINITE, &frameInfo, &pResource);
```

**Pros**: Harder to bypass than BitBlt  
**Cons**: Still can be bypassed with DWM manipulation

#### 5. ETW (Event Tracing for Windows)

```cpp
// Monitor DLL loads via ETW
EVENT_TRACE_PROPERTIES properties = {0};
properties.Wnode.BufferSize = sizeof(EVENT_TRACE_PROPERTIES);
properties.LoggerNameOffset = sizeof(EVENT_TRACE_PROPERTIES);
properties.LogFileMode = EVENT_TRACE_REAL_TIME_MODE;

StartTraceW(&sessionHandle, L"MySession", &properties);
EnableTraceEx2(sessionHandle, &ImageLoadGuid, ...);
```

**Pros**: Kernel-level visibility without driver  
**Cons**: Requires admin, can be disabled

---

## Advanced Evasion Techniques

### 1. Reflective DLL Injection

Load DLL from memory without touching disk:

```cpp
// Map DLL into memory manually
LPVOID pRemoteCode = VirtualAllocEx(hProcess, NULL, dllSize, MEM_COMMIT, PAGE_EXECUTE_READWRITE);
WriteProcessMemory(hProcess, pRemoteCode, dllData, dllSize, NULL);

// Manually resolve imports and relocations
// Call DllMain directly
```

**Advantage**: No file on disk, harder to detect

### 2. Process Doppelgänging

```cpp
// Create transaction
HANDLE hTransaction = CreateTransaction(...);

// Create file in transaction
HANDLE hFile = CreateFileTransacted(L"legitimate.exe", ..., hTransaction);

// Write malicious code
WriteFile(hFile, maliciousCode, ...);

// Create section from transacted file
NtCreateSection(&hSection, ..., hFile);

// Rollback transaction (file disappears)
RollbackTransaction(hTransaction);

// Create process from section
NtCreateProcessEx(&hProcess, ..., hSection);
```

**Advantage**: Bypasses most AV/EDR solutions

### 3. Parent Process Spoofing

```cpp
// Make nvidia.exe appear as child of explorer.exe
STARTUPINFOEXW si = {0};
SIZE_T size;
InitializeProcThreadAttributeList(NULL, 1, 0, &size);
si.lpAttributeList = (LPPROC_THREAD_ATTRIBUTE_LIST)malloc(size);
InitializeProcThreadAttributeList(si.lpAttributeList, 1, 0, &size);

HANDLE hExplorer = OpenProcess(PROCESS_CREATE_PROCESS, FALSE, explorerPid);
UpdateProcThreadAttribute(si.lpAttributeList, 0, 
                         PROC_THREAD_ATTRIBUTE_PARENT_PROCESS,
                         &hExplorer, sizeof(HANDLE), NULL, NULL);

CreateProcessW(L"nvidia.exe", ..., &si.StartupInfo, &pi);
```

**Advantage**: Process tree looks legitimate

---

## Performance Considerations

### Injection Overhead

- **DLL load time**: ~50-100ms
- **Process spawn time**: ~200-500ms
- **Stealth application**: ~10ms per window
- **Total startup**: ~1-2 seconds

### Runtime Overhead

- **Monitor thread**: ~0.1% CPU (checks every 1 second)
- **Memory**: ~5MB for injected DLL
- **Network**: No overhead (direct sockets)

### Optimization Tips

1. **Reduce monitor frequency**: Check every 2-3 seconds instead of 1
2. **Lazy stealth application**: Only apply when window becomes visible
3. **Batch window enumeration**: Process multiple windows per cycle

---

## Legal & Ethical Considerations

### Academic Integrity

Using this tool to cheat on exams violates:
- University honor codes
- Academic integrity policies
- Terms of service agreements

**Consequences**:
- Expulsion from institution
- Degree revocation
- Permanent academic record

### Legal Risks

Depending on jurisdiction, this may violate:
- Computer Fraud and Abuse Act (CFAA) - USA
- Computer Misuse Act - UK
- Criminal Code provisions - Canada
- Similar laws in other countries

**Potential charges**:
- Unauthorized access to computer systems
- Circumvention of technological protection measures
- Wire fraud (if monetary gain involved)

### Authorized Use Cases

✅ **Legitimate uses**:
- Security research with permission
- Penetration testing for proctoring companies
- Educational demonstrations in controlled environments
- Developing defensive countermeasures

---

## Conclusion

This bypass works because:

1. **SEB operates in user-mode** - No kernel driver to detect DLL injection
2. **Process-level monitoring only** - Cannot see DLLs or threads within processes
3. **Win32 API always available** - Cannot block without breaking Windows
4. **No integrity checking** - Does not detect code injection or memory modification

To defend against this, proctoring platforms need:

1. **Kernel driver** with image load notifications
2. **Module enumeration** to detect unexpected DLLs
3. **Thread monitoring** to detect injected threads
4. **DXGI capture** instead of BitBlt
5. **Behavioral analysis** to detect unusual patterns

---

## References

- [DLL Injection Techniques](https://www.ired.team/offensive-security/code-injection-process-injection/dll-injection)
- [SetWindowDisplayAffinity](https://docs.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- [Process Injection](https://attack.mitre.org/techniques/T1055/)
- [Windows Internals](https://docs.microsoft.com/en-us/sysinternals/resources/windows-internals)
- [Safe Exam Browser Architecture](https://safeexambrowser.org/developer/)

---

**Document Version**: 1.0  
**Last Updated**: 2024  
**Classification**: Technical Documentation
