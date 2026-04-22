@echo off
title SEB Injection Helper
color 0A

echo ========================================
echo   SEB Injection Helper
echo ========================================
echo.
echo This will:
echo 1. Start the injector (waiting mode)
echo 2. Wait for you to start SEB
echo 3. Inject automatically when SEB starts
echo.
echo Press any key to start...
pause >nul

echo.
echo [*] Starting injector in background...
start /min injector.exe

echo [*] Injector is now waiting for SEB
echo.
echo Now:
echo 1. Start Safe Exam Browser
echo 2. Injector will detect it automatically
echo 3. Check C:\temp\seb_injection.log for status
echo.
echo Press any key to exit this window...
pause >nul
