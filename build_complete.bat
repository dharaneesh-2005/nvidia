@echo off
title Complete Build Process
cd /d "%~dp0"

echo ========================================
echo   Nvidia - Complete Build Process
echo ========================================
echo.
echo This script will:
echo   1. Build the main Rust application (nvidia.exe)
echo   2. Build the Tauri PiP application (nvidia-tauri.exe)
echo   3. Compile the installer
echo.
pause

echo.
echo [1/3] Building main Rust application...
echo ========================================
echo.

cargo build --release --bin nvidia

if errorlevel 1 (
    echo.
    echo Error: Main Rust build failed!
    echo Please check the error messages above.
    pause
    exit /b 1
)

echo.
echo Main Rust build completed successfully!
echo.

echo [2/3] Building Tauri PiP application...
echo ========================================
echo.

cd src-tauri
cargo build --release --package nvidia-tauri --bin nvidia-tauri
set BUILD_ERROR=%ERRORLEVEL%
cd ..

if %BUILD_ERROR% neq 0 (
    echo.
    echo Error: Tauri build failed!
    echo Please check the error messages above.
    pause
    exit /b 1
)

REM Check if executable was created (Tauri builds to workspace target)
if exist "target\release\nvidia-tauri.exe" (
    echo Tauri executable found at: target\release\nvidia-tauri.exe
) else (
    echo ERROR: Tauri executable not found!
    echo Searching for nvidia-tauri.exe...
    dir /s /b nvidia-tauri.exe 2>nul
    pause
    exit /b 1
)

echo.
echo Tauri build completed successfully!)

echo.
echo Tauri build completed successfully!
echo.

echo [2/2] Building installer...
echo ========================================
echo.

call build_installer.bat

if errorlevel 1 (
    echo.
    echo Error: Installer build failed!
    pause
    exit /b 1
)

echo.
echo ========================================
echo   Complete Build Finished!
echo ========================================
echo.
echo Your installer is ready:
echo   installer\Nvidia-Setup.exe
echo.
echo The installer includes:
echo   - Interactive domain selection
echo   - Automatic configuration
echo   - All required files
echo.
echo Users will be able to choose:
echo   - Root domain (pinmypic.online)
echo   - Helper subdomain (helper.pinmypic.online)
echo   - Interview subdomain (interview.pinmypic.online)
echo   - Custom subdomain
echo.
pause
