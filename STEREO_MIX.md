# Enable Stereo Mix (System Audio Capture)

## Windows Setup

### Step 1: Enable Stereo Mix

1. **Right-click** the speaker icon in taskbar
2. Click **"Sounds"** or **"Sound settings"**
3. Click **"Sound Control Panel"** (or go to Recording tab)
4. In **Recording** tab:
   - Right-click in empty space
   - Check **"Show Disabled Devices"**
   - You should see **"Stereo Mix"**
5. **Right-click "Stereo Mix"** → **Enable**
6. **Right-click "Stereo Mix"** → **Set as Default Device**
7. Click **OK**

### Step 2: Rebuild and Run

```bash
cargo build --release
cargo run --release
```

You should see: `Using audio device: Stereo Mix`

## If Stereo Mix is Missing

Some audio drivers don't include Stereo Mix. Alternatives:

### Option 1: Update Audio Drivers
- Update Realtek/audio drivers from manufacturer
- Stereo Mix often appears after driver update

### Option 2: Use VB-Audio Virtual Cable (Free)
1. Download: https://vb-audio.com/Cable/
2. Install VB-CABLE
3. Set VB-CABLE as default playback device
4. App will capture from VB-CABLE Input

### Option 3: Use VoiceMeeter (Free)
1. Download: https://vb-audio.com/Voicemeeter/
2. Route system audio through VoiceMeeter
3. App captures VoiceMeeter output

## Verify It's Working

When you play audio/video:
- App should show: "Transcription: [audio content]"
- Check console logs for audio capture messages

## Troubleshooting

**No audio captured:**
- Ensure Stereo Mix is enabled AND set as default
- Play some audio/video to test
- Check volume levels in Sound settings

**Still using microphone:**
- Disable microphone in Recording devices
- Set Stereo Mix as default recording device
