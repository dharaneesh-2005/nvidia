@echo off
echo Stopping Interview Helper...
taskkill /F /IM interview_helper.exe >nul 2>nul
taskkill /F /IM cloudflared.exe >nul 2>nul
echo Stopped.
timeout /t 2 /nobreak >nul
