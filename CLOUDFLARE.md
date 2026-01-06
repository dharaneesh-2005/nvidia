# Cloudflare Tunnel Setup

## Step 1: Download cloudflared

**Windows:**
https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe

Download and rename to `cloudflared.exe`

## Step 2: Quick Start (No Login Required)

```bash
# Run this in a new terminal (keep your app running)
cloudflared tunnel --url http://localhost:5000
```

You'll get a URL like: `https://random-words-123.trycloudflare.com`

**Done!** Share this URL to access from anywhere.

---

## Step 3: Permanent Tunnel (Recommended)

### Login to Cloudflare
```bash
cloudflared tunnel login
```
This opens browser - login with your Cloudflare account (free signup if needed)

### Create Named Tunnel
```bash
cloudflared tunnel create interview-helper
```

### Configure Tunnel
Create file: `cloudflare-config.yml`

```yaml
tunnel: interview-helper
credentials-file: C:\Users\YOUR_USERNAME\.cloudflared\TUNNEL_ID.json

ingress:
  - hostname: interview.yourdomain.com
    service: http://localhost:5000
  - service: http_status:404
```

### Run Tunnel
```bash
cloudflared tunnel run interview-helper
```

### Route DNS (in Cloudflare Dashboard)
```bash
cloudflared tunnel route dns interview-helper interview.yourdomain.com
```

---

## Quick Command (No Setup)

Just run this alongside your app:

```bash
cloudflared tunnel --url http://localhost:5000
```

Access the generated URL from anywhere!

## Run Both Together

**Terminal 1:**
```bash
cd E:\project\newphonewrtc\interview-helper
cargo run --release
```

**Terminal 2:**
```bash
cloudflared tunnel --url http://localhost:5000
```

Copy the `https://` URL and use it anywhere in the world!
