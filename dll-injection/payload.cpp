// Payload DLL - Runs inside SEB process
// Spawns interview helper with stealth mode
// Compile: cl /LD /O2 payload.cpp /link /OUT:payload.dll user32.lib kernel32.lib advapi32.lib

#include <windows.h>
#include <tlhelp32.h>
#include <stdio.h>
#include <string>

// Log to file (since we can't use console inside SEB)
void Log(const char* message) {
    FILE* f = fopen("C:\\temp\\seb_injection.log", "a");
    if (f) {
        fprintf(f, "[%lu] %s\n", GetTickCount(), message);
        fclose(f);
    }
}

// Spawn interview helper with stealth
bool SpawnInterviewHelper() {
    Log("Spawning interview helper backend...");
    
    // Get DLL directory
    wchar_t dllPath[MAX_PATH];
    GetModuleFileNameW(GetModuleHandleW(L"payload.dll"), dllPath, MAX_PATH);
    wchar_t* lastSlash = wcsrchr(dllPath, L'\\');
    if (lastSlash) *lastSlash = 0;
    
    // Build path to nvidia.exe (backend)
    wchar_t backendPath[MAX_PATH];
    swprintf_s(backendPath, L"%s\\..\\nvidia.exe", dllPath);
    
    Log("Backend path constructed");
    
    // Create backend process with CREATE_NO_WINDOW flag
    STARTUPINFOW si = { sizeof(si) };
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE;
    
    PROCESS_INFORMATION pi = { 0 };
    
    if (!CreateProcessW(
        backendPath,
        NULL,
        NULL,
        NULL,
        FALSE,
        CREATE_NO_WINDOW | DETACHED_PROCESS,
        NULL,
        NULL,
        &si,
        &pi
    )) {
        Log("Failed to create backend process");
        return false;
    }
    
    Log("Backend spawned successfully");
    
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
    
    // Wait for backend to start
    Sleep(2000);
    
    // Now spawn Tauri PiP window
    Log("Spawning Tauri PiP window...");
    
    wchar_t tauriPath[MAX_PATH];
    swprintf_s(tauriPath, L"%s\\..\\src-tauri\\target\\release\\nvidia-tauri.exe", dllPath);
    
    // Check if Tauri app exists
    if (GetFileAttributesW(tauriPath) == INVALID_FILE_ATTRIBUTES) {
        Log("Tauri PiP not found, skipping");
        return true; // Not critical, continue
    }
    
    STARTUPINFOW siTauri = { sizeof(siTauri) };
    siTauri.dwFlags = STARTF_USESHOWWINDOW;
    siTauri.wShowWindow = SW_SHOW; // Show the PiP window
    
    PROCESS_INFORMATION piTauri = { 0 };
    
    if (!CreateProcessW(
        tauriPath,
        NULL,
        NULL,
        NULL,
        FALSE,
        0, // Normal process, visible
        NULL,
        NULL,
        &siTauri,
        &piTauri
    )) {
        Log("Failed to create Tauri PiP process (non-critical)");
        return true; // Not critical
    }
    
    Log("Tauri PiP spawned successfully");
    
    CloseHandle(piTauri.hThread);
    CloseHandle(piTauri.hProcess);
    
    return true;
}

// Apply stealth to all windows of our interview helper
BOOL CALLBACK EnumWindowsProc(HWND hwnd, LPARAM lParam) {
    DWORD pid = 0;
    GetWindowThreadProcessId(hwnd, &pid);
    
    // Check if this window belongs to our target process
    if (pid == (DWORD)lParam) {
        // Apply WDA_EXCLUDEFROMCAPTURE
        if (SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)) {
            char buf[256];
            sprintf_s(buf, "Applied stealth to window HWND: 0x%p", hwnd);
            Log(buf);
        }
        
        // Also make it topmost
        SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, 
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
    
    return TRUE;
}

// Find process by name
DWORD FindProcess(const wchar_t* name) {
    HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnapshot == INVALID_HANDLE_VALUE) return 0;
    
    PROCESSENTRY32W pe32;
    pe32.dwSize = sizeof(PROCESSENTRY32W);
    
    DWORD pid = 0;
    if (Process32FirstW(hSnapshot, &pe32)) {
        do {
            if (_wcsicmp(pe32.szExeFile, name) == 0) {
                pid = pe32.th32ProcessID;
                break;
            }
        } while (Process32NextW(hSnapshot, &pe32));
    }
    
    CloseHandle(hSnapshot);
    return pid;
}

// Hotkey listener thread for Ctrl+Alt+P
DWORD WINAPI HotkeyThread(LPVOID param) {
    Log("Hotkey thread started - listening for Ctrl+Alt+P");
    
    // Get DLL directory for Tauri path
    wchar_t dllPath[MAX_PATH];
    GetModuleFileNameW(GetModuleHandleW(L"payload.dll"), dllPath, MAX_PATH);
    wchar_t* lastSlash = wcsrchr(dllPath, L'\\');
    if (lastSlash) *lastSlash = 0;
    
    wchar_t tauriPath[MAX_PATH];
    swprintf_s(tauriPath, L"%s\\..\\src-tauri\\target\\release\\nvidia-tauri.exe", dllPath);
    
    // Register Ctrl+Alt+P hotkey
    const int HOTKEY_ID = 100;
    if (!RegisterHotKey(NULL, HOTKEY_ID, MOD_CONTROL | MOD_ALT, 'P')) {
        Log("Failed to register Ctrl+Alt+P hotkey");
        return 1;
    }
    
    Log("Ctrl+Alt+P hotkey registered successfully");
    
    MSG msg = { 0 };
    while (GetMessageW(&msg, NULL, 0, 0)) {
        if (msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID) {
            Log("Ctrl+Alt+P pressed - toggling PiP window");
            
            // Check if Tauri PiP is already running
            DWORD pid = FindProcess(L"nvidia-tauri.exe");
            
            if (pid) {
                // Already running, close it
                Log("PiP already running, closing...");
                HANDLE hProcess = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
                if (hProcess) {
                    TerminateProcess(hProcess, 0);
                    CloseHandle(hProcess);
                    Log("PiP closed");
                }
            } else {
                // Not running, start it
                Log("Starting PiP window...");
                
                STARTUPINFOW si = { sizeof(si) };
                si.dwFlags = STARTF_USESHOWWINDOW;
                si.wShowWindow = SW_SHOW;
                
                PROCESS_INFORMATION pi = { 0 };
                
                if (CreateProcessW(
                    tauriPath,
                    NULL,
                    NULL,
                    NULL,
                    FALSE,
                    0,
                    NULL,
                    NULL,
                    &si,
                    &pi
                )) {
                    Log("PiP window opened");
                    CloseHandle(pi.hThread);
                    CloseHandle(pi.hProcess);
                } else {
                    Log("Failed to open PiP window");
                }
            }
        }
        
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    
    UnregisterHotKey(NULL, HOTKEY_ID);
    return 0;
}

// Monitor thread - continuously applies stealth to new windows
DWORD WINAPI MonitorThread(LPVOID param) {
    Log("Monitor thread started");
    
    // Wait for interview helper to start
    Sleep(3000);
    
    // Track multiple processes
    const wchar_t* targetProcesses[] = {
        L"nvidia.exe",           // Main backend
        L"nvidia-tauri.exe",     // PiP window
        L"chrome.exe",           // If using Chrome for PiP
        L"msedge.exe"            // If using Edge for PiP
    };
    
    // Continuously apply stealth to all windows
    while (true) {
        for (int i = 0; i < 4; i++) {
            DWORD pid = FindProcess(targetProcesses[i]);
            if (pid) {
                EnumWindows(EnumWindowsProc, (LPARAM)pid);
            }
        }
        Sleep(1000); // Check every second
    }
    
    return 0;
}

// DLL entry point
BOOL APIENTRY DllMain(HMODULE hModule, DWORD dwReason, LPVOID lpReserved) {
    if (dwReason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(hModule);
        
        Log("=== Payload DLL loaded into SEB ===");
        
        // Create directory for logs
        CreateDirectoryA("C:\\temp", NULL);
        
        // Spawn interview helper (backend + PiP)
        if (SpawnInterviewHelper()) {
            // Start monitor thread
            HANDLE hMonitorThread = CreateThread(NULL, 0, MonitorThread, NULL, 0, NULL);
            if (hMonitorThread) {
                CloseHandle(hMonitorThread);
                Log("Monitor thread created");
            }
            
            // Start hotkey listener thread
            HANDLE hHotkeyThread = CreateThread(NULL, 0, HotkeyThread, NULL, 0, NULL);
            if (hHotkeyThread) {
                CloseHandle(hHotkeyThread);
                Log("Hotkey thread created");
            }
        }
    }
    
    return TRUE;
}
