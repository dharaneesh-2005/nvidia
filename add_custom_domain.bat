@echo off
title Add Custom Domain to Cloudflare
cd /d "%~dp0"

echo ========================================
echo   Add Custom Domain to Cloudflare
echo ========================================
echo.
echo Current directory: %CD%
echo.

REM Check if we're in the right directory
if not exist "cloudflared.exe" (
    echo Error: cloudflared.exe not found!
    echo.
    echo You must run this script from the Nvidia installation folder.
    echo Default location: C:\Program Files\Nvidia\
    echo.
    pause
    exit /b 1
)

REM Check if config.yml exists (means installation completed)
if not exist "config.yml" (
    echo Error: config.yml not found!
    echo.
    echo Installation appears incomplete. Please reinstall the application.
    pause
    exit /b 1
)

REM Check if tunnel.txt exists
if not exist "tunnel.txt" (
    echo Error: tunnel.txt not found!
    echo.
    echo This file should have been created during installation.
    echo Checking what files exist...
    echo.
    dir /b *.txt
    echo.
    echo Please reinstall the application or contact support.
    pause
    exit /b 1
)

REM Read the tunnel name from tunnel.txt
set /p TUNNEL_NAME=<tunnel.txt

REM Check if domain.txt exists
if not exist "domain.txt" (
    echo Error: domain.txt not found!
    echo.
    echo This file should have been created during installation.
    echo Please reinstall the application or contact support.
    pause
    exit /b 1
)

REM Read the domain from domain.txt
set /p CUSTOM_DOMAIN=<domain.txt

echo Current tunnel: %TUNNEL_NAME%
echo Current domain: %CUSTOM_DOMAIN%
echo.

echo Adding DNS route for: %CUSTOM_DOMAIN%
echo.

cloudflared.exe --origincert cert.pem tunnel route dns %TUNNEL_NAME% %CUSTOM_DOMAIN%

if errorlevel 1 (
    echo.
    echo ========================================
    echo   DNS Record Already Exists or Failed
    echo ========================================
    echo.
    echo The DNS record might already exist, which is fine!
    echo Your app should work at: https://%CUSTOM_DOMAIN%
    echo.
    echo If it doesn't work, check Cloudflare Dashboard
    echo to verify the DNS record points to the correct tunnel.
    echo.
    pause
    exit /b 0
)

echo.
echo ========================================
echo   DNS Route Added Successfully!
echo ========================================
echo.
echo Your custom domain is now configured:
echo   https://%CUSTOM_DOMAIN%
echo.
echo It may take a few minutes for DNS to propagate.
echo.
pause
