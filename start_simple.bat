@echo off
title Interview Helper
cd /d "%~dp0"

echo ========================================
echo   Starting Interview Helper
echo ========================================
echo.

REM Start the application
echo [1/2] Starting server...
start /B interview-helper.exe

timeout /t 2 /nobreak >nul

REM Start LocalTunnel with fixed subdomain
echo [2/2] Creating tunnel...
start /B npx localtunnel --port 5000 --subdomain interview-helper-app

timeout /t 3 /nobreak >nul

echo.
echo ========================================
echo   Interview Helper is Running!
echo ========================================
echo.
echo Access from your phone:
echo   https://interview-helper-app.loca.lt
echo.
echo (First time: click "Continue" on the page)
echo ========================================
echo.
echo Press any key to stop...
pause >nul

taskkill /F /IM interview-helper.exe >nul 2>nul
taskkill /F /IM node.exe >nul 2>nul
echo Stopped.
pause
