use std::sync::{Arc, Mutex};

pub struct McqHotkey {
    triggered: Arc<Mutex<bool>>,
}

impl McqHotkey {
    pub fn new() -> Self {
        let triggered = Arc::new(Mutex::new(false));
        let triggered_clone = triggered.clone();
        
        println!("[McqHotkey] Registering Ctrl+Alt+Q hotkey for MCQ capture");
        
        #[cfg(windows)]
        std::thread::spawn(move || {
            use winapi::um::winuser::{RegisterHotKey, GetMessageW, TranslateMessage, DispatchMessageW, MSG, WM_HOTKEY, MOD_CONTROL, MOD_ALT};
            use winapi::um::errhandlingapi::GetLastError;
            
            const MCQ_HOTKEY_ID: i32 = 7;
            
            unsafe {
                let result = RegisterHotKey(
                    std::ptr::null_mut(),
                    MCQ_HOTKEY_ID,
                    (MOD_CONTROL | MOD_ALT) as u32,
                    'Q' as u32,
                );
                
                if result != 0 {
                    println!("[McqHotkey] ✓ Hotkey Ctrl+Alt+Q registered successfully!");
                    
                    let mut msg: MSG = std::mem::zeroed();
                    loop {
                        let msg_result = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                        if msg_result > 0 {
                            if msg.message == WM_HOTKEY && msg.wParam == MCQ_HOTKEY_ID as usize {
                                println!("[McqHotkey] ✓✓✓ MCQ hotkey pressed! Triggering MCQ capture...");
                                let mut flag = triggered_clone.lock().unwrap();
                                *flag = true;
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                } else {
                    let error_code = GetLastError();
                    println!("[McqHotkey] ✗ Failed to register MCQ hotkey! Error code: {}", error_code);
                    println!("[McqHotkey] Error 1409 means hotkey is already registered by another app");
                }
            }
        });
        
        Self { triggered }
    }
    
    pub fn check_triggered(&mut self) -> bool {
        let mut flag = self.triggered.lock().unwrap();
        if *flag {
            *flag = false;
            true
        } else {
            false
        }
    }
}
