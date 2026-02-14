@echo off
title Add Custom Domain to Cloudflare
cd /d "%~dp0"

echo ========================================
echo   Add Custom Domain to Cloudflare
echo ========================================
echo.

REM Check if domain.txt exists
if not exist "domain.txt" (
    echo Error: domain.txt not found!
    echo This file should be created during installation.
    pause
    exit /b 1
)

REM Read the domain from domain.txt
set /p CUSTOM_DOMAIN=<domain.txt

echo Current domain: %CUSTOM_DOMAIN%
echo Tunnel: interview-helper
echo.

echo Adding DNS route for: %CUSTOM_DOMAIN%
echo.

cloudflared.exe --origincert cert.pem tunnel route dns interview-helper %CUSTOM_DOMAIN%

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
