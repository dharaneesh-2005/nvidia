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

REM Start Pinggy tunnel with fixed subdomain
echo [2/2] Creating your permanent URL...
start /MIN pinggy.exe -p 5000 tcp://a.pinggy.io

timeout /t 3 /nobreak >nul

echo.
echo ========================================
echo   Interview Helper is Running!
echo ========================================
echo.
echo Your permanent URL will be shown in the
echo Pinggy window (check taskbar)
echo.
echo It looks like: https://xxxxx.a.pinggy.io
echo (Same URL every time!)
echo ========================================
echo.
echo Press any key to stop...
pause >nul

taskkill /F /IM interview-helper.exe >nul 2>nul
taskkill /F /IM pinggy.exe >nul 2>nul
echo Stopped.
pause
