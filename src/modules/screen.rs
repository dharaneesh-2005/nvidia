use screenshots::Screen;
use image::ImageFormat;
use base64::{Engine as _, engine::general_purpose};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use winapi::um::winuser::{RegisterHotKey, MOD_CONTROL, MOD_SHIFT};

pub struct ScreenCapture {
    captured: Arc<Mutex<Option<String>>>,
    multi_captured: Arc<Mutex<Option<String>>>,
    cancel_triggered: Arc<Mutex<bool>>,
}

impl ScreenCapture {
    pub fn new(_hotkey_str: String) -> Self {
        let captured = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();
        let multi_captured = Arc::new(Mutex::new(None));
        let multi_captured_clone = multi_captured.clone();
        let cancel_triggered = Arc::new(Mutex::new(false));
        let cancel_triggered_clone = cancel_triggered.clone();
        
        println!("[ScreenCapture] Registering Ctrl+Alt+X hotkey");
        println!("[ScreenCapture] Registering Ctrl+Alt+C hotkey for multi-capture");
        println!("[ScreenCapture] Registering Ctrl+Alt+Z hotkey to cancel multi-capture");
        
        #[cfg(windows)]
        std::thread::spawn(move || {
            use winapi::um::winuser::{GetMessageW, TranslateMessage, DispatchMessageW, MSG, WM_HOTKEY, MOD_ALT};
            use winapi::um::errhandlingapi::GetLastError;
            
            const SCREENSHOT_HOTKEY_ID: i32 = 1;
            const SEARCH_HOTKEY_ID: i32 = 2;
            const MULTI_CAPTURE_HOTKEY_ID: i32 = 4;
            const MULTI_CANCEL_HOTKEY_ID: i32 = 5;
            const SCREENSHOT_SHIFT_HOTKEY_ID: i32 = 6;
            
            unsafe {
                // Register Ctrl+Alt+X
                let result = RegisterHotKey(
                    std::ptr::null_mut(),
                    SCREENSHOT_HOTKEY_ID,
                    (MOD_CONTROL | MOD_ALT) as u32,
                    'X' as u32,
                );
                
                if result != 0 {
                    // Register Ctrl+Shift+X as alternate finalize hotkey
                    let shift_result = RegisterHotKey(
                        std::ptr::null_mut(),
                        SCREENSHOT_SHIFT_HOTKEY_ID,
                        (MOD_CONTROL | MOD_SHIFT) as u32,
                        'X' as u32,
                    );
                    
                    if shift_result != 0 {
                        println!("[ScreenCapture] Hotkey Ctrl+Shift+X registered as alternate finalize!");
                    }
                    println!("[ScreenCapture] ✓ Hotkey Ctrl+Alt+X registered successfully!");
                    
                    // Register Ctrl+Alt+C for multi-capture
                    let multi_result = RegisterHotKey(
                        std::ptr::null_mut(),
                        MULTI_CAPTURE_HOTKEY_ID,
                        (MOD_CONTROL | MOD_ALT) as u32,
                        'C' as u32,
                    );
                    
                    if multi_result != 0 {
                        println!("[ScreenCapture] âœ“ Hotkey Ctrl+Alt+C registered for multi-capture!");
                    }
                    
                    // Register Ctrl+Alt+Z to cancel multi-capture
                    let cancel_result = RegisterHotKey(
                        std::ptr::null_mut(),
                        MULTI_CANCEL_HOTKEY_ID,
                        (MOD_CONTROL | MOD_ALT) as u32,
                        'Z' as u32,
                    );
                    
                    if cancel_result != 0 {
                        println!("[ScreenCapture] Hotkey Ctrl+Alt+Z registered to cancel multi-capture!");
                    }
                    
                    // Also register Ctrl+Alt+S for search
                    let search_result = RegisterHotKey(
                        std::ptr::null_mut(),
                        SEARCH_HOTKEY_ID,
                        (MOD_CONTROL | MOD_ALT) as u32,
                        'S' as u32,
                    );
                    
                    if search_result != 0 {
                        println!("[ScreenCapture] ✓ Hotkey Ctrl+Alt+S registered for manual questions!");
                    }
                    
                    let mut msg: MSG = std::mem::zeroed();
                    loop {
                        let msg_result = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                        if msg_result > 0 {
                            if msg.message == WM_HOTKEY {
                                if msg.wParam == SCREENSHOT_HOTKEY_ID as usize || msg.wParam == SCREENSHOT_SHIFT_HOTKEY_ID as usize {
                                    println!("[ScreenCapture] ✓✓✓ Hotkey pressed! Capturing screen...");
                                    if let Ok(image_data) = Self::capture_screen() {
                                        println!("[ScreenCapture] ✓ Screen captured, storing in mutex");
                                        let mut cap = captured_clone.lock().unwrap();
                                        *cap = Some(image_data);
                                    }
                                } else if msg.wParam == MULTI_CAPTURE_HOTKEY_ID as usize {
                                    println!("[ScreenCapture] âœ“âœ“âœ“ Multi-capture hotkey pressed! Capturing screen chunk...");
                                    if let Ok(image_data) = Self::capture_screen() {
                                        println!("[ScreenCapture] âœ“ Screen chunk captured, storing in multi buffer");
                                        let mut cap = multi_captured_clone.lock().unwrap();
                                        *cap = Some(image_data);
                                    }
                                } else if msg.wParam == MULTI_CANCEL_HOTKEY_ID as usize {
                                    println!("[ScreenCapture] Multi-capture cancel hotkey pressed");
                                    let mut flag = cancel_triggered_clone.lock().unwrap();
                                    *flag = true;
                                } else if msg.wParam == SEARCH_HOTKEY_ID as usize {
                                    println!("[ScreenCapture] ✓ Search hotkey pressed - modal will open in browser");
                                    // The browser will handle opening the modal via JavaScript
                                }
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                } else {
                    let error_code = GetLastError();
                    println!("[ScreenCapture] ✗ Failed to register hotkey! Error code: {}", error_code);
                    println!("[ScreenCapture] Error 1409 means hotkey is already registered by another app");
                }
            }
        });
        
        Self { captured, multi_captured, cancel_triggered }
    }
    
    pub fn capture_now() -> Result<String, String> {
        Self::capture_screen()
    }
    
    fn capture_screen() -> Result<String, String> {
        let screens = Screen::all().map_err(|e| e.to_string())?;
        let screen = screens.first().ok_or("No screen found")?;
        
        let image = screen.capture().map_err(|e| e.to_string())?;
        
        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        image.write_to(&mut cursor, ImageFormat::Png).map_err(|e| e.to_string())?;
        
        let base64_image = general_purpose::STANDARD.encode(&buffer);
        Ok(base64_image)
    }
    
    pub fn check_capture(&mut self) -> Option<String> {
        let mut cap = self.captured.lock().unwrap();
        cap.take()
    }

    pub fn check_multi_capture(&mut self) -> Option<String> {
        let mut cap = self.multi_captured.lock().unwrap();
        cap.take()
    }

    pub fn check_cancel(&mut self) -> bool {
        let mut flag = self.cancel_triggered.lock().unwrap();
        if *flag {
            *flag = false;
            true
        } else {
            false
        }
    }
}



