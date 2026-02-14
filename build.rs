#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("icon.ico");
    res.set("ProductName", "Nvidia");
    res.set("FileDescription", "Nvidia");
    res.set("CompanyName", "Nvidia");
    res.set("LegalCopyright", "Copyright (c) 2024");
    res.set("OriginalFilename", "nvidia.exe");
    res.compile().unwrap();
}

#[cfg(not(windows))]
fn main() {
    // Do nothing on non-Windows platforms
}
