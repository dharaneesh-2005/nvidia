@echo off
echo Testing injection without SEB
echo.
echo Step 1: Starting notepad as test target...
start notepad.exe
timeout /t 2 /nobreak >nul

echo Step 2: Running injector...
echo (This will fail because injector looks for SafeExamBrowser.exe)
echo.
injector.exe

echo.
echo To test properly:
echo 1. Edit injector.cpp to also look for "notepad.exe"
echo 2. Recompile
echo 3. Run this script again
echo.
pause
