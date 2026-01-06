@echo off
cd /d "%~dp0"
echo Getting tunnel info...
echo.
cloudflared tunnel info interview-helper
echo.
pause
