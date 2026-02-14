@echo off
setlocal enabledelayedexpansion
title Create 5 Tunnels for Multiple Users
cd /d "%~dp0"

echo ========================================
echo   Create 5 Tunnels
echo ========================================
echo.
echo This will create 5 separate tunnels:
echo   1. nvidia-dharaneesh
echo   2. nvidia-steepan
echo   3. nvidia-dinesh
echo   4. nvidia-backup1
echo   5. nvidia-backup2
echo.
pause

REM Login to Cloudflare once
echo Logging into Cloudflare...
cloudflared.exe tunnel login

if errorlevel 1 (
    echo Login failed!
    pause
    exit /b 1
)

REM Create directory for credentials
if not exist "tunnels" mkdir tunnels

REM Array of tunnel names
set TUNNELS=nvidia-dharaneesh nvidia-steepan nvidia-dinesh nvidia-backup1 nvidia-backup2

for %%T in (%TUNNELS%) do (
    echo.
    echo ========================================
    echo Creating tunnel: %%T
    echo ========================================
    
    cloudflared.exe tunnel create %%T
    
    REM Get tunnel ID
    for /f "tokens=1" %%i in ('cloudflared.exe tunnel list ^| findstr "%%T"') do (
        set TUNNEL_ID=%%i
        echo Tunnel ID: %%i
        
        REM Copy credentials to tunnels folder
        copy "%USERPROFILE%\.cloudflared\%%i.json" "tunnels\%%T.json" >nul
        echo Credentials saved: tunnels\%%T.json
    )
    
    echo Done!
)

echo.
echo ========================================
echo   All Tunnels Created!
echo ========================================
echo.
echo Tunnels created:
cloudflared.exe tunnel list | findstr "nvidia-"
echo.
echo Credentials saved in: tunnels\
dir /b tunnels\*.json
echo.
echo Next step: Run build_installer.bat to create installer
echo The installer will include all 5 tunnel credentials.
echo.
pause
