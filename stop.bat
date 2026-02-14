@echo off
echo Stopping Nvidia...
taskkill /F /IM nvidia.exe >nul 2>nul
taskkill /F /IM cloudflared.exe >nul 2>nul
echo Stopped.
timeout /t 2 /nobreak >nul
