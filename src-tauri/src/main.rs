#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn main() {
    #[cfg(windows)]
    {
        let is_admin = {
            let output = std::process::Command::new("whoami")
                .args(&["/groups"])
                .creation_flags(0x08000000)
                .output();
            
            match output {
                Ok(o) => String::from_utf8_lossy(&o.stdout).contains("S-1-16-12288"),
                Err(_) => false,
            }
        };

        if !is_admin {
            let exe = std::env::current_exe().unwrap();
            let _ = std::process::Command::new("powershell")
                .args(&["-Command", &format!("Start-Process -FilePath '{}' -Verb RunAs", exe.display())])
                .creation_flags(0x08000000)
                .spawn();
            
            std::process::exit(0);
        }
    }

    velocity_lib::run();
}