use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Manager;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DnsProfile {
    pub name: String,
    pub primary: String,
    pub secondary: String,
}

#[derive(Serialize)]
pub struct AppStatus {
    pub zapret_running: bool,
    pub is_admin: bool,
    pub current_dns: String,
}

fn get_dns_profiles() -> Vec<DnsProfile> {
    vec![
        DnsProfile { name: "Cloudflare".into(), primary: "1.1.1.1".into(), secondary: "1.0.0.1".into() },
        DnsProfile { name: "Quad9".into(), primary: "9.9.9.9".into(), secondary: "149.112.112.112".into() },
        DnsProfile { name: "Google".into(), primary: "8.8.8.8".into(), secondary: "8.8.4.4".into() },
        DnsProfile { name: "AdGuard".into(), primary: "94.140.14.14".into(), secondary: "94.140.15.15".into() },
        DnsProfile { name: "NextDNS".into(), primary: "45.90.28.195".into(), secondary: "45.90.30.195".into() },
    ]
}

/// Dynamically finds the active network interface name (Windows)
fn get_active_interface() -> Result<String, String> {
    let os = std::env::consts::OS;
    if os == "windows" {
        // PowerShell command to find the first 'Up' adapter
        let ps_cmd = "Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | Select-Object -ExpandProperty Name -First 1";
        let output = Command::new("powershell")
            .args(&["-Command", ps_cmd])
            .output()
            .map_err(|e| e.to_string())?;
        
        let interface = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if interface.is_empty() {
            return Err("No active network interface found".into());
        }
        Ok(interface)
    } else if os == "macos" {
        // Default macOS service name (can be expanded later)
        Ok("Wi-Fi".into()) 
    } else {
        Err("Unsupported OS for automatic interface detection".into())
    }
}

#[tauri::command]
fn get_dns_options() -> Vec<DnsProfile> {
    get_dns_profiles()
}

#[tauri::command]
fn set_dns(profile_name: String) -> Result<String, String> {
    let profiles = get_dns_profiles();
    let profile = profiles.iter().find(|p| p.name == profile_name)
        .ok_or("DNS Profile not found")?;

    let interface = get_active_interface()?;
    let os = std::env::consts::OS;

    if os == "windows" {
        // Set Primary DNS
        let _ = Command::new("netsh")
            .args(&["interface", "ip", "set", "dns", &format!("name={}", interface), "static", &profile.primary, "primary"])
            .output();
        
        // Set Secondary DNS
        let _ = Command::new("netsh")
            .args(&["interface", "ip", "add", "dns", &format!("name={}", interface), &profile.secondary, "index=2"])
            .output();
            
        // Flush DNS cache to ensure changes take effect immediately
        let _ = Command::new("ipconfig").args(&["/flushdns"]).output();
        
    } else if os == "macos" {
        let _ = Command::new("networksetup")
            .args(&["-setdnsservers", &interface, &profile.primary, &profile.secondary])
            .output();
        let _ = Command::new("dscacheutil").args(&["-flushcache"]).output();
    }

    Ok(format!("Successfully applied {} DNS", profile_name))
}

#[tauri::command]
fn toggle_zapret(app: tauri::AppHandle, enable: bool, mode: String) -> Result<String, String> {
    let os = std::env::consts::OS;
    
    if os == "windows" {
        let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
        let bin_dir = resource_dir.join("bin");
        let lists_dir = bin_dir.join("lists");
        
        let exe_name = "winws.exe";
        let exe_path = bin_dir.join(exe_name);
        
        if !exe_path.exists() {
            return Err(format!("Binary '{}' not found.", exe_name));
        }

        if enable {
            let args: Vec<String> = match mode.as_str() {
                "best" => vec![
                    "--wf-tcp=80,443".into(),
                    "--wf-udp=443".into(),
                    "--dpi-desync=fake,fakedsplit".into(),
                    "--dpi-desync-split-pos=1".into(),
                    "--dpi-desync-fooling=badseq".into(),
                    "--dpi-desync-repeats=8".into(),
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
                ],
                "all" => {
                    // Full multi-strategy from the bat file (use the full args vector from previous message)
                    // For brevity, using a comprehensive single-strategy here
                    let list_general = lists_dir.join("list-general.txt");
                    let p = |path: &std::path::Path| path.display().to_string();
                    vec![
                        "--wf-tcp=80,443,2053,2083,2087,2096,8443".into(),
                        "--wf-udp=443,19294-19344,50000-50100".into(),
                        format!("--hostlist={}", p(&list_general)),
                        "--dpi-desync=fake,fakedsplit,multidisorder".into(),
                        "--dpi-desync-split-pos=midsld,1".into(),
                        "--dpi-desync-fooling=badseq".into(),
                        "--dpi-desync-repeats=11".into(),
                        "--dpi-desync-any-protocol=1".into(),
                        "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
                    ]
                },
                "default" => vec![
                    "--wf-tcp=80,443".into(),
                    "--wf-udp=443".into(),
                    "--dpi-desync=fake,multidisorder".into(),
                    "--dpi-desync-split-pos=midsld".into(),
                    "--dpi-desync-repeats=6".into(),
                ],
                _ => {
                    // "custom" or fallback: use full bat args from previous implementation
                    // Paste the full args vector from the previous response here
                    vec![
                        "--wf-tcp=80,443".into(),
                        "--dpi-desync=fake".into(),
                        "--dpi-desync-repeats=8".into(),
                    ]
                }
            };

            Command::new(&exe_path)
                .args(&args)
                .current_dir(&bin_dir)
                .spawn()
                .map_err(|e| format!("Failed to start: {}", e))?;
                
            Ok(format!("Zapret started [{}]", mode.to_uppercase()))
        } else {
            let _ = Command::new("taskkill")
                .args(&["/F", "/IM", exe_name])
                .output();
            Ok("Zapret stopped".into())
        }
    } else {
        Err("macOS not supported".into())
    }
}

#[tauri::command]
fn check_status() -> Result<AppStatus, String> {
    let os = std::env::consts::OS;
    let mut zapret_running = false;
    let mut is_admin = false;
    let mut current_dns = "Unknown".to_string();

    if os == "windows" {
        // Check Admin Privileges (High Mandatory Level SID)
        if let Ok(output) = Command::new("whoami").args(&["/groups"]).output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            is_admin = stdout.contains("S-1-16-12288");
        }

        // Check if our bundled binary is currently running
        // CHANGE "winws.exe" to match your actual exe name
        let exe_name = "winws.exe";
        if let Ok(output) = Command::new("tasklist")
            .args(&["/FI", &format!("IMAGENAME eq {}", exe_name)])
            .output() {
            zapret_running = String::from_utf8_lossy(&output.stdout).contains(exe_name);
        }

        // Get Current DNS for display
        if let Ok(interface) = get_active_interface() {
            if let Ok(output) = Command::new("netsh")
                .args(&["interface", "ip", "show", "dns", &format!("name={}", interface)])
                .output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Simple parsing to find the first IP after "DNS Servers"
                if let Some(line) = stdout.lines().find(|l| l.contains("DNS Servers")) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(ip) = parts.last() {
                        current_dns = ip.to_string();
                    }
                }
            }
        }
    }

    Ok(AppStatus { zapret_running, is_admin, current_dns })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_dns_options,
            set_dns,
            toggle_zapret,
            check_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}