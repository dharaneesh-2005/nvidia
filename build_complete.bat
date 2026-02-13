@echo off
title Complete Build Process
cd /d "%~dp0"

echo ========================================
echo   Nvidia - Complete Build Process
echo ========================================
echo.
echo This script will:
echo   1. Build the Rust application
echo   2. Compile the installer
echo.
pause

echo.
echo [1/2] Building Rust application...
echo ========================================
echo.

cargo build --release

if errorlevel 1 (
    echo.
    echo Error: Rust build failed!
    echo Please check the error messages above.
    pause
    exit /b 1
)

echo.
echo Rust build completed successfully!
echo.

echo [2/2] Building installer...
echo ========================================
echo.

call build_installer.bat

if errorlevel 1 (
    echo.
    echo Error: Installer build failed!
    pause
    exit /b 1
)

echo.
echo ========================================
echo   Complete Build Finished!
echo ========================================
echo.
echo Your installer is ready:
echo   installer\Nvidia-Setup.exe
echo.
echo The installer includes:
echo   - Interactive domain selection
echo   - Automatic configuration
echo   - All required files
echo.
echo Users will be able to choose:
echo   - Root domain (pinmypic.online)
echo   - Helper subdomain (helper.pinmypic.online)
echo   - Interview subdomain (interview.pinmypic.online)
echo   - Custom subdomain
echo.
pause
