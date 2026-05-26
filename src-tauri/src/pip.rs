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

    // Calculate default position (center of screen)
    let window_x = x.unwrap_or_else(|| {
        // Get primary monitor size and position window in center
        if let Some(monitor) = app.primary_monitor().ok().flatten() {
            let monitor_size = monitor.size();
            let monitor_pos = monitor.position();
            (monitor_pos.x as f64) + ((monitor_size.width as f64) - window_width) / 2.0
        } else {
            100.0
        }
    });

    let window_y = y.unwrap_or_else(|| {
        if let Some(monitor) = app.primary_monitor().ok().flatten() {
            let monitor_size = monitor.size();
            let monitor_pos = monitor.position();
            (monitor_pos.y as f64) + ((monitor_size.height as f64) - window_height) / 2.0
        } else {
            100.0
        }
    });

    println!("[PiP] Creating PiP window at ({}, {}) with size {}x{}", 
        window_x, window_y, window_width, window_height);

    // Create the PiP window HIDDEN initially to prevent white flash
    // The window will be shown after WebView2 finishes loading content
    let pip_window = WebviewWindowBuilder::new(
        &app,
        "pip",
        WebviewUrl::External(format!("http://localhost:5000/?pip=true").parse().unwrap()),
    )
    .title("Nvidia PiP")
    .inner_size(window_width, window_height)
    .position(window_x, window_y)
    .always_on_top(true)
    .decorations(true)           // Enable title bar for easy dragging
    .resizable(true)
    .skip_taskbar(true)          // Hide from taskbar
    .visible(false)              // Start hidden - show after content loads
    .focused(false)              // Don't steal focus when created
    .build()
    .map_err(|e| format!("Failed to create PiP window: {}", e))?;

    println!("[PiP] Window created hidden, waiting for content to load...");

    // Apply screen capture exclusion
    #[cfg(target_os = "windows")]
    {
        apply_capture_exclusion_windows(&pip_window)?;
        apply_no_activate_windows(&pip_window)?;
        force_static_cursor_windows(&pip_window)?;
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

    // Fallback: show the window after a delay WITHOUT stealing focus
    let pip_window_clone = pip_window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(800));
        // Only show if still hidden
        if let Ok(visible) = pip_window_clone.is_visible() {
            if !visible {
                // Use Windows API ShowWindow with SW_SHOWNOACTIVATE to avoid focus steal
                #[cfg(target_os = "windows")]
                {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        ShowWindow, SW_SHOWNOACTIVATE,
                        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE,
                        WS_EX_NOACTIVATE, GetWindow, GW_CHILD, GW_HWNDNEXT,
                    };
                    use windows::Win32::Foundation::HWND;
                    
                    if let Ok(hwnd) = pip_window_clone.hwnd() {
                        let hwnd_ptr = hwnd.0 as *mut core::ffi::c_void;
                        unsafe {
                            let hwnd_win = HWND(hwnd_ptr);
                            ShowWindow(hwnd_win, SW_SHOWNOACTIVATE);
                            
                            // Re-apply WS_EX_NOACTIVATE to child windows
                            let mut child = GetWindow(hwnd_win, GW_CHILD);
                            while let Ok(child_hwnd) = child {
                                let style = GetWindowLongPtrW(child_hwnd, GWL_EXSTYLE);
                                let new_style = style | (WS_EX_NOACTIVATE.0 as isize);
                                SetWindowLongPtrW(child_hwnd, GWL_EXSTYLE, new_style);
                                child = GetWindow(child_hwnd, GW_HWNDNEXT);
                            }
                        }
                        println!("[PiP] ✓ Window shown + no-activate re-applied to children");
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = pip_window_clone.show();
                    println!("[PiP] ✓ Window shown via fallback timer");
                }
            }
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

/// Called by pip.html when content is fully rendered and ready to display
#[tauri::command]
pub async fn pip_content_ready(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("pip") {
        window.show().map_err(|e| format!("Failed to show PiP window: {}", e))?;
        println!("[PiP] ✓ Content ready signal received, window shown");
    }
    Ok(())
}

/// Toggle the PiP window (close if open, create if closed)
/// 
/// Note: We close and recreate instead of hide/show because Windows doesn't
/// preserve WDA_EXCLUDEFROMCAPTURE and WS_EX_NOACTIVATE flags when showing
/// a hidden window. This ensures screen capture exclusion and no-focus behavior
/// work correctly every time.
#[tauri::command]
pub async fn toggle_pip_window(app: AppHandle) -> Result<String, String> {
    if let Some(window) = app.get_webview_window("pip") {
        // Window exists, close it (don't just hide)
        // This ensures flags are properly re-applied on next open
        window.close().map_err(|e| format!("Failed to close PiP window: {}", e))?;
        PIP_OPEN.store(false, Ordering::SeqCst);
        println!("[PiP] Window closed (toggle off)");
        Ok("PiP window closed".to_string())
    } else {
        // Window doesn't exist, create it
        // This will apply all necessary flags (WDA_EXCLUDEFROMCAPTURE, WS_EX_NOACTIVATE)
        println!("[PiP] Creating new window (toggle on)");
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

/// Apply Windows WS_EX_NOACTIVATE to prevent focus stealing
#[cfg(target_os = "windows")]
fn apply_no_activate_windows(window: &tauri::WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, 
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        GetWindow, GW_CHILD, GW_HWNDNEXT,
    };
    use windows::Win32::Foundation::HWND;

    let hwnd = window.hwnd().map_err(|e| format!("Failed to get window handle: {}", e))?;
    let hwnd_ptr = hwnd.0 as *mut core::ffi::c_void;

    unsafe {
        let hwnd_win = HWND(hwnd_ptr);
        
        // Apply to main window
        let current_style = GetWindowLongPtrW(hwnd_win, GWL_EXSTYLE);
        let new_style = current_style 
            | (WS_EX_NOACTIVATE.0 as isize)
            | (WS_EX_TOOLWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd_win, GWL_EXSTYLE, new_style);
        
        // Also apply to ALL child windows using GetWindow traversal
        let mut child = GetWindow(hwnd_win, GW_CHILD);
        while let Ok(child_hwnd) = child {
            let style = GetWindowLongPtrW(child_hwnd, GWL_EXSTYLE);
            let child_new_style = style | (WS_EX_NOACTIVATE.0 as isize);
            SetWindowLongPtrW(child_hwnd, GWL_EXSTYLE, child_new_style);
            child = GetWindow(child_hwnd, GW_HWNDNEXT);
        }
        
        println!("[PiP] ✓ WS_EX_NOACTIVATE + WS_EX_TOOLWINDOW applied to window + children");
        println!("[PiP] You can click on the PiP window without losing focus on your browser");
    }
    
    Ok(())
}

/// Force cursor to always be the standard arrow (no hand/resize/text cursors)
/// This sets the window class cursor at the Windows API level
#[cfg(target_os = "windows")]
fn force_static_cursor_windows(window: &tauri::WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SetClassLongPtrW, LoadCursorW, SetCursor, GCLP_HCURSOR, IDC_ARROW,
    };
    use windows::Win32::Foundation::HWND;

    let hwnd = window.hwnd().map_err(|e| format!("Failed to get window handle: {}", e))?;
    let hwnd_ptr = hwnd.0 as *mut core::ffi::c_void;

    unsafe {
        let hwnd_win = HWND(hwnd_ptr);
        
        // Load the standard arrow cursor from system
        let arrow_cursor = LoadCursorW(None, IDC_ARROW)
            .map_err(|e| format!("Failed to load arrow cursor: {}", e))?;
        
        // Set the window class cursor to always be the arrow
        SetClassLongPtrW(hwnd_win, GCLP_HCURSOR, arrow_cursor.0 as isize);
        
        // Also set the current cursor immediately
        SetCursor(Some(arrow_cursor));
        
        println!("[PiP] ✓ Static cursor applied at Windows API level");
        println!("[PiP] CSS will handle WebView cursor enforcement");
    }
    
    Ok(())
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
        SetClassLongPtrW, GCLP_HCURSOR,
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

/// Set PiP window opacity (0.0 = fully transparent, 1.0 = fully opaque)
/// Uses Windows Layered Window API for transparency
#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn set_pip_opacity(app: AppHandle, opacity: f64) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, SetLayeredWindowAttributes,
        GWL_EXSTYLE, LWA_ALPHA, WS_EX_LAYERED,
    };
    use windows::Win32::Foundation::{HWND, COLORREF};

    if let Some(window) = app.get_webview_window("pip") {
        // Clamp opacity between 0.1 and 1.0 (prevent fully invisible window)
        let clamped_opacity = opacity.max(0.1).min(1.0);
        
        // Convert opacity to alpha value (0-255)
        let alpha = (clamped_opacity * 255.0) as u8;
        
        let hwnd = window.hwnd().map_err(|e| format!("Failed to get window handle: {}", e))?;
        let hwnd_ptr = hwnd.0 as *mut core::ffi::c_void;
        
        unsafe {
            let hwnd_win = HWND(hwnd_ptr);
            
            // Get current extended window styles
            let current_style = GetWindowLongW(hwnd_win, GWL_EXSTYLE);
            
            // Add WS_EX_LAYERED flag if not present (required for transparency)
            let new_style = current_style | WS_EX_LAYERED.0 as i32;
            SetWindowLongW(hwnd_win, GWL_EXSTYLE, new_style);
            
            // Set the alpha value (crKey is not used, set to 0)
            let result = SetLayeredWindowAttributes(hwnd_win, COLORREF(0), alpha, LWA_ALPHA);
            
            if result.is_ok() {
                println!("[PiP] Opacity set to {:.2} (alpha: {})", clamped_opacity, alpha);
                Ok(())
            } else {
                Err("Failed to set layered window attributes".to_string())
            }
        }
    } else {
        Err("PiP window not found".to_string())
    }
}

/// Set PiP window opacity (macOS/Linux fallback - not implemented)
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn set_pip_opacity(_app: AppHandle, opacity: f64) -> Result<(), String> {
    println!("[PiP] Opacity control not implemented for this platform");
    Ok(())
}
