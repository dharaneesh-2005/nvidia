@echo off
cd /d "%~dp0"

echo ========================================
echo   Configuring Custom Domain
echo   helper.pinmypic.online
echo ========================================
echo.

echo [1/2] Adding DNS route...
cloudflared tunnel route dns interview-helper helper.pinmypic.online

if errorlevel 1 (
    echo.
    echo Error: Failed to add DNS route
    echo Make sure pinmypic.online is in your Cloudflare account
    pause
    exit /b 1
)

echo.
echo [2/2] Updating config.yml...

(
echo tunnel: interview-helper
echo credentials-file: credentials.json
echo.
echo ingress:
echo   - hostname: helper.pinmypic.online
echo     service: http://localhost:5000
echo   - service: http_status:404
) > config.yml

echo.
echo ========================================
echo   Setup Complete!
echo ========================================
echo.
echo Your permanent URL:
echo   https://helper.pinmypic.online
echo.
echo This URL will work every time you run the app!
echo.
pause
