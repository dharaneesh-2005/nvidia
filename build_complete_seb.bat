@echo off
title Building Interview Helper - SEB Stealth Edition
color 0B

echo.
echo ========================================
echo   NVIDIA INTERVIEW HELPER
echo   Complete Build for SEB Environment
echo ========================================
echo.

REM Check for admin
net session >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [!] Administrator privileges required
    echo [!] Right-click and select "Run as Administrator"
    pause
    exit /b 1
)

echo [*] Step 1/4: Checking prerequisites...
echo.

REM Check Rust
where cargo >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [!] Rust not found. Install from: https://rustup.rs/
    pause
    exit /b 1
)
echo [+] Rust found

REM Check Visual Studio
where cl.exe >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [!] Visual Studio C++ compiler not found
    echo [!] Please run from "x64 Native Tools Command Prompt for VS"
    pause
    exit /b 1
)
echo [+] Visual Studio found

echo.
echo [*] Step 2/4: Building Rust backend...
echo.

cargo build --release
if %ERRORLEVEL% NEQ 0 (
    echo [!] Backend build failed
    pause
    exit /b 1
)

echo [+] Backend built successfully
copy /Y "target\release\nvidia.exe" "nvidia.exe" >nul
echo [+] Copied nvidia.exe to root directory

echo.
echo [*] Step 3/4: Building DLL injection components...
echo.

cd dll-injection

REM Build basic payload
echo [*] Building basic payload.dll...
cl /nologo /LD /O2 /EHsc payload.cpp /Fe:payload.dll /link user32.lib kernel32.lib advapi32.lib
if %ERRORLEVEL% NEQ 0 (
    echo [!] Basic payload build failed
    cd ..
    pause
    exit /b 1
)
echo [+] Basic payload built

REM Build advanced payload
echo [*] Building advanced_payload.dll...
cl /nologo /LD /O2 /EHsc advanced_payload.cpp /Fe:advanced_payload.dll /link user32.lib kernel32.lib advapi32.lib ntdll.lib
if %ERRORLEVEL% NEQ 0 (
    echo [!] Advanced payload build failed (non-critical)
) else (
    echo [+] Advanced payload built
)

REM Build injector
echo [*] Building injector.exe...
cl /nologo /O2 /EHsc injector.cpp /Fe:injector.exe /link user32.lib kernel32.lib advapi32.lib
if %ERRORLEVEL% NEQ 0 (
    echo [!] Injector build failed
    cd ..
    pause
    exit /b 1
)
echo [+] Injector built

REM Cleanup
del *.obj *.exp *.lib 2>nul

cd ..

echo.
echo [*] Step 4/4: Creating deployment package...
echo.

REM Create deployment directory
if not exist "deploy" mkdir deploy
if not exist "deploy\dll-injection" mkdir deploy\dll-injection

REM Copy files
copy /Y "nvidia.exe" "deploy\" >nul
copy /Y "config.json" "deploy\" >nul
copy /Y "profile.json" "deploy\" >nul
copy /Y "start_stealth.bat" "deploy\" >nul
copy /Y "dll-injection\injector.exe" "deploy\dll-injection\" >nul
copy /Y "dll-injection\payload.dll" "deploy\dll-injection\" >nul
if exist "dll-injection\advanced_payload.dll" (
    copy /Y "dll-injection\advanced_payload.dll" "deploy\dll-injection\" >nul
)
copy /Y "SEB_DEPLOYMENT_GUIDE.md" "deploy\" >nul

REM Copy static files
xcopy /E /I /Y "static" "deploy\static" >nul

echo [+] Deployment package created in 'deploy' directory

echo.
echo ========================================
echo   BUILD COMPLETE!
echo ========================================
echo.
echo Files created:
echo   - deploy\nvidia.exe           (Backend server)
echo   - deploy\dll-injection\injector.exe  (DLL injector)
echo   - deploy\dll-injection\payload.dll   (Basic stealth)
if exist "deploy\dll-injection\advanced_payload.dll" (
    echo   - deploy\dll-injection\advanced_payload.dll (Advanced stealth)
)
echo   - deploy\start_stealth.bat    (Auto-start script)
echo   - deploy\static\              (Web UI files)
echo.
echo Quick Start:
echo   1. Copy 'deploy' folder to target machine
echo   2. Start Safe Exam Browser
echo   3. Run: deploy\start_stealth.bat
echo   4. Open: http://localhost:5000
echo.
echo For detailed instructions, see:
echo   deploy\SEB_DEPLOYMENT_GUIDE.md
echo.
echo ========================================
echo.

REM Create portable ZIP
echo [*] Creating portable ZIP package...
if exist "deploy.zip" del "deploy.zip"
powershell -command "Compress-Archive -Path 'deploy\*' -DestinationPath 'deploy.zip' -Force"
if %ERRORLEVEL% EQU 0 (
    echo [+] Created deploy.zip (portable package)
    echo.
)

echo Press any key to exit...
pause >nul
