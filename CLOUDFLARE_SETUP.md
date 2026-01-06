# Cloudflare Tunnel Setup for Permanent URL

## Option 1: Quick Tunnel (Temporary - Changes on restart)
This is what start.bat uses by default:
```
cloudflared tunnel --url http://localhost:5000
```
- Free, no login required
- URL changes every time you restart
- Good for testing

## Option 2: Named Tunnel (PERMANENT - Recommended)

### Step 1: Login to Cloudflare
```
cloudflared tunnel login
```
This opens browser - login with your Cloudflare account (free)

### Step 2: Create Named Tunnel
```
cloudflared tunnel create interview-helper
```
This creates a permanent tunnel and saves credentials

### Step 3: Create Config File
Create file: C:\Users\<YourUser>\.cloudflared\config.yml
```yaml
tunnel: interview-helper
credentials-file: C:\Users\<YourUser>\.cloudflared\<tunnel-id>.json

ingress:
  - hostname: interview-helper.yourdomain.com
    service: http://localhost:5000
  - service: http_status:404
```

### Step 4: Route DNS (if you have domain)
```
cloudflared tunnel route dns interview-helper interview-helper.yourdomain.com
```

### Step 5: Run Tunnel
```
cloudflared tunnel run interview-helper
```

## Option 3: Cloudflare Pages Tunnel (FREE + Custom Domain)

### Step 1: Create Tunnel with Pages
```
cloudflared tunnel create interview-helper
```

### Step 2: Get Tunnel ID
```
cloudflared tunnel list
```

### Step 3: Use Cloudflare Dashboard
1. Go to https://one.dash.cloudflare.com/
2. Navigate to Networks > Tunnels
3. Create tunnel "interview-helper"
4. Add public hostname: interview-helper-<random>.trycloudflare.com
5. Point to http://localhost:5000

This gives you a PERMANENT URL like:
https://interview-helper-abc123.trycloudflare.com

## Recommended: Use Option 3 for permanent free URL without domain!

### Update start.bat for permanent tunnel:
```batch
@echo off
title Interview Helper
cd /d "%~dp0"

echo Starting Interview Helper...
start /B interview-helper.exe

timeout /t 3 /nobreak >nul

echo Starting Cloudflare Tunnel...
cloudflared tunnel run interview-helper

pause
```
