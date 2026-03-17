@echo off
chcp 65001 >nul
echo ============================================
echo  Nvidia PiP - Native Window Build Script
echo ============================================
echo.

:: Check if cargo-tauri is installed
cargo tauri --version >nul 2>&1
if errorlevel 1 (
    echo [INFO] Installing Tauri CLI...
    cargo install tauri-cli --version "^2.0"
)

echo [1/4] Building Tauri native application...
cd src-tauri
cargo build --release --bin nvidia-tauri
if errorlevel 1 (
    echo [ERROR] Failed to build Tauri application
    pause
    exit /b 1
)
cd ..

echo [2/4] Building main Rust backend...
cargo build --release --bin nvidia
if errorlevel 1 (
    echo [ERROR] Failed to build main backend
    pause
    exit /b 1
)

echo [3/4] Creating distribution directory...
if not exist "dist" mkdir dist
copy "target\release\nvidia.exe" "dist\"
copy "target\release\nvidia-tauri.exe" "dist\" 2>nul || copy "target\release\nvidia_tauri.exe" "dist\" 2>nul || echo [WARN] Tauri binary not found, skipping...
xcopy /E /I /Y "static" "dist\static" 2>nul || echo [WARN] Static files not copied"

echo [4/4] Build complete!
echo.
echo Distribution files in 'dist' folder:
dir /B dist
echo.
echo To run:
echo   1. Start the backend: dist\nvidia.exe
echo   2. Start the native PiP wrapper: dist\nvidia-tauri.exe
echo.
echo Or use the combined launcher: start_pip.bat
echo.
pause