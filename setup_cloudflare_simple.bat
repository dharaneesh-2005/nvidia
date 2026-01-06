@echo off
title Cloudflare Quick Setup (No Domain Needed)
cd /d "%~dp0"

echo ========================================
echo   Cloudflare Quick Setup
echo   (No custom domain needed!)
echo ========================================
echo.

REM Check if cloudflared exists
if not exist cloudflared.exe (
    echo [1/3] Downloading cloudflared...
    powershell -Command "Invoke-WebRequest -Uri 'https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe' -OutFile 'cloudflared.exe'"
    echo Done!
) else (
    echo [1/3] cloudflared.exe found
)

echo.
echo [2/3] Logging into Cloudflare...
echo A browser will open - login with your Cloudflare account
echo (Create free account at cloudflare.com if needed)
echo.
pause
cloudflared.exe tunnel login

if errorlevel 1 (
    echo Login failed!
    pause
    exit /b 1
)

echo.
echo [3/3] Creating tunnel...
cloudflared.exe tunnel create interview-helper

REM Get tunnel ID
for /f "tokens=1" %%i in ('cloudflared.exe tunnel list ^| findstr "interview-helper"') do set TUNNEL_ID=%%i

if not defined TUNNEL_ID (
    echo Error: Could not find tunnel
    pause
    exit /b 1
)

REM Copy credentials
copy "%USERPROFILE%\.cloudflared\%TUNNEL_ID%.json" credentials.json

REM Create simple config
(
echo tunnel: interview-helper
echo credentials-file: credentials.json
) > config.yml

echo.
echo ========================================
echo   Setup Complete!
echo ========================================
echo.
echo Files created:
echo   - cloudflared.exe
echo   - credentials.json  
echo   - config.yml
echo.
echo Now you can build your installer.
echo Users will get a URL like:
echo   https://xxxxx.trycloudflare.com
echo.
pause
