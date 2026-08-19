use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::Manager;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
        DnsProfile { name: "NextDNS".into(), primary: "45.90.28.0".into(), secondary: "45.90.30.0".into() },
    ]
}

fn get_active_interface() -> Result<String, String> {
    let os = std::env::consts::OS;
    if os == "windows" {
        let ps_cmd = "Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | Select-Object -ExpandProperty Name -First 1";
        let mut cmd = Command::new("powershell");
        cmd.args(&["-Command", ps_cmd]);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output().map_err(|e| e.to_string())?;

        let interface = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if interface.is_empty() {
            return Err("No active network interface found".into());
        }
        Ok(interface)
    } else if os == "macos" {
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
        let mut cmd1 = Command::new("netsh");
        cmd1.args(&["interface", "ip", "set", "dns", &format!("name={}", interface), "static", &profile.primary, "primary"]);
        #[cfg(windows)]
        cmd1.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd1.output();

        // Set Secondary DNS
        let mut cmd2 = Command::new("netsh");
        cmd2.args(&["interface", "ip", "add", "dns", &format!("name={}", interface), &profile.secondary, "index=2"]);
        #[cfg(windows)]
        cmd2.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd2.output();

        // Flush DNS cache
        let mut cmd3 = Command::new("ipconfig");
        cmd3.args(&["/flushdns"]);
        #[cfg(windows)]
        cmd3.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd3.output();

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
            return Err(format!("Binary '{}' not found in bundled resources.", exe_name));
        }

        if enable {
            let list_general = lists_dir.join("list-general.txt");
            let list_general_user = lists_dir.join("list-general-user.txt");
            let list_exclude = lists_dir.join("list-exclude.txt");
            let list_exclude_user = lists_dir.join("list-exclude-user.txt");
            let ipset_exclude = lists_dir.join("ipset-exclude.txt");
            let ipset_exclude_user = lists_dir.join("ipset-exclude-user.txt");
            let list_google = lists_dir.join("list-google.txt");
            let ipset_all = lists_dir.join("ipset-all.txt");

            let quic_google = bin_dir.join("quic_initial_www_google_com.bin");
            let quic_dbank = bin_dir.join("quic_initial_dbankcloud_ru.bin");
            let tls_fake = bin_dir.join("tls_clienthello_max_ru.bin");

            let p = |path: &std::path::Path| path.display().to_string();

            let game_tcp = "";
            let game_udp = "";

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
                "all" => vec![
                    format!("--wf-tcp=80,443,2053,2083,2087,2096,8443,{}", game_tcp),
                    format!("--wf-udp=443,19294-19344,50000-50100,{}", game_udp),
                    "--filter-udp=443".into(),
                    format!("--hostlist={}", p(&list_general)),
                    format!("--hostlist={}", p(&list_general_user)),
                    format!("--hostlist-exclude={}", p(&list_exclude)),
                    format!("--hostlist-exclude={}", p(&list_exclude_user)),
                    format!("--ipset-exclude={}", p(&ipset_exclude)),
                    format!("--ipset-exclude={}", p(&ipset_exclude_user)),
                    "--dpi-desync=fake".into(),
                    "--dpi-desync-repeats=11".into(),
                    format!("--dpi-desync-fake-quic={}", p(&quic_google)),
                    "--new".into(),
                    "--filter-udp=19294-19344,50000-50100".into(),
                    "--filter-l7=discord,stun".into(),
                    "--dpi-desync=fake".into(),
                    format!("--dpi-desync-fake-discord={}", p(&quic_dbank)),
                    format!("--dpi-desync-fake-stun={}", p(&quic_dbank)),
                    "--dpi-desync-repeats=6".into(),
                    "--new".into(),
                    "--filter-tcp=2053,2083,2087,2096,8443".into(),
                    "--hostlist-domains=discord.media".into(),
                    "--dpi-desync=fake,fakedsplit".into(),
                    "--dpi-desync-split-pos=1".into(),
                    "--dpi-desync-fooling=badseq".into(),
                    "--dpi-desync-badseq-increment=2".into(),
                    "--dpi-desync-repeats=8".into(),
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
                    "--new".into(),
                    "--filter-tcp=443".into(),
                    format!("--hostlist={}", p(&list_google)),
                    "--ip-id=zero".into(),
                    "--dpi-desync=fake,fakedsplit".into(),
                    "--dpi-desync-split-pos=1".into(),
                    "--dpi-desync-fooling=badseq".into(),
                    "--dpi-desync-badseq-increment=2".into(),
                    "--dpi-desync-repeats=8".into(),
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
                    "--new".into(),
                    "--filter-tcp=80,443".into(),
                    format!("--hostlist={}", p(&list_general)),
                    format!("--hostlist={}", p(&list_general_user)),
                    format!("--hostlist-exclude={}", p(&list_exclude)),
                    format!("--hostlist-exclude={}", p(&list_exclude_user)),
                    format!("--ipset-exclude={}", p(&ipset_exclude)),
                    format!("--ipset-exclude={}", p(&ipset_exclude_user)),
                    "--dpi-desync=fake,fakedsplit".into(),
                    "--dpi-desync-split-pos=1".into(),
                    "--dpi-desync-fooling=badseq".into(),
                    "--dpi-desync-badseq-increment=2".into(),
                    "--dpi-desync-repeats=8".into(),
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
                    format!("--dpi-desync-fake-http={}", p(&tls_fake)),
                    "--new".into(),
                    "--filter-udp=443".into(),
                    format!("--ipset={}", p(&ipset_all)),
                    format!("--hostlist-exclude={}", p(&list_exclude)),
                    format!("--hostlist-exclude={}", p(&list_exclude_user)),
                    format!("--ipset-exclude={}", p(&ipset_exclude)),
                    format!("--ipset-exclude={}", p(&ipset_exclude_user)),
                    "--dpi-desync=fake".into(),
                    "--dpi-desync-repeats=11".into(),
                    format!("--dpi-desync-fake-quic={}", p(&quic_google)),
                    "--new".into(),
                    "--filter-tcp=80,443,8443".into(),
                    format!("--ipset={}", p(&ipset_all)),
                    format!("--hostlist-exclude={}", p(&list_exclude)),
                    format!("--hostlist-exclude={}", p(&list_exclude_user)),
                    format!("--ipset-exclude={}", p(&ipset_exclude)),
                    format!("--ipset-exclude={}", p(&ipset_exclude_user)),
                    "--dpi-desync=fake,fakedsplit".into(),
                    "--dpi-desync-split-pos=1".into(),
                    "--dpi-desync-fooling=badseq".into(),
                    "--dpi-desync-badseq-increment=2".into(),
                    "--dpi-desync-repeats=8".into(),
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
                    format!("--dpi-desync-fake-http={}", p(&tls_fake)),
                    "--new".into(),
                    format!("--filter-tcp={}", game_tcp),
                    format!("--ipset={}", p(&ipset_all)),
                    format!("--ipset-exclude={}", p(&ipset_exclude)),
                    format!("--ipset-exclude={}", p(&ipset_exclude_user)),
                    "--dpi-desync=fake,fakedsplit".into(),
                    "--dpi-desync-any-protocol=1".into(),
                    "--dpi-desync-cutoff=n3".into(),
                    "--dpi-desync-split-pos=1".into(),
                    "--dpi-desync-fooling=badseq".into(),
                    "--dpi-desync-badseq-increment=2".into(),
                    "--dpi-desync-repeats=8".into(),
                    "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
                    format!("--dpi-desync-fake-http={}", p(&tls_fake)),
                    "--new".into(),
                    format!("--filter-udp={}", game_udp),
                    format!("--ipset={}", p(&ipset_all)),
                    format!("--ipset-exclude={}", p(&list_exclude)),
                    format!("--ipset-exclude={}", p(&list_exclude_user)),
                    "--dpi-desync=fake".into(),
                    "--dpi-desync-repeats=10".into(),
                    "--dpi-desync-any-protocol=1".into(),
                    format!("--dpi-desync-fake-unknown-udp={}", p(&quic_dbank)),
                    "--dpi-desync-cutoff=n2".into(),
                ],
                "default" => vec![
                    "--wf-tcp=80,443".into(),
                    "--wf-udp=443".into(),
                    "--dpi-desync=fake,multidisorder".into(),
                    "--dpi-desync-split-pos=midsld".into(),
                    "--dpi-desync-repeats=6".into(),
                ],
                _ => vec![
                    "--wf-tcp=80,443".into(),
                    "--dpi-desync=fake".into(),
                    "--dpi-desync-repeats=8".into(),
                ],
            };

            let mut cmd = Command::new(&exe_path);
            cmd.args(&args).current_dir(&bin_dir);
            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.spawn().map_err(|e| format!("Failed to start winws: {}", e))?;

            Ok(format!("Zapret started [{}]", mode.to_uppercase()))
        } else {
            let mut kill_cmd = Command::new("taskkill");
            kill_cmd.args(&["/F", "/IM", exe_name]);
            #[cfg(windows)]
            kill_cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = kill_cmd.output();
            Ok("Zapret stopped".into())
        }
    } else {
        Err("macOS support requires manual pf/tpws configuration".into())
    }
}

#[tauri::command]
fn check_status() -> Result<AppStatus, String> {
    let os = std::env::consts::OS;
    let mut zapret_running = false;
    let mut is_admin = false;
    let mut current_dns = "Unknown".to_string();

    if os == "windows" {
        // Check Admin Privileges
        let mut whoami_cmd = Command::new("whoami");
        whoami_cmd.args(&["/groups"]);
        #[cfg(windows)]
        whoami_cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = whoami_cmd.output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            is_admin = stdout.contains("S-1-16-12288");
        }

        // Check if winws is running
        let exe_name = "winws.exe";
        let mut tasklist_cmd = Command::new("tasklist");
        tasklist_cmd.args(&["/FI", &format!("IMAGENAME eq {}", exe_name)]);
        #[cfg(windows)]
        tasklist_cmd.creation_flags(CREATE_NO_WINDOW);
        if let Ok(output) = tasklist_cmd.output() {
            zapret_running = String::from_utf8_lossy(&output.stdout).contains(exe_name);
        }

        // Get Current DNS
        if let Ok(interface) = get_active_interface() {
            let mut dns_cmd = Command::new("netsh");
            dns_cmd.args(&["interface", "ip", "show", "dns", &format!("name={}", interface)]);
            #[cfg(windows)]
            dns_cmd.creation_flags(CREATE_NO_WINDOW);
            if let Ok(output) = dns_cmd.output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
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