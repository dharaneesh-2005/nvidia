# Cloudflare DNS Setup Guide

## Why Custom Domains Don't Work Automatically

When you choose a custom subdomain during installation (anything other than `helper.pinmypic.online`), the subdomain needs to be registered with Cloudflare's DNS system. This is done by adding a DNS route.

## What Works Out of the Box

- ✅ `helper.pinmypic.online` - Already configured during initial setup

## What Needs Additional Setup

- ❌ Any custom subdomain (e.g., `myapp.pinmypic.online`, `john.pinmypic.online`)
- ❌ Root domain (`pinmypic.online`)

## How to Add Custom Subdomain

### Method 1: During Installation (Recommended)

When you install and choose a custom subdomain, the installer will ask:

```
Would you like to add the DNS route for [your-domain] to Cloudflare now?
```

Click **Yes** to automatically add the DNS route.

### Method 2: After Installation

If you skipped the DNS setup during installation, run:

```bash
cd "C:\Program Files\Nvidia"
add_custom_domain.bat
```

This will add the DNS route for the domain specified in `domain.txt`.

### Method 3: Manual Setup via Command Line

```bash
cd "C:\Program Files\Nvidia"
cloudflared.exe tunnel route dns interview-helper [your-subdomain].pinmypic.online
```

Example:
```bash
cloudflared.exe tunnel route dns interview-helper myapp.pinmypic.online
```

### Method 4: Manual Setup via Cloudflare Dashboard

If the command-line methods fail, add the DNS record manually:

1. Go to [Cloudflare Dashboard](https://dash.cloudflare.com)
2. Select domain: `pinmypic.online`
3. Go to **DNS** → **Records**
4. Click **Add record**
5. Configure:
   - **Type**: `CNAME`
   - **Name**: `[your-subdomain]` (e.g., `myapp`)
   - **Target**: `[tunnel-id].cfargotunnel.com`
   - **Proxy status**: Proxied (orange cloud)
6. Click **Save**

To find your tunnel ID:
```bash
cloudflared.exe tunnel list
```

Look for `interview-helper` and copy the ID.

## Troubleshooting

### Error: "tunnel route dns failed"

**Cause**: Cloudflare credentials not configured or domain not in your account.

**Solution**:
1. Ensure `pinmypic.online` is added to your Cloudflare account
2. Run the initial setup: `setup_cloudflare.bat`
3. Try adding the DNS route again

### Error: "domain not found"

**Cause**: The domain `pinmypic.online` is not in your Cloudflare account.

**Solution**:
1. Add `pinmypic.online` to your Cloudflare account
2. Or use a different base domain you own

### Custom domain not accessible

**Cause**: DNS not propagated yet or DNS route not added.

**Solution**:
1. Wait 2-5 minutes for DNS propagation
2. Verify DNS route exists:
   ```bash
   cloudflared.exe tunnel route dns interview-helper [your-domain]
   ```
3. Check Cloudflare DNS records in dashboard

### "helper.pinmypic.online" works but custom doesn't

**Cause**: DNS route not added for custom subdomain.

**Solution**: Run `add_custom_domain.bat` or add DNS route manually.

## DNS Propagation Time

After adding a DNS route:
- **Cloudflare**: Usually instant to 2 minutes
- **Global DNS**: Can take up to 5 minutes
- **Your ISP**: May cache old records for up to 24 hours

To test immediately, use:
```bash
nslookup [your-subdomain].pinmypic.online 1.1.1.1
```

## Important Notes

1. **One-time setup**: Each custom subdomain needs DNS route added once
2. **Credentials required**: Must have Cloudflare credentials configured
3. **Domain ownership**: `pinmypic.online` must be in your Cloudflare account
4. **Helper subdomain**: Already configured, no additional setup needed

## Quick Reference

| Subdomain | DNS Setup Required? | How to Setup |
|-----------|---------------------|--------------|
| helper.pinmypic.online | ❌ No | Already configured |
| Custom (e.g., myapp.pinmypic.online) | ✅ Yes | Run `add_custom_domain.bat` |
| Root (pinmypic.online) | ✅ Yes | Run `add_custom_domain.bat` |

## Example Workflow

### User chooses "myapp" as custom subdomain:

1. **During installation**:
   - Installer updates `config.yml` with `myapp.pinmypic.online`
   - Installer creates `domain.txt` with `myapp.pinmypic.online`
   - Installer asks to add DNS route → User clicks Yes
   - DNS route added automatically

2. **After installation**:
   - User launches app
   - App accessible at `https://myapp.pinmypic.online`

### If DNS setup was skipped:

1. User launches app
2. App not accessible (DNS not configured)
3. User runs `add_custom_domain.bat`
4. DNS route added
5. Wait 2-5 minutes
6. App now accessible at `https://myapp.pinmypic.online`

## Support

If you continue to have issues:

1. Verify Cloudflare credentials:
   ```bash
   cloudflared.exe tunnel list
   ```
   Should show `interview-helper` tunnel

2. Check config.yml has correct domain:
   ```bash
   type "C:\Program Files\Nvidia\config.yml"
   ```

3. Verify DNS route exists in Cloudflare dashboard

4. Test DNS resolution:
   ```bash
   nslookup [your-domain] 1.1.1.1
   ```
