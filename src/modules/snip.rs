use screenshots::Screen;
use image::{ImageFormat, DynamicImage, GenericImageView};
use base64::{Engine as _, engine::general_purpose};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::process::Command;

pub struct SnipTool;

impl SnipTool {
    #[cfg(windows)]
    pub fn capture_region() -> Result<String, String> {
        // Capture full screen
        let screens = Screen::all().map_err(|e| e.to_string())?;
        let screen = screens.first().ok_or("No screen found")?;
        let image = screen.capture().map_err(|e| e.to_string())?;
        
        // Save to temp file
        let temp_path = std::env::temp_dir().join("nvidia_snip_full.png");
        image.save(&temp_path).map_err(|e| e.to_string())?;
        
        // Launch PowerShell script for region selection
        let ps_script = format!(r#"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$img = [System.Drawing.Image]::FromFile('{}')
$form = New-Object System.Windows.Forms.Form
$form.WindowState = 'Maximized'
$form.FormBorderStyle = 'None'
$form.TopMost = $true
$form.BackColor = 'Black'
$form.Opacity = 0.3
$form.Cursor = [System.Windows.Forms.Cursors]::Cross

$startX = 0
$startY = 0
$endX = 0
$endY = 0
$isDrawing = $false
$rect = New-Object System.Drawing.Rectangle

$form.Add_MouseDown({{
    $script:startX = $_.X
    $script:startY = $_.Y
    $script:isDrawing = $true
}})

$form.Add_MouseMove({{
    if ($script:isDrawing) {{
        $script:endX = $_.X
        $script:endY = $_.Y
        $form.Invalidate()
    }}
}})

$form.Add_Paint({{
    param($sender, $e)
    if ($script:isDrawing) {{
        $x = [Math]::Min($script:startX, $script:endX)
        $y = [Math]::Min($script:startY, $script:endY)
        $w = [Math]::Abs($script:endX - $script:startX)
        $h = [Math]::Abs($script:endY - $script:startY)
        $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::Red, 2)
        $e.Graphics.DrawRectangle($pen, $x, $y, $w, $h)
        $pen.Dispose()
    }}
}})

$form.Add_MouseUp({{
    $script:endX = $_.X
    $script:endY = $_.Y
    $form.Close()
}})

$form.ShowDialog() | Out-Null

$x = [Math]::Min($startX, $endX)
$y = [Math]::Min($startY, $endY)
$w = [Math]::Abs($endX - $startX)
$h = [Math]::Abs($endY - $startY)

if ($w -gt 10 -and $h -gt 10) {{
    $crop = New-Object System.Drawing.Bitmap($w, $h)
    $graphics = [System.Drawing.Graphics]::FromImage($crop)
    $graphics.DrawImage($img, 0, 0, (New-Object System.Drawing.Rectangle($x, $y, $w, $h)), [System.Drawing.GraphicsUnit]::Pixel)
    $crop.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $crop.Dispose()
}}

$img.Dispose()
"#, temp_path.display(), std::env::temp_dir().join("nvidia_snip_crop.png").display());
        
        // Execute PowerShell
        let output = Command::new("powershell")
            .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
            .output()
            .map_err(|e| format!("Failed to launch snipping overlay: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Snipping failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        // Read cropped image
        let crop_path = std::env::temp_dir().join("nvidia_snip_crop.png");
        if !crop_path.exists() {
            return Err("No region selected".to_string());
        }
        
        let crop_bytes = std::fs::read(&crop_path).map_err(|e| e.to_string())?;
        let base64_image = general_purpose::STANDARD.encode(&crop_bytes);
        
        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
        let _ = std::fs::remove_file(&crop_path);
        
        Ok(base64_image)
    }
    
    #[cfg(not(windows))]
    pub fn capture_region() -> Result<String, String> {
        Err("Region selection only available on Windows".to_string())
    }
}
