@echo off
cd /d "%~dp0"

REM Start the main application hidden
start /B nvidia.exe

REM Wait a moment for main app to initialize
timeout /t 2 /nobreak >nul

REM Start the Tauri PiP application hidden
start /B nvidia-tauri.exe

REM Wait a moment for Tauri app to initialize
timeout /t 2 /nobreak >nul

REM Start cloudflared tunnel hidden
start /B cloudflared.exe tunnel --config config.yml run interview-helper
