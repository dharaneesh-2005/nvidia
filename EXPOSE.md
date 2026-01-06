# Expose to Internet

## Option 1: ngrok (Recommended - Easiest)

### Setup
1. Download ngrok: https://ngrok.com/download
2. Sign up for free account: https://dashboard.ngrok.com/signup
3. Get your auth token from dashboard

### Usage
```bash
# Install auth token (one time)
ngrok config add-authtoken YOUR_AUTH_TOKEN

# Expose port 5000
ngrok http 5000
```

You'll get a public URL like: `https://abc123.ngrok.io`

Access from anywhere: `https://abc123.ngrok.io`

**Pros:**
- Free tier available
- HTTPS included
- No router configuration needed
- Works behind firewalls

**Cons:**
- URL changes on restart (unless paid plan)
- Free tier has connection limits

---

## Option 2: Cloudflare Tunnel (Free, Permanent URL)

### Setup
```bash
# Download cloudflared
# https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/

# Login
cloudflared tunnel login

# Create tunnel
cloudflared tunnel create interview-helper

# Run tunnel
cloudflared tunnel --url http://localhost:5000
```

**Pros:**
- Completely free
- Permanent URL
- No connection limits
- HTTPS included

**Cons:**
- Requires Cloudflare account
- Slightly more setup

---

## Option 3: Port Forwarding (Your Own Domain)

### Setup
1. Configure router to forward port 5000 to your PC
2. Get your public IP: https://whatismyipaddress.com/
3. Access via: `http://YOUR_PUBLIC_IP:5000`

**Optional: Use Dynamic DNS**
- Sign up: https://www.noip.com/ or https://www.duckdns.org/
- Get free domain like: `yourname.ddns.net`

**Pros:**
- No third-party service
- Full control

**Cons:**
- Requires router access
- Security risk (expose your home IP)
- No HTTPS (unless you setup SSL)
- Dynamic IP changes

---

## Option 4: Deploy to Cloud (Production)

### VPS Deployment (DigitalOcean, AWS, etc.)

1. Rent a VPS ($5-10/month)
2. Copy your binary to server
3. Run as systemd service
4. Configure nginx with SSL

**Best for:** Production use, multiple users

---

## Quick Start: ngrok

```bash
# 1. Download ngrok
# https://ngrok.com/download

# 2. Run your app
cargo run --release

# 3. In another terminal, expose it
ngrok http 5000

# 4. Copy the https URL and share it
# Example: https://abc123.ngrok-free.app
```

Now anyone can access your interview helper from anywhere!

## Security Warning

⚠️ **Important:** When exposing to internet:
- Your Groq API key is in the server (safe)
- Anyone with the URL can use your service
- They can see your project files in Code tab
- Consider adding authentication if needed

## Add Basic Authentication (Optional)

Edit `src/main.rs` to add password protection if exposing publicly.
