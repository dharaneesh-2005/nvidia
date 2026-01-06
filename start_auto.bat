@echo off
title Interview Helper
cd /d "%~dp0"

echo ========================================
echo   Starting Interview Helper
echo ========================================
echo.

REM Start the application
echo [1/2] Starting server...
start /B interview-helper.exe

timeout /t 2 /nobreak >nul

REM Start cloudflared tunnel (auto-connects)
echo [2/2] Connecting to your URL...
start /MIN cloudflared.exe tunnel --config config.yml run

timeout /t 3 /nobreak >nul

echo.
echo ========================================
echo   Interview Helper is Running!
echo ========================================
echo.
echo Access from anywhere:
echo   https://pinmypic.online
echo.
echo ========================================
echo.
echo Press any key to stop...
pause >nul

taskkill /F /IM interview-helper.exe >nul 2>nul
taskkill /F /IM cloudflared.exe >nul 2>nul
echo Stopped.
pause
