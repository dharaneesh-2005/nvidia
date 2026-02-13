@echo off
title Building Nvidia Installer
cd /d "%~dp0"

echo ========================================
echo   Building Nvidia Installer
echo ========================================
echo.

REM Check if Inno Setup is installed
set INNO_PATH=C:\Program Files (x86)\Inno Setup 6\ISCC.exe

if not exist "%INNO_PATH%" (
    echo Error: Inno Setup not found at: %INNO_PATH%
    echo.
    echo Please install Inno Setup 6 from:
    echo https://jrsoftware.org/isdl.php
    echo.
    pause
    exit /b 1
)

echo [1/3] Checking required files...
echo.

REM Check for required files
set MISSING=0

if not exist "target\release\interview_helper.exe" (
    echo Missing: target\release\interview_helper.exe
    echo Run: cargo build --release
    set MISSING=1
)

if not exist "cloudflared.exe" (
    echo Missing: cloudflared.exe
    echo Run: setup_cloudflare.bat first
    set MISSING=1
)

if not exist "credentials.json" (
    echo Missing: credentials.json
    echo Run: setup_cloudflare.bat first
    set MISSING=1
)

if not exist "config.yml" (
    echo Missing: config.yml
    echo Run: setup_cloudflare.bat first
    set MISSING=1
)

if %MISSING%==1 (
    echo.
    echo Error: Missing required files!
    echo Please ensure all files are present before building.
    pause
    exit /b 1
)

echo All required files found!
echo.

echo [2/3] Compiling installer with Inno Setup...
echo.

"%INNO_PATH%" setup.iss

if errorlevel 1 (
    echo.
    echo Error: Installer compilation failed!
    pause
    exit /b 1
)

echo.
echo [3/3] Installer built successfully!
echo.

if exist "installer\Nvidia-Setup.exe" (
    echo ========================================
    echo   Build Complete!
    echo ========================================
    echo.
    echo Installer location:
    echo   installer\Nvidia-Setup.exe
    echo.
    echo The installer will prompt users to select their subdomain
    echo during installation.
    echo.
    echo Domain options:
    echo   1. Root domain (pinmypic.online)
    echo   2. Helper subdomain (helper.pinmypic.online)
    echo   3. Interview subdomain (interview.pinmypic.online)
    echo   4. Custom subdomain
    echo.
) else (
    echo Warning: Installer file not found in expected location
)

pause
