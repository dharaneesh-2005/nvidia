# Multi-Tunnel Setup Guide

## Overview

This setup allows 5 users to run the application simultaneously without conflicts. Each user selects their own tunnel during installation.

## One-Time Setup (Developer)

### Step 1: Create 5 Tunnels

Run this script once:
```bash
create_5_tunnels.bat
```

This creates:
1. `nvidia-dharaneesh`
2. `nvidia-steepan`
3. `nvidia-dinesh`
4. `nvidia-backup1`
5. `nvidia-backup2`

Credentials are saved in `tunnels/` folder.

### Step 2: Build Installer

```bash
build_complete.bat
```

The installer will include all 5 tunnel credentials.

## User Installation

When users run the installer, they will see:

### Page 1: Tunnel Selection
```
Choose your tunnel:
○ Dharaneesh (nvidia-dharaneesh)
○ Steepan (nvidia-steepan)
○ Dinesh (nvidia-dinesh)
○ Backup 1 (nvidia-backup1)
○ Backup 2 (nvidia-backup2)
```

### Page 2: Domain Selection
```
Choose your subdomain:
○ Helper subdomain (helper.pinmypic.online)
○ Custom subdomain
```

### Page 3: Custom Subdomain (if selected)
```
Enter subdomain name: [_______]
```

## How It Works

1. User selects tunnel (e.g., "Dharaneesh")
2. User selects subdomain (e.g., "john.pinmypic.online")
3. Installer copies the selected tunnel credentials to `credentials.json`
4. Installer creates `config.yml` with:
   - Tunnel: nvidia-dharaneesh
   - Hostname: john.pinmypic.online
5. User launches app
6. App connects using their specific tunnel

## Important Rules

- **Each user must select a DIFFERENT tunnel**
- If two users select the same tunnel, only one will work at a time
- Subdomains can be the same or different (doesn't matter)
- The tunnel selection is what prevents conflicts

## Example Scenario

**User 1 (Dharaneesh):**
- Tunnel: nvidia-dharaneesh
- Domain: dharaneesh.pinmypic.online
- ✅ Works

**User 2 (Steepan):**
- Tunnel: nvidia-steepan
- Domain: steepan.pinmypic.online
- ✅ Works simultaneously with User 1

**User 3 (Dinesh):**
- Tunnel: nvidia-dinesh
- Domain: dinesh.pinmypic.online
- ✅ Works simultaneously with Users 1 & 2

**User 4 (Backup1):**
- Tunnel: nvidia-backup1
- Domain: backup1.pinmypic.online
- ✅ Works simultaneously with all others

**User 5 (Backup2):**
- Tunnel: nvidia-backup2
- Domain: backup2.pinmypic.online
- ✅ Works simultaneously with all others

## Files Created

After installation, each user has:
```
C:\Program Files\Nvidia\
├── nvidia.exe
├── cloudflared.exe
├── credentials.json      ← Selected tunnel credentials
├── config.yml            ← Tunnel name + subdomain
├── domain.txt            ← Selected subdomain
├── tunnel.txt            ← Selected tunnel name
├── tunnels\
│   ├── nvidia-dharaneesh.json
│   ├── nvidia-steepan.json
│   ├── nvidia-dinesh.json
│   ├── nvidia-backup1.json
│   └── nvidia-backup2.json
└── static\
```

## Troubleshooting

### Problem: App not working on second device

**Cause:** Both devices selected the same tunnel

**Solution:** 
1. Uninstall on second device
2. Reinstall and select a DIFFERENT tunnel

### Problem: Need more than 5 users

**Solution:** Create more tunnels:
```bash
cloudflared tunnel create nvidia-user6
cloudflared tunnel create nvidia-user7
# etc.
```

Then add them to the installer.

### Problem: Forgot which tunnel a user selected

**Solution:** Check `tunnel.txt` in the installation directory:
```bash
type "C:\Program Files\Nvidia\tunnel.txt"
```

## Scaling Beyond 5 Users

To support more users:

1. Create additional tunnels:
   ```bash
   cloudflared tunnel create nvidia-user6
   cloudflared tunnel create nvidia-user7
   ```

2. Copy credentials to `tunnels/` folder

3. Update `setup.iss` to add more options in TunnelPage

4. Rebuild installer

## Benefits

✅ Up to 5 simultaneous users
✅ No conflicts between users
✅ No Cloudflare login during installation
✅ Each user has unique tunnel
✅ Easy to identify which user is which
✅ Can scale to more users by adding tunnels

## Summary

- **One-time setup:** Create 5 tunnels
- **User installation:** Select tunnel + subdomain
- **Result:** 5 users can run simultaneously
- **Limitation:** Maximum 5 concurrent users (expandable)
