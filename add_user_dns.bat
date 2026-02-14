@echo off
setlocal enabledelayedexpansion
title Add DNS for User
cd /d "%~dp0"

echo ========================================
echo   Add DNS Route for User
echo ========================================
echo.
echo This script adds DNS routes for users.
echo Run this on YOUR machine (with Cloudflare credentials).
echo.

echo Available tunnels:
echo   1. nvidia-dharaneesh
echo   2. nvidia-steepan
echo   3. nvidia-dinesh
echo   4. nvidia-backup1
echo   5. nvidia-backup2
echo.

set /p TUNNEL_CHOICE="Select tunnel (1-5): "

if "%TUNNEL_CHOICE%"=="1" set TUNNEL_NAME=nvidia-dharaneesh
if "%TUNNEL_CHOICE%"=="2" set TUNNEL_NAME=nvidia-steepan
if "%TUNNEL_CHOICE%"=="3" set TUNNEL_NAME=nvidia-dinesh
if "%TUNNEL_CHOICE%"=="4" set TUNNEL_NAME=nvidia-backup1
if "%TUNNEL_CHOICE%"=="5" set TUNNEL_NAME=nvidia-backup2

if not defined TUNNEL_NAME (
    echo Invalid choice!
    pause
    exit /b 1
)

echo.
set /p SUBDOMAIN="Enter subdomain (e.g., john for john.pinmypic.online): "

if "%SUBDOMAIN%"=="" (
    echo Subdomain cannot be empty!
    pause
    exit /b 1
)

set FULL_DOMAIN=%SUBDOMAIN%.pinmypic.online

echo.
echo Adding DNS route:
echo   Tunnel: %TUNNEL_NAME%
echo   Domain: %FULL_DOMAIN%
echo.

cloudflared.exe tunnel route dns %TUNNEL_NAME% %FULL_DOMAIN%

if errorlevel 1 (
    echo.
    echo Failed to add DNS route!
    pause
    exit /b 1
) else (
    echo.
    echo ========================================
    echo   DNS Route Added!
    echo ========================================
    echo.
    echo Tunnel: %TUNNEL_NAME%
    echo Domain: https://%FULL_DOMAIN%
    echo.
    echo The user can now access their app at:
    echo https://%FULL_DOMAIN%
    echo.
    echo Wait 2-5 minutes for DNS propagation.
    echo.
)

pause
