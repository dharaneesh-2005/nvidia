//! Picture-in-Picture Window Module
//!
//! This module provides a native PiP window that:
//! - Floats always-on-top
//! - Is excluded from screen capture (invisible to OBS, Zoom, GMeet, etc.)
//! - Loads the pip.html content via WebView2
//! - Maintains WebSocket connection for real-time updates

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

static PIP_OPEN: AtomicBool = AtomicBool::new(false);

/// Get the current PiP window state (Tauri command)
#[tauri::command]
pub fn get_pip_state() -> bool {
    PIP_OPEN.load(Ordering::SeqCst)
}

/// Open the native PiP window with screen capture exclusion
#[tauri::command]
pub async fn open_pip_window(
    app: AppHandle,
    width: Option<f64>,
    height: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<String, String> {
    if PIP_OPEN.load(Ordering::SeqCst) {
        return Ok("PiP window already open".to_string());
    }

    let window_width = width.unwrap_or(400.0);
    let window_height = height.unwrap_or(350.0);

    // Calculate default position (bottom-right corner)
    let window_x = x.unwrap_or_else(|| {
        // Get primary monitor size and position window in bottom-right
        if let Some(monitor) = app.primary_monitor().ok().flatten() {
            let monitor_size = monitor.size();
            let monitor_pos = monitor.position();
            (monitor_pos.x as f64) + (monitor_size.width as f64) - window_width - 20.0
        } else {
            100.0
        }
    });

    let window_y = y.unwrap_or_else(|| {
        if let Some(monitor) = app.primary_monitor().ok().flatten() {
            let monitor_size = monitor.size();
            let monitor_pos = monitor.position();
            (monitor_pos.y as f64) + (monitor_size.height as f64) - window_height - 40.0
        } else {
            100.0
        }
    });

    println!("[PiP] Creating PiP window at ({}, {}) with size {}x{}", 
        window_x, window_y, window_width, window_height);

    // Create the PiP window with decorations for easy moving
    let pip_window = WebviewWindowBuilder::new(
        &app,
        "pip",
        WebviewUrl::External(format!("http://localhost:5000/").parse().unwrap()),
    )
    .title("Nvidia PiP")
    .inner_size(window_width, window_height)
    .position(window_x, window_y)
    .always_on_top(true)
    .decorations(true)           // Enable title bar/borders for easy moving
    .resizable(true)
    .skip_taskbar(true)          // Hide from taskbar
    .visible(true)
    .build()
    .map_err(|e| format!("Failed to create PiP window: {}", e))?;

    println!("[PiP] Window created with title bar for easy moving");

    // Apply screen capture exclusion
    #[cfg(target_os = "windows")]
    {
        apply_capture_exclusion_windows(&pip_window)?;
    }

    #[cfg(target_os = "macos")]
    {
        apply_capture_exclusion_macos(&pip_window)?;
    }

    PIP_OPEN.store(true, Ordering::SeqCst);

    // Listen for window close to reset the flag
    let _app_handle = app.clone();
    pip_window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            PIP_OPEN.store(false, Ordering::SeqCst);
            println!("[PiP] PiP window closed");
        }
    });

    println!("[PiP] ✓ PiP window created with screen capture exclusion");
    Ok("PiP window opened successfully".to_string())
}

/// Close the PiP window
#[tauri::command]
pub async fn close_pip_window(app: AppHandle) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("pip") {
        window.close().map_err(|e| format!("Failed to close PiP window: {}", e))?;
        PIP_OPEN.store(false, Ordering::SeqCst);
        Ok("PiP window closed".to_string())
    } else {
        Ok("PiP window not found".to_string())
    }
}

/// Toggle the PiP window (hide if visible, show if hidden)
#[tauri::command]
pub async fn toggle_pip_window(app: AppHandle) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("pip") {
        // Window exists, toggle visibility
        match window.is_visible() {
            Ok(true) => {
                // Window is visible, hide it
                window.hide().map_err(|e| format!("Failed to hide PiP window: {}", e))?;
                println!("[PiP] Window hidden");
                Ok("PiP window hidden".to_string())
            }
            Ok(false) => {
                // Window is hidden, show it
                window.show().map_err(|e| format!("Failed to show PiP window: {}", e))?;
                window.set_focus().map_err(|e| format!("Failed to focus PiP window: {}", e))?;
                println!("[PiP] Window shown");
                Ok("PiP window shown".to_string())
            }
            Err(e) => Err(format!("Failed to check window visibility: {}", e))
        }
    } else {
        // Window doesn't exist, create it
        open_pip_window(app, None, None, None, None).await
    }
}

/// Check if PiP window is currently open
#[tauri::command]
pub fn is_pip_open() -> bool {
    PIP_OPEN.load(Ordering::SeqCst)
}

/// Minimize the PiP window
#[tauri::command]
pub async fn minimize_pip_window(app: AppHandle) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("pip") {
        window.minimize().map_err(|e| format!("Failed to minimize PiP window: {}", e))?;
        Ok("PiP window minimized".to_string())
    } else {
        Ok("PiP window not found".to_string())
    }
}

/// Apply Windows SetWindowDisplayAffinity for screen capture exclusion
#[cfg(target_os = "windows")]
fn apply_capture_exclusion_windows(window: &tauri::WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
    };

    // Get the native window handle as isize (HWND)
    let hwnd = window.hwnd().map_err(|e| format!("Failed to get window handle: {}", e))?;
    let hwnd_ptr = hwnd.0 as *mut core::ffi::c_void;

    unsafe {
        // WDA_EXCLUDEFROMCAPTURE makes the window invisible to screen capture
        // This works on Windows 10 2004+ and Windows 11
        // Convert to the expected HWND type
        let hwnd_win = windows::Win32::Foundation::HWND(hwnd_ptr);
        let result = SetWindowDisplayAffinity(hwnd_win, WDA_EXCLUDEFROMCAPTURE);
        
        if result.is_ok() {
            println!("[PiP] ✓ Screen capture exclusion applied (SetWindowDisplayAffinity)");
            println!("[PiP] Window is now invisible to: OBS, Zoom, GMeet, Teams, etc.");
            Ok(())
        } else {
            let error_code = windows::Win32::Foundation::GetLastError();
            println!("[PiP] ⚠ Failed to apply capture exclusion: {:?}", error_code);
            // Don't fail - window still works, just visible to capture
            Ok(())
        }
    }
}



/// Apply macOS screen capture exclusion
#[cfg(target_os = "macos")]
fn apply_capture_exclusion_macos(window: &tauri::WebviewWindow) -> Result<(), String> {
    // On macOS, we use NSWindow's setSharingType to exclude from screen capture
    // NSWindowSharingNone = 0 means the window contents are not readable by other processes
    
    use cocoa::appkit::NSWindow;
    use cocoa::base::{id, nil};
    use objc::runtime::{Object, Sel};
    use objc::{msg_send, sel, sel_impl};

    unsafe {
        let ns_window = window.ns_window().map_err(|e| format!("Failed to get NSWindow: {}", e))?;
        let ns_window_id = ns_window as id;
        
        if ns_window_id != nil {
            // Set sharing type to 0 (NSWindowSharingNone)
            // This makes the window invisible to screen capture
            let _: () = msg_send![ns_window_id, setSharingType: 0u64];
            println!("[PiP] ✓ Screen capture exclusion applied (setSharingType: 0)");
        }
    }
    
    Ok(())
}

/// Get the current screen dimensions for positioning
#[tauri::command]
pub fn get_screen_dimensions(app: AppHandle) -> Result<ScreenDimensions, String> {
    if let Some(monitor) = app.primary_monitor().ok().flatten() {
        let size = monitor.size();
        let pos = monitor.position();
        
        Ok(ScreenDimensions {
            width: size.width as f64,
            height: size.height as f64,
            x: pos.x as f64,
            y: pos.y as f64,
        })
    } else {
        Err("Failed to get screen dimensions".to_string())
    }
}

#[derive(serde::Serialize)]
pub struct ScreenDimensions {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}

/// Update PiP window position
#[tauri::command]
pub async fn update_pip_position(
    app: AppHandle,
    x: f64,
    y: f64,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("pip") {
        window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: x as i32,
            y: y as i32,
        })).map_err(|e| format!("Failed to update position: {}", e))?;
        Ok(())
    } else {
        Err("PiP window not found".to_string())
    }
}

/// Update PiP window size
#[tauri::command]
pub async fn update_pip_size(
    app: AppHandle,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("pip") {
        window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: width as u32,
            height: height as u32,
        })).map_err(|e| format!("Failed to update size: {}", e))?;
        Ok(())
    } else {
        Err("PiP window not found".to_string())
    }
}

/// Hide cursor for PiP window (Windows only)
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn hide_pip_cursor(app: AppHandle) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetClassLongPtrW, GCLP_HCURSOR, GetClassLongPtrW,
    };
    use windows::Win32::Foundation::HWND;

    if let Some(window) = app.get_webview_window("pip") {
        let hwnd = window.hwnd().map_err(|e| format!("Failed to get window handle: {}", e))?;
        let hwnd_ptr = hwnd.0 as *mut core::ffi::c_void;

        unsafe {
            let hwnd_win = HWND(hwnd_ptr);
            // Set cursor to NULL (invisible)
            SetClassLongPtrW(hwnd_win, GCLP_HCURSOR, 0);
            println!("[PiP] Cursor hidden");
        }
        Ok(())
    } else {
        Err("PiP window not found".to_string())
    }
}

/// Show cursor for PiP window (Windows only)
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn show_pip_cursor(_app: AppHandle) -> Result<(), String> {
    // Cursor restoration is handled by the OS when the window is destroyed
    // CSS cursor styling is handled by the frontend
    println!("[PiP] Cursor show requested (handled by CSS)");
    Ok(())
}
