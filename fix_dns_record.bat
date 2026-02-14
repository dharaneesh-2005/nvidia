@echo off
title Fix DNS Record
cd /d "%~dp0"

echo ========================================
echo   Fix DNS Record
echo ========================================
echo.
echo This will help you fix the DNS record that's pointing
echo to the wrong tunnel.
echo.
echo Steps:
echo 1. Go to Cloudflare Dashboard: https://dash.cloudflare.com
echo 2. Select domain: pinmypic.online
echo 3. Go to DNS ^> Records
echo 4. Find the record: dhansa.pinmypic.online
echo 5. Delete it
echo 6. Run this script again to add the correct record
echo.
pause

echo.
echo Adding DNS route for nvidia-dharaneesh tunnel...
echo.

cloudflared.exe --origincert cert.pem tunnel route dns nvidia-dharaneesh dhansa.pinmypic.online

if errorlevel 1 (
    echo.
    echo Failed! The record still exists.
    echo Please delete it manually in Cloudflare Dashboard first.
    echo.
) else (
    echo.
    echo Success! DNS record added.
    echo Wait 2-5 minutes and try accessing:
    echo https://dhansa.pinmypic.online
    echo.
)

pause
