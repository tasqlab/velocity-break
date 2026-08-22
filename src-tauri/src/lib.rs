use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// Global source of truth for Zapret state
static ZAPRET_RUNNING: AtomicBool = AtomicBool::new(false);

/* ── Types ── */
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DnsProfile {
    pub name: String,
    pub primary: String,
    pub secondary: String,
    pub doh_url: String,
}

#[derive(Serialize)]
pub struct AppStatus {
    pub zapret_running: bool,
    pub is_admin: bool,
    pub current_dns: String,
    pub doh_active: bool,
}

/* ── DNS Profiles ── */
fn get_dns_profiles() -> Vec<DnsProfile> {
    vec![
        DnsProfile { name: "Cloudflare".into(), primary: "1.1.1.1".into(), secondary: "1.0.0.1".into(), doh_url: "https://cloudflare-dns.com/dns-query".into() },
        DnsProfile { name: "Quad9".into(), primary: "9.9.9.9".into(), secondary: "149.112.112.112".into(), doh_url: "https://dns.quad9.net/dns-query".into() },
        DnsProfile { name: "Google".into(), primary: "8.8.8.8".into(), secondary: "8.8.4.4".into(), doh_url: "https://dns.google/dns-query".into() },
        DnsProfile { name: "AdGuard".into(), primary: "94.140.14.14".into(), secondary: "94.140.15.15".into(), doh_url: "https://dns.adguard-dns.com/dns-query".into() },
        DnsProfile { name: "NextDNS".into(), primary: "45.90.28.0".into(), secondary: "45.90.30.0".into(), doh_url: "https://dns.nextdns.io".into() },
    ]
}

/* ── Helpers ── */
fn get_active_interface() -> Result<String, String> {
    #[cfg(windows)]
    {
        let ps = "Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | Select-Object -ExpandProperty Name -First 1";
        let out = Command::new("powershell")
            .args(&["-Command", ps])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| e.to_string())?;
        let iface = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if iface.is_empty() { return Err("No active network interface".into()); }
        Ok(iface)
    }
    #[cfg(not(windows))]
    { Ok("Wi-Fi".into()) }
}

/* ── Commands ── */
#[tauri::command]
fn get_dns_options() -> Vec<DnsProfile> { get_dns_profiles() }

#[tauri::command]
fn set_dns(profile_name: String, use_doh: bool) -> Result<String, String> {
    let profiles = get_dns_profiles();
    let profile = profiles.iter().find(|p| p.name == profile_name).ok_or("DNS Profile not found")?;
    let interface = get_active_interface()?;

    #[cfg(windows)]
    {
        let _ = Command::new("netsh").args(&["interface", "ip", "set", "dns", &format!("name={}", interface), "static", &profile.primary, "primary"]).creation_flags(CREATE_NO_WINDOW).output();
        let _ = Command::new("netsh").args(&["interface", "ip", "add", "dns", &format!("name={}", interface), &profile.secondary, "index=2"]).creation_flags(CREATE_NO_WINDOW).output();

        if use_doh {
            let ps = format!("Add-DnsClientDohServerAddress -ServerAddress '{}' -DohTemplate '{}' -AllowFallbackToUdp $false -AutoUpgrade $true; Add-DnsClientDohServerAddress -ServerAddress '{}' -DohTemplate '{}' -AllowFallbackToUdp $false -AutoUpgrade $true", profile.primary, profile.doh_url, profile.secondary, profile.doh_url);
            let _ = Command::new("powershell").args(&["-Command", &ps]).creation_flags(CREATE_NO_WINDOW).output();
        } else {
            let ps = format!("Remove-DnsClientDohServerAddress -ServerAddress '{}' -Confirm:$false -ErrorAction SilentlyContinue; Remove-DnsClientDohServerAddress -ServerAddress '{}' -Confirm:$false -ErrorAction SilentlyContinue", profile.primary, profile.secondary);
            let _ = Command::new("powershell").args(&["-Command", &ps]).creation_flags(CREATE_NO_WINDOW).output();
        }
        let _ = Command::new("ipconfig").args(&["/flushdns"]).creation_flags(CREATE_NO_WINDOW).output();
    }
    Ok(format!("Applied {} DNS{}", profile_name, if use_doh { " + DoH" } else { "" }))
}

#[tauri::command]
fn set_custom_dns(primary: String, secondary: String, doh_url: String) -> Result<String, String> {
    if primary.parse::<std::net::IpAddr>().is_err() { return Err(format!("Invalid DNS: {}", primary)); }
    let interface = get_active_interface()?;

    #[cfg(windows)]
    {
        let _ = Command::new("netsh").args(&["interface", "ip", "set", "dns", &format!("name={}", interface), "static", &primary, "primary"]).creation_flags(CREATE_NO_WINDOW).output();
        if !secondary.is_empty() {
            let _ = Command::new("netsh").args(&["interface", "ip", "add", "dns", &format!("name={}", interface), &secondary, "index=2"]).creation_flags(CREATE_NO_WINDOW).output();
        }
        if !doh_url.is_empty() {
            let ps = format!("Add-DnsClientDohServerAddress -ServerAddress '{}' -DohTemplate '{}' -AllowFallbackToUdp $false -AutoUpgrade $true", primary, doh_url);
            let _ = Command::new("powershell").args(&["-Command", &ps]).creation_flags(CREATE_NO_WINDOW).output();
        }
        let _ = Command::new("ipconfig").args(&["/flushdns"]).creation_flags(CREATE_NO_WINDOW).output();
    }
    Ok(format!("Custom DNS applied: {}", primary))
}

#[tauri::command]
fn reset_dns() -> Result<String, String> {
    let interface = get_active_interface()?;
    #[cfg(windows)]
    {
        let _ = Command::new("powershell").args(&["-Command", "Get-DnsClientDohServerAddress | Remove-DnsClientDohServerAddress -Confirm:$false -ErrorAction SilentlyContinue"]).creation_flags(CREATE_NO_WINDOW).output();
        let _ = Command::new("netsh").args(&["interface", "ip", "set", "dns", &format!("name={}", interface), "dhcp"]).creation_flags(CREATE_NO_WINDOW).output();
        let _ = Command::new("ipconfig").args(&["/flushdns"]).creation_flags(CREATE_NO_WINDOW).output();
    }
    Ok("DNS reset to DHCP".into())
}

#[tauri::command]
fn toggle_zapret(app: tauri::AppHandle, enable: bool, mode: String) -> Result<String, String> {
    #[cfg(windows)]
    {
        let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
        let bin_dir = resource_dir.join("bin");
        let exe_name = "winws.exe";
        let exe_path = bin_dir.join(exe_name);

        if !exe_path.exists() { return Err(format!("Binary '{}' not found at {}", exe_name, exe_path.display())); }

        if enable {
            // Use RELATIVE paths. winws.exe will look for these inside current_dir (which is bin_dir)
            let args: Vec<String> = match mode.as_str() {
                "best" => vec![
                    "--wf-tcp=80,443".into(), 
                    "--wf-udp=443".into(), 
                    "--hostlist=lists/list-general.txt".into(), 
                    "--hostlist-exclude=lists/list-exclude.txt".into(), 
                    "--dpi-desync=fake,fakedsplit".into(), 
                    "--dpi-desync-split-pos=1".into(), 
                    "--dpi-desync-fooling=badseq".into(), 
                    "--dpi-desync-repeats=8".into(), 
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into()
                ],
                "all" => vec![
                    "--wf-tcp=80,443,2053,2083,2087,2096,8443".into(), 
                    "--wf-udp=443,19294-19344,50000-50100".into(), 
                    "--hostlist=lists/list-general.txt".into(), 
                    "--hostlist-exclude=lists/list-exclude.txt".into(), 
                    "--dpi-desync=fake,fakedsplit,multidisorder".into(), 
                    "--dpi-desync-split-pos=midsld,1".into(), 
                    "--dpi-desync-fooling=badseq".into(), 
                    "--dpi-desync-repeats=11".into(), 
                    "--dpi-desync-any-protocol=1".into(), 
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into()
                ],
                _ => vec![
                    "--wf-tcp=80,443".into(), 
                    "--wf-udp=443".into(), 
                    "--dpi-desync=fake,multidisorder".into(), 
                    "--dpi-desync-split-pos=midsld".into(), 
                    "--dpi-desync-repeats=6".into()
                ],
            };

            let mut cmd = Command::new(&exe_path);
            cmd.args(&args)
               .current_dir(&bin_dir) // Force working directory to target/debug/bin/
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());

            #[cfg(windows)] cmd.creation_flags(CREATE_NO_WINDOW);
            
            let mut child = cmd.spawn().map_err(|e| format!("Failed to start: {}", e))?;
            
            // Give winws time to load WinDivert and reject invalid arguments.
            std::thread::sleep(std::time::Duration::from_millis(1000));

            match child.try_wait() {
                Ok(Some(status)) => {
                    return Err(format!(
                        "Zapret exited immediately with status {} (check WinDivert files and administrator rights)",
                        status
                    ));
                }
                Ok(None) => {
                    // Still running, success!
                    ZAPRET_RUNNING.store(true, Ordering::SeqCst);
                    if let Some(tray) = app.tray_by_id("velocity_tray") {
                        let _ = tray.set_tooltip(Some("Velocity - DPI Bypass (ACTIVE)"));
                    }
                    Ok(format!("Zapret started [{}]", mode.to_uppercase()))
                }
                Err(e) => Err(format!("Error checking process: {}", e))
            }
        } else {
            let mut k = Command::new("taskkill");
            k.args(&["/F", "/IM", exe_name]);
            #[cfg(windows)] k.creation_flags(CREATE_NO_WINDOW);
            let _ = k.output();
            
            ZAPRET_RUNNING.store(false, Ordering::SeqCst);
            
            if let Some(tray) = app.tray_by_id("velocity_tray") {
                let _ = tray.set_tooltip(Some("Velocity - DPI Bypass (Inactive)"));
            }
            Ok("Zapret stopped".into())
        }
    }
    #[cfg(not(windows))]
    { let _ = (app, enable, mode); Err("macOS not supported".into()) }
}

#[tauri::command]
fn check_status() -> Result<AppStatus, String> {
    let mut zapret_running = ZAPRET_RUNNING.load(Ordering::SeqCst);
    let mut is_admin = false;
    let mut current_dns = "Unknown".to_string();
    let mut doh_active = false;

    #[cfg(windows)]
    {
        if let Ok(out) = Command::new("tasklist").args(&["/FI", "IMAGENAME eq winws.exe"]).creation_flags(CREATE_NO_WINDOW).output() {
            let output = String::from_utf8_lossy(&out.stdout);
            if output.contains("winws.exe") {
                zapret_running = true;
                ZAPRET_RUNNING.store(true, Ordering::SeqCst);
            } else {
                zapret_running = false;
                ZAPRET_RUNNING.store(false, Ordering::SeqCst);
            }
        }

        if let Ok(out) = Command::new("whoami").args(&["/groups"]).creation_flags(CREATE_NO_WINDOW).output() {
            is_admin = String::from_utf8_lossy(&out.stdout).contains("S-1-16-12288");
        }

        if let Ok(iface) = get_active_interface() {
            if let Ok(out) = Command::new("netsh").args(&["interface", "ip", "show", "dns", &format!("name={}", iface)]).creation_flags(CREATE_NO_WINDOW).output() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(line) = stdout.lines().find(|l| l.contains("DNS Servers")) {
                    if let Some(ip) = line.split_whitespace().last() { current_dns = ip.to_string(); }
                }
            }
        }

        if let Ok(out) = Command::new("powershell").args(&["-Command", "(Get-DnsClientDohServerAddress -ErrorAction SilentlyContinue).Count"]).creation_flags(CREATE_NO_WINDOW).output() {
            let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            doh_active = count_str != "0" && !count_str.is_empty();
        }
    }
    Ok(AppStatus { zapret_running, is_admin, current_dns, doh_active })
}

#[tauri::command]
fn minimize_window(window: tauri::Window) { let _ = window.minimize(); }

#[tauri::command]
fn hide_to_tray(window: tauri::Window) { let _ = window.hide(); }

#[tauri::command]
fn set_autostart(enable: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
        let (key, _) = hkcu.create_subkey(path).map_err(|e| format!("Registry error: {}", e))?;
        if enable {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            key.set_value("Velocity", &exe.to_string_lossy().to_string()).map_err(|e| e.to_string())?;
        } else {
            let _ = key.delete_value("Velocity");
        }
        Ok(())
    }
    #[cfg(not(windows))]
    { let _ = enable; Err("Auto-start only supported on Windows".into()) }
}

/* ── App entry ── */
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let show_item = MenuItemBuilder::new("Show Velocity").id("show").build(app)?;
            let quit_item = MenuItemBuilder::new("Quit").id("quit").build(app)?;
            let tray_menu = MenuBuilder::new(app).item(&show_item).separator().item(&quit_item).build()?;

            // FIX: Use with_id() instead of new().id()
            let _tray = TrayIconBuilder::with_id("velocity_tray")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .tooltip("Velocity - DPI Bypass (Inactive)")
                .on_menu_event(|app, event| {
                    match event.id.as_ref() {
                        "show" => { if let Some(w) = app.get_webview_window("main") { let _ = w.show(); let _ = w.set_focus(); } }
                        "quit" => { app.exit(0); }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") { let _ = w.show(); let _ = w.set_focus(); }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_dns_options, set_dns, set_custom_dns, reset_dns,
            toggle_zapret, check_status, minimize_window, hide_to_tray, set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}