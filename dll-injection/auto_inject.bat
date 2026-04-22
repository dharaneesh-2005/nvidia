@echo off
REM Wait for SEB to fully start
timeout /t 5 /nobreak >nul

REM Run injector
cd /d "%~dp0"
injector.exe

REM Keep window open to see results
pause
