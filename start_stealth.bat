@echo off
title Nvidia Interview Helper - Stealth Mode
color 0A

echo.
echo ========================================
echo   NVIDIA INTERVIEW HELPER
echo   Stealth Mode for SEB
echo ========================================
echo.

REM Check if running as admin
net session >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [!] This script requires Administrator privileges
    echo [!] Right-click and select "Run as Administrator"
    echo.
    pause
    exit /b 1
)

echo [*] Checking components...

REM Check if backend exists
if not exist "nvidia.exe" (
    if exist "target\release\nvidia.exe" (
        echo [*] Copying backend from target\release...
        copy /Y "target\release\nvidia.exe" "nvidia.exe" >nul
    ) else (
        echo [!] Backend not found. Building...
        cargo build --release
        if %ERRORLEVEL% NEQ 0 (
            echo [!] Build failed
            pause
            exit /b 1
        )
        copy /Y "target\release\nvidia.exe" "nvidia.exe" >nul
    )
)

REM Check if DLL injection components exist
if not exist "dll-injection\injector.exe" (
    echo [!] DLL injection components not found
    echo [*] Building injection tools...
    cd dll-injection
    call build.bat
    cd ..
)

echo [+] All components ready
echo.

REM Check if SEB is running
tasklist /FI "IMAGENAME eq SafeExamBrowser.exe" 2>NUL | find /I /N "SafeExamBrowser.exe">NUL
if %ERRORLEVEL% EQU 0 (
    echo [+] Safe Exam Browser detected
    echo [*] Injecting into SEB process...
    cd dll-injection
    injector.exe
    cd ..
    echo.
    echo [+] Injection complete!
    echo [*] Interview helper is now running invisibly
    echo [*] Open browser to: http://localhost:5000
) else (
    echo [!] Safe Exam Browser not detected
    echo [*] Starting in normal mode...
    start /B nvidia.exe
    timeout /t 2 /nobreak >nul
    echo [+] Backend started
    echo [*] Open browser to: http://localhost:5000
)

echo.
echo ========================================
echo   READY
echo ========================================
echo.
echo Press any key to keep this window open...
echo (Closing this window will NOT stop the helper)
pause >nul
