@echo off
title Interview Helper
cd /d "%~dp0"

echo ========================================
echo   Starting Interview Helper
echo ========================================
echo.

REM Start the application
echo [1/2] Starting server...
start /B interview_helper.exe

timeout /t 2 /nobreak >nul

REM Start cloudflared tunnel with custom domain
echo [2/2] Starting tunnel...
echo.
cloudflared.exe tunnel --config config.yml run interview-helper

timeout /t 5 /nobreak >nul

echo.
echo ========================================
echo   Interview Helper is Running!
echo ========================================
echo.
echo Access from your phone:
echo.
echo   https://helper.dhans.online
echo.
echo (This URL never changes!)
echo ========================================
echo.
echo Press any key to stop...
pause >nul

taskkill /F /IM interview_helper.exe >nul 2>nul
taskkill /F /IM cloudflared.exe >nul 2>nul
echo.
echo Stopped.
pause

timeout /t 5 /nobreak >nul

echo.
echo ========================================
echo   Interview Helper is Running!
echo ========================================
echo.
echo Your tunnel URL is shown in the Cloudflare window.
echo It looks like: https://xxxxx.trycloudflare.com
echo.
echo This URL stays the same every time!
echo Bookmark it on your phone.
echo ========================================
echo.
echo Press any key to stop...
pause >nul

taskkill /F /IM interview_helper.exe >nul 2>nul
taskkill /F /IM cloudflared.exe >nul 2>nul
echo.
echo Stopped.
pause
