@echo off
setlocal enabledelayedexpansion
title Complete Wildcard Setup
cd /d "%~dp0"

echo ========================================
echo   Complete Wildcard Setup
echo   (One-time Developer Setup)
echo ========================================
echo.
echo This will configure your tunnel to accept ANY subdomain
echo under pinmypic.online, so users can choose any subdomain
echo during installation without additional DNS setup.
echo.
pause

REM Check if cloudflared exists
if not exist cloudflared.exe (
    echo [1/5] Downloading cloudflared...
    powershell -Command "Invoke-WebRequest -Uri 'https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe' -OutFile 'cloudflared.exe'"
    echo Done!
) else (
    echo [1/5] cloudflared.exe found
)

echo.
echo [2/5] Logging into Cloudflare...
echo A browser window will open - please login
echo.
cloudflared.exe tunnel login

if errorlevel 1 (
    echo Login failed!
    pause
    exit /b 1
)

echo.
echo [3/5] Creating tunnel 'interview-helper'...
cloudflared.exe tunnel create interview-helper 2>nul

REM Get tunnel ID
for /f "tokens=1" %%i in ('cloudflared.exe tunnel list ^| findstr "interview-helper"') do set TUNNEL_ID=%%i

if not defined TUNNEL_ID (
    echo Error: Could not find tunnel ID
    pause
    exit /b 1
)

echo Tunnel ID: %TUNNEL_ID%

REM Copy credentials file
if exist "%USERPROFILE%\.cloudflared\%TUNNEL_ID%.json" (
    copy "%USERPROFILE%\.cloudflared\%TUNNEL_ID%.json" credentials.json >nul
    echo Credentials copied
)

echo.
echo [4/5] Creating wildcard config.yml...

(
echo tunnel: interview-helper
echo credentials-file: credentials.json
echo.
echo ingress:
echo   - hostname: "*.pinmypic.online"
echo     service: http://localhost:5000
echo   - hostname: pinmypic.online
echo     service: http://localhost:5000
echo   - service: http_status:404
) > config.yml

echo Config created with wildcard support

echo.
echo [5/5] Adding wildcard DNS route...
cloudflared.exe tunnel route dns interview-helper *.pinmypic.online

if errorlevel 1 (
    echo.
    echo ========================================
    echo   Manual DNS Setup Required
    echo ========================================
    echo.
    echo Automatic wildcard DNS setup failed.
    echo Please add manually in Cloudflare Dashboard:
    echo.
    echo 1. Go to: https://dash.cloudflare.com
    echo 2. Select: pinmypic.online
    echo 3. DNS ^> Records ^> Add record
    echo 4. Configure:
    echo    Type: CNAME
    echo    Name: *
    echo    Target: %TUNNEL_ID%.cfargotunnel.com
    echo    Proxy: Enabled
    echo.
    pause
) else (
    echo Wildcard DNS route added successfully!
)

echo.
echo ========================================
echo   Setup Complete!
echo ========================================
echo.
echo Wildcard configuration enabled!
echo.
echo ALL subdomains will now work:
echo   - helper.pinmypic.online
echo   - myapp.pinmypic.online
echo   - john.pinmypic.online
echo   - ANY.pinmypic.online
echo.
echo Files created:
echo   - cloudflared.exe
echo   - credentials.json
echo   - config.yml (with wildcard support)
echo.
echo You can now build your installer!
echo Users can choose ANY subdomain during installation.
echo.
pause
