@echo off
cd /d "%~dp0"

REM Check if already running
tasklist /FI "IMAGENAME eq interview_helper.exe" 2>NUL | find /I /N "interview_helper.exe">NUL
if "%ERRORLEVEL%"=="0" (
    exit
)

REM Start the application hidden
start /B interview_helper.exe

REM Wait a moment
timeout /t 2 /nobreak >nul

REM Start cloudflared tunnel hidden
start /B cloudflared.exe tunnel --config config.yml run interview-helper
