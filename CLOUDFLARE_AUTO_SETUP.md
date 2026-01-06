# Automated Cloudflare Setup

## Choose Your Setup:

### Option 1: With Custom Domain (pinmypic.online)
Run: `setup_cloudflare.bat`

**Requirements:**
- Cloudflare account (free)
- Domain managed by Cloudflare

**Result:** https://pinmypic.online

---

### Option 2: Free Subdomain (Recommended)
Run: `setup_cloudflare_simple.bat`

**Requirements:**
- Cloudflare account (free) - create at cloudflare.com

**Result:** https://xxxxx-xxxx.trycloudflare.com (permanent)

---

## Steps:

1. **Run the setup script:**
   ```
   setup_cloudflare_simple.bat
   ```

2. **Login when browser opens:**
   - Create free account if needed
   - Authorize cloudflared

3. **Wait for completion:**
   - Script creates tunnel
   - Copies credentials
   - Creates config

4. **Done!**
   - Files are ready
   - Build your installer

---

## What Gets Created:

```
✅ cloudflared.exe       (tunnel client)
✅ credentials.json      (your tunnel auth)
✅ config.yml           (tunnel config)
```

These 3 files go into your installer - users never touch them!

---

## After Setup:

1. Build installer: `iscc setup.iss`
2. Test it
3. Distribute!

Users just install and run - the tunnel auto-connects using your embedded credentials.

---

## Troubleshooting:

**Q: Login fails?**
- Make sure you have internet
- Try running as administrator

**Q: Tunnel already exists?**
- That's fine! Script will use existing tunnel

**Q: Want to reset?**
- Delete credentials.json and config.yml
- Run setup again

---

## Time Required:

- First time: 2 minutes
- Already have account: 30 seconds

Much simpler than manual setup!
