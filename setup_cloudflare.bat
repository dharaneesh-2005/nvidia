@echo off
setlocal enabledelayedexpansion
title Cloudflare Tunnel Setup
cd /d "%~dp0"

echo ========================================
echo   Cloudflare Tunnel Auto-Setup
echo   with Domain Selection
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
echo [4/5] Domain Selection
echo ========================================
echo.
echo Choose your subdomain for pinmypic.online:
echo.
echo   1. Helper subdomain (helper.pinmypic.online)
echo   2. Custom subdomain
echo.
set /p DOMAIN_CHOICE="Enter your choice (1-2): "

if "%DOMAIN_CHOICE%"=="1" (
    set FULL_DOMAIN=helper.pinmypic.online
) else if "%DOMAIN_CHOICE%"=="2" (
    set /p CUSTOM_SUB="Enter subdomain name: "
    set FULL_DOMAIN=!CUSTOM_SUB!.pinmypic.online
) else (
    echo Invalid choice, using helper subdomain
    set FULL_DOMAIN=helper.pinmypic.online
)

echo.
echo Selected: %FULL_DOMAIN%
echo.

REM Get tunnel ID
for /f "tokens=1" %%i in ('cloudflared.exe tunnel list ^| findstr "interview-helper"') do set TUNNEL_ID=%%i

if not defined TUNNEL_ID (
    echo Error: Could not find tunnel ID
    pause
    exit /b 1
)

REM Copy credentials file
copy "%USERPROFILE%\.cloudflared\%TUNNEL_ID%.json" credentials.json

REM Save domain configuration
echo %FULL_DOMAIN% > domain.txt

REM Create config.yml with selected domain
(
echo tunnel: interview-helper
echo credentials-file: credentials.json
echo.
echo ingress:
echo   - hostname: %FULL_DOMAIN%
echo     service: http://localhost:5000
echo   - service: http_status:404
) > config.yml

echo.
echo [5/5] Setting up DNS route...
cloudflared.exe tunnel route dns interview-helper %FULL_DOMAIN%

echo.
echo ========================================
echo   Setup Complete!
echo ========================================
echo.
echo Your permanent URL: https://%FULL_DOMAIN%
echo.
echo Files created:
echo   - cloudflared.exe
echo   - credentials.json
echo   - config.yml
echo   - domain.txt (your domain configuration)
echo.
echo You can now build your installer!
echo.
pause
