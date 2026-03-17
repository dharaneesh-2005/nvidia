//! Native PiP Window - Standalone Binary
//!
//! This executable creates a Picture-in-Picture window.
//! For full screen capture exclusion, this would need native WebView2 with SetWindowDisplayAffinity.
//! Currently uses Chrome/Edge app mode for reliability.

use std::process::Command;

fn main() {
    println!("[PiP Native] Starting PiP window...");

    // Get server URL from command line args or use default
    let args: Vec<String> = std::env::args().collect();
    let server_url = args.get(1).cloned().unwrap_or_else(|| "http://localhost:3000".to_string());
    let pip_url = format!("{}/pip", server_url);

    println!("[PiP Native] Opening: {}", pip_url);
    println!("[PiP Native] Looking for Chrome or Edge...");

    // Try Chrome first
    let chrome_paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];

    let edge_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];

    for path in &chrome_paths {
        if std::path::Path::new(path).exists() {
            println!("[PiP Native] Launching Chrome in app mode...");
            let _ = Command::new(path)
                .args(&[
                    "--app",
                    &pip_url,
                    "--window-size=450,350",
                    "--window-position=20,20",
                ])
                .spawn();
            println!("[PiP Native] ✓ Chrome PiP window launched");
            println!("[PiP Native] Note: Chrome windows are visible to screen capture");
            println!("[PiP Native] For capture exclusion, native WebView2 implementation needed");
            return;
        }
    }

    // Try Edge
    for path in &edge_paths {
        if std::path::Path::new(path).exists() {
            println!("[PiP Native] Launching Edge in app mode...");
            let _ = Command::new(path)
                .args(&[
                    "--app",
                    &pip_url,
                    "--window-size=450,350",
                    "--window-position=20,20",
                ])
                .spawn();
            println!("[PiP Native] ✓ Edge PiP window launched");
            return;
        }
    }

    println!("[PiP Native] Error: Chrome or Edge not found!");
    println!("[PiP Native] Please install Chrome or Edge to use PiP mode");
}
