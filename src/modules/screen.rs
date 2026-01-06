use screenshots::Screen;
use image::ImageFormat;
use base64::{Engine as _, engine::general_purpose};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use winapi::um::winuser::{RegisterHotKey, MOD_CONTROL, MOD_ALT};

pub struct ScreenCapture {
    captured: Arc<Mutex<Option<String>>>,
}

impl ScreenCapture {
    pub fn new(_hotkey_str: String) -> Self {
        let captured = Arc::new(Mutex::new(None));
        let captured_clone = captured.clone();
        
        println!("[ScreenCapture] Registering Ctrl+Alt+X hotkey");
        
        #[cfg(windows)]
        std::thread::spawn(move || {
            use winapi::um::winuser::{GetMessageW, TranslateMessage, DispatchMessageW, MSG, WM_HOTKEY, MOD_ALT};
            use winapi::um::errhandlingapi::GetLastError;
            
            const SCREENSHOT_HOTKEY_ID: i32 = 1;
            const SEARCH_HOTKEY_ID: i32 = 2;
            
            unsafe {
                // Register Ctrl+Alt+X
                let result = RegisterHotKey(
                    std::ptr::null_mut(),
                    SCREENSHOT_HOTKEY_ID,
                    (MOD_CONTROL | MOD_ALT) as u32,
                    'X' as u32,
                );
                
                if result != 0 {
                    println!("[ScreenCapture] ✓ Hotkey Ctrl+Alt+X registered successfully!");
                    
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
                                if msg.wParam == SCREENSHOT_HOTKEY_ID as usize {
                                    println!("[ScreenCapture] ✓✓✓ Hotkey pressed! Capturing screen...");
                                    if let Ok(image_data) = Self::capture_screen() {
                                        println!("[ScreenCapture] ✓ Screen captured, storing in mutex");
                                        let mut cap = captured_clone.lock().unwrap();
                                        *cap = Some(image_data);
                                    }
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
        
        Self { captured }
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
}
