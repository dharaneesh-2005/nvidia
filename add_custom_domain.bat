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
echo.

REM Check if it's already helper.pinmypic.online (already configured)
if "%CUSTOM_DOMAIN%"=="helper.pinmypic.online" (
    echo This domain is already configured!
    echo No additional setup needed.
    pause
    exit /b 0
)

echo This custom domain needs to be added to Cloudflare.
echo.
echo Adding DNS route for: %CUSTOM_DOMAIN%
echo.

cloudflared.exe tunnel route dns interview-helper %CUSTOM_DOMAIN%

if errorlevel 1 (
    echo.
    echo ========================================
    echo   DNS Route Setup Failed
    echo ========================================
    echo.
    echo Please add the DNS record manually:
    echo.
    echo 1. Go to Cloudflare Dashboard
    echo 2. Select domain: pinmypic.online
    echo 3. Go to DNS settings
    echo 4. Add CNAME record:
    echo    - Type: CNAME
    echo    - Name: %CUSTOM_DOMAIN:~0,-16%
    echo    - Target: [tunnel-id].cfargotunnel.com
    echo.
    echo To find your tunnel ID, run:
    echo    cloudflared.exe tunnel list
    echo.
    pause
    exit /b 1
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
