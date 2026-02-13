@echo off
title Setup Wildcard Config
cd /d "%~dp0"

echo ========================================
echo   Setup Wildcard Configuration
echo ========================================
echo.

echo This will update config.yml to accept ALL subdomains
echo under pinmypic.online
echo.
pause

echo.
echo Updating config.yml...
echo.

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

echo.
echo ========================================
echo   Configuration Updated!
echo ========================================
echo.
echo config.yml now accepts:
echo   - ANY subdomain (*.pinmypic.online)
echo   - Root domain (pinmypic.online)
echo.
echo All subdomains will route to localhost:5000
echo.
pause
