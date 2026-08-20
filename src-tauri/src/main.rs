#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        // Belt AND suspenders — manifest handles DPI at OS level,
        // this ensures WebView2 doesn't double-scale on top
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", 
            "--force-device-scale-factor=1"
        );
    }
    
    velocity_lib::run();
}