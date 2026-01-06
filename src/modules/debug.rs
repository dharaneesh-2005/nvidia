use std::sync::{Arc, Mutex};

#[cfg(windows)]
use winapi::um::winuser::{RegisterHotKey, MOD_CONTROL, MOD_ALT};

pub struct DebugHotkey {
    triggered: Arc<Mutex<bool>>,
}

impl DebugHotkey {
    pub fn new() -> Self {
        let triggered = Arc::new(Mutex::new(false));
        let triggered_clone = triggered.clone();
        
        println!("[DebugHotkey] Registering Ctrl+Alt+D hotkey for code debugging");
        
        #[cfg(windows)]
        std::thread::spawn(move || {
            use winapi::um::winuser::{GetMessageW, TranslateMessage, DispatchMessageW, MSG, WM_HOTKEY};
            use winapi::um::errhandlingapi::GetLastError;
            
            const DEBUG_HOTKEY_ID: i32 = 3;
            
            unsafe {
                let result = RegisterHotKey(
                    std::ptr::null_mut(),
                    DEBUG_HOTKEY_ID,
                    (MOD_CONTROL | MOD_ALT) as u32,
                    'D' as u32,
                );
                
                if result != 0 {
                    println!("[DebugHotkey] ✓ Hotkey Ctrl+Alt+D registered successfully!");
                    
                    let mut msg: MSG = std::mem::zeroed();
                    loop {
                        let msg_result = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                        if msg_result > 0 {
                            if msg.message == WM_HOTKEY && msg.wParam == DEBUG_HOTKEY_ID as usize {
                                println!("[DebugHotkey] ✓ Debug hotkey pressed!");
                                let mut trig = triggered_clone.lock().unwrap();
                                *trig = true;
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                } else {
                    let error_code = GetLastError();
                    println!("[DebugHotkey] ✗ Failed to register hotkey! Error code: {}", error_code);
                }
            }
        });
        
        Self { triggered }
    }
    
    pub fn check_triggered(&mut self) -> bool {
        let mut trig = self.triggered.lock().unwrap();
        if *trig {
            *trig = false;
            true
        } else {
            false
        }
    }
}
