@echo off
cd /d "%~dp0"
echo Adding public hostname to tunnel...
echo.
cloudflared tunnel route dns interview-helper interview-helper
echo.
echo Done! Your URL will be shown when you run start.bat
pause
