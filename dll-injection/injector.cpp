// DLL Injector - Injects payload.dll into SafeExamBrowser.exe
// Compile: cl /LD /O2 injector.cpp /link /OUT:injector.exe user32.lib kernel32.lib advapi32.lib

#include <windows.h>
#include <tlhelp32.h>
#include <stdio.h>
#include <string>

// Find process by name
DWORD FindProcessId(const wchar_t* processName) {
    PROCESSENTRY32W pe32;
    pe32.dwSize = sizeof(PROCESSENTRY32W);
    
    HANDLE hSnapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnapshot == INVALID_HANDLE_VALUE) return 0;
    
    DWORD pid = 0;
    if (Process32FirstW(hSnapshot, &pe32)) {
        do {
            if (_wcsicmp(pe32.szExeFile, processName) == 0) {
                pid = pe32.th32ProcessID;
                break;
            }
        } while (Process32NextW(hSnapshot, &pe32));
    }
    
    CloseHandle(hSnapshot);
    return pid;
}

// Inject DLL into target process
bool InjectDLL(DWORD processId, const wchar_t* dllPath) {
    // Open target process
    HANDLE hProcess = OpenProcess(
        PROCESS_CREATE_THREAD | PROCESS_QUERY_INFORMATION | 
        PROCESS_VM_OPERATION | PROCESS_VM_WRITE | PROCESS_VM_READ,
        FALSE, processId
    );
    
    if (!hProcess) {
        printf("[-] Failed to open process. Error: %lu\n", GetLastError());
        return false;
    }
    
    // Allocate memory in target process
    size_t dllPathSize = (wcslen(dllPath) + 1) * sizeof(wchar_t);
    LPVOID pRemoteDllPath = VirtualAllocEx(hProcess, NULL, dllPathSize, 
                                           MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    
    if (!pRemoteDllPath) {
        printf("[-] Failed to allocate memory. Error: %lu\n", GetLastError());
        CloseHandle(hProcess);
        return false;
    }
    
    // Write DLL path to target process
    if (!WriteProcessMemory(hProcess, pRemoteDllPath, dllPath, dllPathSize, NULL)) {
        printf("[-] Failed to write memory. Error: %lu\n", GetLastError());
        VirtualFreeEx(hProcess, pRemoteDllPath, 0, MEM_RELEASE);
        CloseHandle(hProcess);
        return false;
    }
    
    // Get LoadLibraryW address
    HMODULE hKernel32 = GetModuleHandleW(L"kernel32.dll");
    LPTHREAD_START_ROUTINE pLoadLibraryW = (LPTHREAD_START_ROUTINE)GetProcAddress(hKernel32, "LoadLibraryW");
    
    if (!pLoadLibraryW) {
        printf("[-] Failed to get LoadLibraryW address\n");
        VirtualFreeEx(hProcess, pRemoteDllPath, 0, MEM_RELEASE);
        CloseHandle(hProcess);
        return false;
    }
    
    // Create remote thread to load DLL
    HANDLE hThread = CreateRemoteThread(hProcess, NULL, 0, pLoadLibraryW, 
                                       pRemoteDllPath, 0, NULL);
    
    if (!hThread) {
        printf("[-] Failed to create remote thread. Error: %lu\n", GetLastError());
        VirtualFreeEx(hProcess, pRemoteDllPath, 0, MEM_RELEASE);
        CloseHandle(hProcess);
        return false;
    }
    
    // Wait for thread to complete
    WaitForSingleObject(hThread, INFINITE);
    
    // Cleanup
    CloseHandle(hThread);
    VirtualFreeEx(hProcess, pRemoteDllPath, 0, MEM_RELEASE);
    CloseHandle(hProcess);
    
    return true;
}

int wmain(int argc, wchar_t* argv[]) {
    printf("[*] SEB DLL Injector\n");
    printf("[*] Waiting for SafeExamBrowser.exe to start...\n");
    printf("[*] (Press Ctrl+C to cancel)\n\n");
    
    // Try multiple possible SEB process names
    const wchar_t* sebNames[] = {
        L"SafeExamBrowser.exe",
        L"SEB.exe",
        L"SafeExamBrowser.Client.exe"
    };
    
    DWORD pid = 0;
    int attempts = 0;
    
    // Keep trying for up to 5 minutes
    while (attempts < 300) {
        for (int i = 0; i < 3; i++) {
            pid = FindProcessId(sebNames[i]);
            if (pid) {
                printf("[+] Found %ls (PID: %lu)\n", sebNames[i], pid);
                goto found;
            }
        }
        
        if (attempts % 10 == 0) {
            printf("[*] Still waiting... (%d seconds)\n", attempts);
        }
        
        Sleep(1000);
        attempts++;
    }
    
    printf("[-] Timeout: SafeExamBrowser not found after 5 minutes\n");
    return 1;
    
found:
    
    // Get DLL path (same directory as injector)
    wchar_t dllPath[MAX_PATH];
    GetModuleFileNameW(NULL, dllPath, MAX_PATH);
    wchar_t* lastSlash = wcsrchr(dllPath, L'\\');
    if (lastSlash) {
        wcscpy_s(lastSlash + 1, MAX_PATH - (lastSlash - dllPath + 1), L"payload.dll");
    }
    
    printf("[*] DLL path: %ls\n", dllPath);
    
    // Check if DLL exists
    if (GetFileAttributesW(dllPath) == INVALID_FILE_ATTRIBUTES) {
        printf("[-] payload.dll not found!\n");
        return 1;
    }
    
    printf("[*] Injecting DLL...\n");
    if (InjectDLL(pid, dllPath)) {
        printf("[+] DLL injected successfully!\n");
        printf("[+] Interview helper should now be running invisibly\n");
        return 0;
    } else {
        printf("[-] Injection failed\n");
        return 1;
    }
}
