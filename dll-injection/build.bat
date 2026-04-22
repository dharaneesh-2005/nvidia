@echo off
echo Building SEB DLL Injection Components...
echo.

REM Check for Visual Studio
where cl.exe >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo Visual Studio not found in PATH
    echo Please run this from "x64 Native Tools Command Prompt for VS"
    pause
    exit /b 1
)

echo [1/3] Compiling injector.exe...
cl /nologo /O2 /EHsc injector.cpp /Fe:injector.exe /link user32.lib kernel32.lib advapi32.lib
if %ERRORLEVEL% NEQ 0 (
    echo Failed to compile injector
    pause
    exit /b 1
)

echo [2/3] Compiling payload.dll...
cl /nologo /LD /O2 /EHsc payload.cpp /Fe:payload.dll /link user32.lib kernel32.lib advapi32.lib
if %ERRORLEVEL% NEQ 0 (
    echo Failed to compile payload
    pause
    exit /b 1
)

echo [3/3] Cleaning up...
del *.obj *.exp *.lib 2>nul

echo.
echo ========================================
echo Build complete!
echo ========================================
echo.
echo Files created:
echo   - injector.exe  (DLL injector)
echo   - payload.dll   (Payload to inject)
echo.
echo Usage:
echo   1. Start SafeExamBrowser
echo   2. Run: injector.exe
echo   3. Interview helper will start invisibly
echo.
pause
