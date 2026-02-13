@echo off
title Setup Wildcard DNS for All Subdomains
cd /d "%~dp0"

echo ========================================
echo   Setup Wildcard DNS
echo   (Allows ANY subdomain to work)
echo ========================================
echo.

echo This will configure Cloudflare to accept ANY subdomain
echo under pinmypic.online for your tunnel.
echo.
echo Examples that will work after this:
echo   - helper.pinmypic.online
echo   - myapp.pinmypic.online
echo   - john.pinmypic.online
echo   - anything.pinmypic.online
echo.
pause

echo.
echo Adding wildcard DNS route...
echo.

cloudflared.exe tunnel route dns interview-helper *.pinmypic.online

if errorlevel 1 (
    echo.
    echo ========================================
    echo   Wildcard DNS Failed
    echo ========================================
    echo.
    echo The wildcard DNS route could not be added automatically.
    echo.
    echo Please add it manually in Cloudflare Dashboard:
    echo.
    echo 1. Go to: https://dash.cloudflare.com
    echo 2. Select domain: pinmypic.online
    echo 3. Go to DNS ^> Records
    echo 4. Add CNAME record:
    echo    - Type: CNAME
    echo    - Name: *
    echo    - Target: [tunnel-id].cfargotunnel.com
    echo    - Proxy: Enabled (orange cloud)
    echo.
    echo To find tunnel ID, run: cloudflared.exe tunnel list
    echo.
    pause
    exit /b 1
)

echo.
echo ========================================
echo   Wildcard DNS Configured!
echo ========================================
echo.
echo ALL subdomains under pinmypic.online will now work!
echo.
echo You can now use ANY subdomain in your installer:
echo   - helper.pinmypic.online
echo   - custom1.pinmypic.online
echo   - custom2.pinmypic.online
echo   - etc.
echo.
echo No additional DNS setup needed for new subdomains!
echo.
pause
