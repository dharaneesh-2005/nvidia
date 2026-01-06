@echo off
title Cloudflare Tunnel Setup
cd /d "%~dp0"

echo ========================================
echo   Cloudflare Tunnel Auto-Setup
echo ========================================
echo.

REM Check if cloudflared exists
if not exist cloudflared.exe (
    echo [1/4] Downloading cloudflared...
    powershell -Command "Invoke-WebRequest -Uri 'https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe' -OutFile 'cloudflared.exe'"
    echo Done!
) else (
    echo [1/4] cloudflared.exe found - skipping download
)

echo.
echo [2/4] Logging into Cloudflare...
echo A browser window will open - please login
echo.
cloudflared.exe tunnel login

if errorlevel 1 (
    echo.
    echo Login failed! Please try again.
    pause
    exit /b 1
)

echo.
echo [3/4] Creating tunnel 'interview-helper'...
cloudflared.exe tunnel create interview-helper

if errorlevel 1 (
    echo.
    echo Note: Tunnel might already exist, continuing...
)

echo.
echo [4/4] Setting up configuration...

REM Get tunnel ID
for /f "tokens=1" %%i in ('cloudflared.exe tunnel list ^| findstr "interview-helper"') do set TUNNEL_ID=%%i

if not defined TUNNEL_ID (
    echo Error: Could not find tunnel ID
    pause
    exit /b 1
)

REM Copy credentials file
copy "%USERPROFILE%\.cloudflared\%TUNNEL_ID%.json" credentials.json

REM Create config.yml
(
echo tunnel: interview-helper
echo credentials-file: credentials.json
echo.
echo ingress:
echo   - hostname: pinmypic.online
echo     service: http://localhost:5000
echo   - service: http_status:404
) > config.yml

echo.
echo [5/5] Setting up DNS route...
cloudflared.exe tunnel route dns interview-helper pinmypic.online

echo.
echo ========================================
echo   Setup Complete!
echo ========================================
echo.
echo Your permanent URL: https://pinmypic.online
echo.
echo Files created:
echo   - cloudflared.exe
echo   - credentials.json
echo   - config.yml
echo.
echo You can now build your installer!
echo.
pause
