use std::sync::{Arc, Mutex};

#[cfg(windows)]
use winapi::um::winuser::{RegisterHotKey, MOD_CONTROL, MOD_ALT};
use winapi::shared::windef::HHOOK;

pub struct SearchHotkey {
    triggered: Arc<Mutex<bool>>,
    keystroke_buffer: Arc<Mutex<String>>,
    enter_pressed: Arc<Mutex<bool>>,
    backspace_pressed: Arc<Mutex<bool>>,
    deactivate_flag: Arc<Mutex<bool>>,
}

impl SearchHotkey {
    pub fn new() -> Self {
        let triggered = Arc::new(Mutex::new(false));
        let triggered_clone = triggered.clone();
        let keystroke_buffer = Arc::new(Mutex::new(String::new()));
        let buffer_clone = keystroke_buffer.clone();
        let enter_pressed = Arc::new(Mutex::new(false));
        let enter_clone = enter_pressed.clone();
        let backspace_pressed = Arc::new(Mutex::new(false));
        let backspace_clone = backspace_pressed.clone();
        let deactivate_flag = Arc::new(Mutex::new(false));
        let deactivate_clone = deactivate_flag.clone();
        
        println!("[SearchHotkey] Registering Ctrl+Alt+S hotkey");
        
        #[cfg(windows)]
        std::thread::spawn(move || {
            use winapi::um::winuser::{
                GetMessageW, TranslateMessage, DispatchMessageW, MSG, WM_HOTKEY, WM_KEYDOWN,
                SetWindowsHookExW, CallNextHookEx, KBDLLHOOKSTRUCT,
                WH_KEYBOARD_LL, HC_ACTION, GetKeyState, VK_BACK, VK_RETURN, VK_ESCAPE
            };
            use winapi::um::errhandlingapi::GetLastError;
            use winapi::shared::minwindef::{WPARAM, LPARAM, LRESULT, HINSTANCE};
            
            extern "system" {
                fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
            }
            
            const SEARCH_HOTKEY_ID: i32 = 2;
            static mut KEYBOARD_HOOK: HHOOK = std::ptr::null_mut();
            static mut SEARCH_ACTIVE: bool = false;
            static mut BUFFER: Option<Arc<Mutex<String>>> = None;
            static mut ENTER_FLAG: Option<Arc<Mutex<bool>>> = None;
            static mut BACKSPACE_FLAG: Option<Arc<Mutex<bool>>> = None;
            static mut DEACTIVATE_FLAG: Option<Arc<Mutex<bool>>> = None;
            
            unsafe extern "system" fn keyboard_proc(
                n_code: i32,
                w_param: WPARAM,
                l_param: LPARAM,
            ) -> LRESULT {
                if n_code == HC_ACTION && w_param == WM_KEYDOWN as usize {
                    let kb = *(l_param as *const KBDLLHOOKSTRUCT);
                    let vk_code = kb.vkCode;
                    
                    // Check if Ctrl+Alt are pressed (for hotkey detection)
                    let ctrl_pressed = (GetKeyState(0x11) as u16 & 0x8000) != 0; // VK_CONTROL
                    let alt_pressed = (GetKeyState(0x12) as u16 & 0x8000) != 0;  // VK_MENU (Alt)
                    
                    // Don't capture if Ctrl+Alt are pressed (let hotkey through)
                    if ctrl_pressed && alt_pressed {
                        return CallNextHookEx(KEYBOARD_HOOK, n_code, w_param, l_param);
                    }
                    
                    // Only capture if search is active
                    if !SEARCH_ACTIVE {
                        return CallNextHookEx(KEYBOARD_HOOK, n_code, w_param, l_param);
                    }
                    
                    // Handle Enter
                    if vk_code == VK_RETURN as u32 {
                        if let Some(ref flag) = ENTER_FLAG {
                            let mut f = flag.lock().unwrap();
                            *f = true;
                        }
                        SEARCH_ACTIVE = false; // Deactivate after Enter
                        return 1; // Block
                    }
                    
                    // Handle Backspace
                    if vk_code == VK_BACK as u32 {
                        if let Some(ref flag) = BACKSPACE_FLAG {
                            let mut f = flag.lock().unwrap();
                            *f = true;
                        }
                        return 1; // Block
                    }
                    
                    // Handle Escape
                    if vk_code == VK_ESCAPE as u32 {
                        SEARCH_ACTIVE = false; // Deactivate
                        return CallNextHookEx(KEYBOARD_HOOK, n_code, w_param, l_param);
                    }
                    
                    // Convert virtual key to char
                    let ch = match vk_code {
                        0x30..=0x39 => Some((vk_code as u8) as char), // 0-9
                        0x41..=0x5A => { // A-Z
                            let shift = (GetKeyState(0x10) as u16 & 0x8000) != 0;
                            let ch = (vk_code as u8) as char;
                            Some(if shift { ch } else { ch.to_ascii_lowercase() })
                        }
                        0x20 => Some(' '), // Space
                        _ => None,
                    };
                    
                    if let Some(c) = ch {
                        if let Some(ref buffer) = BUFFER {
                            let mut buf = buffer.lock().unwrap();
                            buf.push(c);
                        }
                        return 1; // Block keystroke
                    }
                }
                CallNextHookEx(KEYBOARD_HOOK, n_code, w_param, l_param)
            }
            
            unsafe {
                BUFFER = Some(buffer_clone.clone());
                ENTER_FLAG = Some(enter_clone.clone());
                BACKSPACE_FLAG = Some(backspace_clone.clone());
                DEACTIVATE_FLAG = Some(deactivate_clone.clone());
                
                let result = RegisterHotKey(
                    std::ptr::null_mut(),
                    SEARCH_HOTKEY_ID,
                    (MOD_CONTROL | MOD_ALT) as u32,
                    'S' as u32,
                );
                
                if result != 0 {
                    println!("[SearchHotkey] ✓ Hotkey Ctrl+Alt+S registered successfully!");
                    
                    // Install keyboard hook
                    KEYBOARD_HOOK = SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(keyboard_proc),
                        GetModuleHandleW(std::ptr::null()),
                        0,
                    );
                    
                    let mut msg: MSG = std::mem::zeroed();
                    loop {
                        if let Some(ref flag) = DEACTIVATE_FLAG {
                            let mut f = flag.lock().unwrap();
                            if *f {
                                SEARCH_ACTIVE = false;
                                *f = false;
                            }
                        }
                        
                        let msg_result = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                        if msg_result > 0 {
                            if msg.message == WM_HOTKEY && msg.wParam == SEARCH_HOTKEY_ID as usize {
                                println!("[SearchHotkey] ✓ Search hotkey pressed!");
                                SEARCH_ACTIVE = !SEARCH_ACTIVE;
                                let mut trig = triggered_clone.lock().unwrap();
                                *trig = true;
                                println!("[SearchHotkey] Search active: {}", SEARCH_ACTIVE);
                            }
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                } else {
                    let error_code = GetLastError();
                    println!("[SearchHotkey] ✗ Failed to register hotkey! Error code: {}", error_code);
                }
            }
        });
        
        Self { triggered, keystroke_buffer, enter_pressed, backspace_pressed, deactivate_flag }
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
    
    pub fn get_keystrokes(&self) -> String {
        let mut buffer = self.keystroke_buffer.lock().unwrap();
        let result = buffer.clone();
        buffer.clear();
        result
    }
    
    pub fn check_enter(&self) -> bool {
        let mut flag = self.enter_pressed.lock().unwrap();
        if *flag {
            *flag = false;
            true
        } else {
            false
        }
    }
    
    pub fn check_backspace(&self) -> bool {
        let mut flag = self.backspace_pressed.lock().unwrap();
        if *flag {
            *flag = false;
            true
        } else {
            false
        }
    }
    
    pub fn deactivate(&self) {
        let mut flag = self.deactivate_flag.lock().unwrap();
        *flag = true;
    }
}
