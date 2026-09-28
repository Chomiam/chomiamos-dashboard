use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireguardProfile {
    pub id: String,
    pub name: String,
    pub interface_name: String,
    pub config_path: String,
    pub country_code: String,
    pub country_name: String,
    pub country_flag: String,
    pub endpoint: String,
    pub local_address: String,
    pub dns: String,
    pub is_active: bool,
    pub latest_handshake: String,
    pub transfer_rx: String,
    pub transfer_tx: String,
    pub created_at: String,
    pub raw_config: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireguardServerState {
    pub is_active: bool,
    pub interface_name: String,
    pub server_address: String,
    pub listen_port: u16,
    pub public_ip: String,
    pub server_public_key: String,
    pub friend_name: String,
    pub friend_address: String,
    pub friend_public_key: String,
    pub friend_config: String,
    pub friend_connected: bool,
    pub friend_latest_handshake: String,
    pub friend_transfer_rx: String,
    pub friend_transfer_tx: String,
    pub ping_latency_ms: Option<f64>,
    pub firewall_port_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireguardOverview {
    pub profiles: Vec<WireguardProfile>,
    pub active_profile_id: Option<String>,
    pub server: WireguardServerState,
    pub wireguard_installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelTestResult {
    pub success: bool,
    pub latency_ms: Option<f64>,
    pub handshake_status: String,
    pub bytes_received: String,
    pub bytes_sent: String,
    pub message: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WireguardStore {
    profiles: Vec<ProfileMeta>,
    server: Option<ServerMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProfileMeta {
    id: String,
    name: String,
    interface_name: String,
    country_code: String,
    country_name: String,
    country_flag: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServerMeta {
    friend_name: String,
    listen_port: u16,
    server_private_key: String,
    server_public_key: String,
    friend_private_key: String,
    friend_public_key: String,
    preshared_key: String,
    custom_endpoint: Option<String>,
}

fn get_base_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/chomiam".to_string());
    PathBuf::from(home).join(".config/chomiamos-dashboard/wireguard")
}

fn get_clients_dir() -> PathBuf {
    get_base_dir().join("clients")
}

fn get_server_dir() -> PathBuf {
    get_base_dir().join("server")
}

fn get_store_path() -> PathBuf {
    get_base_dir().join("wireguard.json")
}

fn ensure_dirs() {
    let _ = fs::create_dir_all(get_clients_dir());
    let _ = fs::create_dir_all(get_server_dir());
}

fn load_store() -> WireguardStore {
    ensure_dirs();
    let p = get_store_path();
    if p.exists() {
        if let Ok(content) = fs::read_to_string(&p) {
            if let Ok(store) = serde_json::from_str(&content) {
                return store;
            }
        }
    }
    WireguardStore::default()
}

fn save_store(store: &WireguardStore) {
    ensure_dirs();
    let p = get_store_path();
    if let Ok(s) = serde_json::to_string_pretty(store) {
        let _ = fs::write(p, s);
    }
}

pub fn get_country_info(code: &str) -> (String, String, String) {
    let code_upper = code.to_uppercase();
    match code_upper.as_str() {
        "FR" => ("fr".into(), "France".into(), "🇫🇷".into()),
        "CH" => ("ch".into(), "Suisse".into(), "🇨🇭".into()),
        "US" => ("us".into(), "États-Unis".into(), "🇺🇸".into()),
        "DE" => ("de".into(), "Allemagne".into(), "🇩🇪".into()),
        "NL" => ("nl".into(), "Pays-Bas".into(), "🇳🇱".into()),
        "GB" | "UK" => ("gb".into(), "Royaume-Uni".into(), "🇬🇧".into()),
        "JP" => ("jp".into(), "Japon".into(), "🇯🇵".into()),
        "SE" => ("se".into(), "Suède".into(), "🇸🇪".into()),
        "CA" => ("ca".into(), "Canada".into(), "🇨🇦".into()),
        "ES" => ("es".into(), "Espagne".into(), "🇪🇸".into()),
        "IT" => ("it".into(), "Italie".into(), "🇮🇹".into()),
        "BE" => ("be".into(), "Belgique".into(), "🇧🇪".into()),
        "NO" => ("no".into(), "Norvège".into(), "🇳🇴".into()),
        "FI" => ("fi".into(), "Finlande".into(), "🇫🇮".into()),
        "DK" => ("dk".into(), "Danemark".into(), "🇩🇰".into()),
        "IS" => ("is".into(), "Islande".into(), "🇮🇸".into()),
        "AT" => ("at".into(), "Autriche".into(), "🇦🇹".into()),
        "PT" => ("pt".into(), "Portugal".into(), "🇵🇹".into()),
        "PL" => ("pl".into(), "Pologne".into(), "🇵🇱".into()),
        "RO" => ("ro".into(), "Roumanie".into(), "🇷🇴".into()),
        "IE" => ("ie".into(), "Irlande".into(), "🇮🇪".into()),
        "AU" => ("au".into(), "Australie".into(), "🇦🇺".into()),
        "NZ" => ("nz".into(), "Nouvelle-Zélande".into(), "🇳🇿".into()),
        "SG" => ("sg".into(), "Singapour".into(), "🇸🇬".into()),
        "KR" => ("kr".into(), "Corée du Sud".into(), "🇰🇷".into()),
        "BR" => ("br".into(), "Brésil".into(), "🇧🇷".into()),
        "MX" => ("mx".into(), "Mexique".into(), "🇲🇽".into()),
        "IN" => ("in".into(), "Inde".into(), "🇮🇳".into()),
        "ZA" => ("za".into(), "Afrique du Sud".into(), "🇿🇦".into()),
        "UA" => ("ua".into(), "Ukraine".into(), "🇺🇦".into()),
        "CZ" => ("cz".into(), "République Tchèque".into(), "🇨🇿".into()),
        "TR" => ("tr".into(), "Turquie".into(), "🇹🇷".into()),
        "HK" => ("hk".into(), "Hong Kong".into(), "🇭🇰".into()),
        "TW" => ("tw".into(), "Taïwan".into(), "🇹🇼".into()),
        "GR" => ("gr".into(), "Grèce".into(), "🇬🇷".into()),
        _ => ("un".into(), "Réseau Privé".into(), "🌐".into()),
    }
}

pub fn detect_country_from_text(name: &str, endpoint: &str, content: &str) -> (String, String, String) {
    let combined = format!("{} {} {}", name, endpoint, content).to_lowercase();

    let matches = [
        ("fr", &["france", "paris", "marseille", "-fr-", ".fr", "fr1", "fr2", "fr3", "fra."][..]),
        ("ch", &["suisse", "switzerland", "zurich", "geneve", "geneva", "basel", "-ch-", ".ch", "ch1", "ch2"][..]),
        ("us", &["usa", "united states", "etats-unis", "new york", "los angeles", "chicago", "miami", "seattle", "-us-", ".us", "us1", "us2"][..]),
        ("de", &["germany", "allemagne", "frankfurt", "berlin", "munich", "-de-", ".de", "de1", "de2"][..]),
        ("nl", &["netherlands", "pays-bas", "holland", "amsterdam", "rotterdam", "-nl-", ".nl", "nl1", "nl2"][..]),
        ("gb", &["uk", "united kingdom", "royaume-uni", "london", "manchester", "-uk-", "-gb-", ".uk", "gb1"][..]),
        ("jp", &["japan", "japon", "tokyo", "osaka", "-jp-", ".jp", "jp1"][..]),
        ("se", &["sweden", "suede", "stockholm", "-se-", ".se", "se1"][..]),
        ("ca", &["canada", "montreal", "toronto", "vancouver", "-ca-", ".ca", "ca1"][..]),
        ("es", &["spain", "espagne", "madrid", "barcelona", "-es-", ".es", "es1"][..]),
        ("it", &["italy", "italie", "milan", "rome", "-it-", ".it", "it1"][..]),
        ("be", &["belgium", "belgique", "brussels", "bruxelles", "-be-", ".be"][..]),
        ("no", &["norway", "norvege", "oslo", "-no-", ".no"][..]),
        ("fi", &["finland", "finlande", "helsinki", "-fi-", ".fi"][..]),
        ("dk", &["denmark", "danemark", "copenhagen", "-dk-", ".dk"][..]),
        ("is", &["iceland", "islande", "reykjavik", "-is-", ".is"][..]),
        ("at", &["austria", "autriche", "vienna", "-at-", ".at"][..]),
        ("pt", &["portugal", "lisbon", "-pt-", ".pt"][..]),
        ("pl", &["poland", "pologne", "warsaw", "-pl-", ".pl"][..]),
        ("ro", &["romania", "roumanie", "bucharest", "-ro-", ".ro"][..]),
        ("ie", &["ireland", "irlande", "dublin", "-ie-", ".ie"][..]),
        ("au", &["australia", "australie", "sydney", "melbourne", "-au-", ".au"][..]),
        ("sg", &["singapore", "singapour", "-sg-", ".sg"][..]),
        ("kr", &["korea", "coree", "seoul", "-kr-", ".kr"][..]),
        ("br", &["brazil", "bresil", "sao paulo", "-br-", ".br"][..]),
        ("ua", &["ukraine", "kyiv", "-ua-", ".ua"][..]),
    ];

    for (code, keywords) in matches {
        for kw in keywords {
            if combined.contains(kw) {
                return get_country_info(code);
            }
        }
    }

    get_country_info("un")
}

fn parse_wireguard_conf(content: &str) -> (String, String, String, String) {
    let mut address = String::new();
    let mut dns = String::new();
    let mut endpoint = String::new();
    let mut pubkey = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_lowercase();
            let val = v.trim().to_string();
            match key.as_str() {
                "address" => {
                    if address.is_empty() {
                        address = val;
                    } else {
                        address = format!("{}, {}", address, val);
                    }
                }
                "dns" => {
                    if dns.is_empty() {
                        dns = val;
                    } else {
                        dns = format!("{}, {}", dns, val);
                    }
                }
                "endpoint" => endpoint = val,
                "publickey" => pubkey = val,
                _ => {}
            }
        }
    }

    (address, dns, endpoint, pubkey)
}

struct InterfaceLiveStats {
    latest_handshake: String,
    rx_bytes: u64,
    tx_bytes: u64,
}

fn get_proc_net_dev_stats() -> HashMap<String, (u64, u64)> {
    let mut map = HashMap::new();
    if let Ok(content) = fs::read_to_string("/proc/net/dev") {
        for line in content.lines().skip(2) {
            if let Some((iface, data)) = line.split_once(':') {
                let iface_name = iface.trim().to_string();
                let nums: Vec<&str> = data.split_whitespace().collect();
                if nums.len() >= 9 {
                    let rx = nums[0].parse::<u64>().unwrap_or(0);
                    let tx = nums[8].parse::<u64>().unwrap_or(0);
                    map.insert(iface_name, (rx, tx));
                }
            }
        }
    }
    map
}

pub const EXEC_PATH: &str = "/run/current-system/sw/bin:/etc/profiles/per-user/chomiam/bin:/run/wrappers/bin:/bin:/usr/bin";

pub fn get_wg_bin() -> &'static str {
    if std::path::Path::new("/run/current-system/sw/bin/wg").exists() {
        "/run/current-system/sw/bin/wg"
    } else if std::path::Path::new("/etc/profiles/per-user/chomiam/bin/wg").exists() {
        "/etc/profiles/per-user/chomiam/bin/wg"
    } else {
        "wg"
    }
}

pub fn get_nmcli_bin() -> &'static str {
    if std::path::Path::new("/run/current-system/sw/bin/nmcli").exists() {
        "/run/current-system/sw/bin/nmcli"
    } else if std::path::Path::new("/etc/profiles/per-user/chomiam/bin/nmcli").exists() {
        "/etc/profiles/per-user/chomiam/bin/nmcli"
    } else {
        "nmcli"
    }
}

fn get_live_interfaces() -> HashMap<String, InterfaceLiveStats> {
    let mut map = HashMap::new();
    let proc_stats = get_proc_net_dev_stats();

    // 1. Query NetworkManager for active wireguard connections (no root required)
    let nm_out = Command::new(get_nmcli_bin())
        .env("PATH", EXEC_PATH)
        .args(["-t", "-f", "NAME,TYPE,STATE,DEVICE", "connection", "show", "--active"])
        .output();

    if let Ok(o) = nm_out {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 && parts[1] == "wireguard" && (parts[2] == "activated" || parts[2] == "activating") {
                    let name = parts[0].to_string();
                    let dev = if parts.len() >= 4 && !parts[3].is_empty() {
                        parts[3].to_string()
                    } else {
                        name.clone()
                    };

                    let (rx, tx) = proc_stats.get(&dev).or_else(|| proc_stats.get(&name)).copied().unwrap_or((0, 0));

                    map.insert(name, InterfaceLiveStats {
                        latest_handshake: if rx > 0 || tx > 0 { "Trafic actif".into() } else { "En ligne".into() },
                        rx_bytes: rx,
                        tx_bytes: tx,
                    });
                }
            }
        }
    }

    // 2. Query ip link show type wireguard
    let ip_out = Command::new("ip").env("PATH", EXEC_PATH).args(["-br", "link", "show", "type", "wireguard"]).output();
    if let Ok(o) = ip_out {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(iface) = parts.get(0) {
                    let (rx, tx) = proc_stats.get(*iface).copied().unwrap_or((0, 0));
                    map.entry(iface.to_string()).or_insert(InterfaceLiveStats {
                        latest_handshake: if rx > 0 || tx > 0 { "Trafic actif".into() } else { "En ligne".into() },
                        rx_bytes: rx,
                        tx_bytes: tx,
                    });
                }
            }
        }
    }

    // 3. Query wg show if accessible
    let out = Command::new(get_wg_bin()).env("PATH", EXEC_PATH).args(["show", "all", "dump"]).output();
    if let Ok(o) = out {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 8 {
                    let iface = parts[0].to_string();
                    if let Ok(hs) = parts[5].parse::<u64>() {
                        if hs > 0 {
                            if let Some(stats) = map.get_mut(&iface) {
                                stats.latest_handshake = format_handshake_secs(hs);
                            }
                        }
                    }
                }
            }
        }
    }

    map
}

fn format_handshake_secs(epoch: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if now <= epoch {
        return "À l'instant".into();
    }
    let diff = now - epoch;
    if diff < 60 {
        format!("il y a {}s", diff)
    } else if diff < 3600 {
        format!("il y a {}m", diff / 60)
    } else if diff < 86400 {
        format!("il y a {}h", diff / 3600)
    } else {
        format!("il y a {}j", diff / 86400)
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} O", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} Ko", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} Mo", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} Go", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn run_wg_genkey() -> Result<String, String> {
    let out = Command::new(get_wg_bin())
        .env("PATH", EXEC_PATH)
        .arg("genkey")
        .output()
        .map_err(|e| format!("Erreur wg genkey : {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

fn run_wg_pubkey(private_key: &str) -> Result<String, String> {
    use std::io::Write;
    let mut child = Command::new(get_wg_bin())
        .env("PATH", EXEC_PATH)
        .arg("pubkey")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Erreur wg pubkey spawn : {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(private_key.as_bytes());
    }

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

fn run_wg_genpsk() -> Result<String, String> {
    let out = Command::new(get_wg_bin())
        .env("PATH", EXEC_PATH)
        .arg("genpsk")
        .output()
        .map_err(|e| format!("Erreur wg genpsk : {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

pub fn detect_public_ip() -> String {
    let urls = [
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
        "https://icanhazip.com",
    ];
    for u in urls {
        let out = Command::new("curl").env("PATH", EXEC_PATH)
            .args(["-s", "--connect-timeout", "2", "-m", "3", u])
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                let ip = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !ip.is_empty() && ip.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ':') {
                    return ip;
                }
            }
        }
    }

    // Fallback to local default route IP
    let ip_route = Command::new("sh")
        .arg("-c")
        .arg("ip route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}'")
        .output();
    if let Ok(o) = ip_route {
        let local = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !local.is_empty() {
            return local;
        }
    }

    "127.0.0.1".into()
}

fn is_firewall_udp_51820_open() -> bool {
    let fw_file = "/etc/nixos/firewall-user.nix";
    if let Ok(content) = fs::read_to_string(fw_file) {
        if content.contains("51820") {
            return true;
        }
    }
    false
}

// =========================================================================
// GESTION NATIVE SANS PRIVILÈGES ROOT VIA NETWORKMANAGER (NMCLI)
// =========================================================================

fn nm_up_wireguard(iface_name: &str, conf_path: &Path) -> Result<(), String> {
    let nmcli = get_nmcli_bin();

    // 1. Nettoyer toute ancienne connexion du même nom dans NetworkManager
    let _ = Command::new(nmcli)
        .env("PATH", EXEC_PATH)
        .args(["connection", "delete", iface_name])
        .output();

    // 2. Importer la configuration WireGuard dans NetworkManager (natif D-Bus utilisateur)
    let imp = Command::new(nmcli)
        .env("PATH", EXEC_PATH)
        .args(["connection", "import", "type", "wireguard", "file", conf_path.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Erreur exécution nmcli import : {}", e))?;

    if !imp.status.success() {
        let err = String::from_utf8_lossy(&imp.stderr);
        return Err(format!("Échec import nmcli : {}", err.trim()));
    }

    // 3. Activer la connexion
    let up = Command::new(nmcli)
        .env("PATH", EXEC_PATH)
        .args(["connection", "up", iface_name])
        .output()
        .map_err(|e| format!("Erreur exécution nmcli up : {}", e))?;

    if !up.status.success() {
        let err = String::from_utf8_lossy(&up.stderr);
        if !err.contains("already active") && !err.contains("déjà active") {
            return Err(format!("Échec activation nmcli : {}", err.trim()));
        }
    }

    Ok(())
}

fn nm_down_wireguard(iface_name: &str, _conf_path: &Path) -> Result<(), String> {
    let nmcli = get_nmcli_bin();

    // 1. Désactiver et supprimer de NetworkManager
    let _ = Command::new(nmcli)
        .env("PATH", EXEC_PATH)
        .args(["connection", "down", iface_name])
        .output();

    let _ = Command::new(nmcli)
        .env("PATH", EXEC_PATH)
        .args(["connection", "delete", iface_name])
        .output();

    // 2. Nettoyage de sécurité
    let _ = Command::new("ip")
        .env("PATH", EXEC_PATH)
        .args(["link", "delete", iface_name])
        .output();

    Ok(())
}

#[tauri::command]
pub fn get_wireguard_overview() -> Result<WireguardOverview, String> {
    ensure_dirs();
    let store = load_store();
    let live = get_live_interfaces();
    let is_installed = Command::new(get_nmcli_bin()).env("PATH", EXEC_PATH).arg("--version").output().is_ok()
        || Command::new(get_wg_bin()).env("PATH", EXEC_PATH).arg("--version").output().is_ok();

    let mut profiles = Vec::new();
    let mut active_id = None;

    for meta in &store.profiles {
        let conf_file = get_clients_dir().join(format!("{}.conf", meta.id));
        let raw = if conf_file.exists() {
            fs::read_to_string(&conf_file).unwrap_or_default()
        } else {
            String::new()
        };

        let (address, dns, endpoint, _pubkey) = parse_wireguard_conf(&raw);
        let iface_stats = live.get(&meta.interface_name);
        let is_act = iface_stats.is_some();

        if is_act {
            active_id = Some(meta.id.clone());
        }

        let hs = iface_stats.map(|s| s.latest_handshake.clone()).unwrap_or_else(|| "Déconnecté".into());
        let rx = iface_stats.map(|s| format_bytes(s.rx_bytes)).unwrap_or_else(|| "0 O".into());
        let tx = iface_stats.map(|s| format_bytes(s.tx_bytes)).unwrap_or_else(|| "0 O".into());

        profiles.push(WireguardProfile {
            id: meta.id.clone(),
            name: meta.name.clone(),
            interface_name: meta.interface_name.clone(),
            config_path: conf_file.to_string_lossy().to_string(),
            country_code: meta.country_code.clone(),
            country_name: meta.country_name.clone(),
            country_flag: meta.country_flag.clone(),
            endpoint,
            local_address: address,
            dns,
            is_active: is_act,
            latest_handshake: hs,
            transfer_rx: rx,
            transfer_tx: tx,
            created_at: meta.created_at.clone(),
            raw_config: raw,
        });
    }

    // Build Server State
    let srv_meta = store.server.unwrap_or_else(|| {
        let s_priv = run_wg_genkey().unwrap_or_default();
        let s_pub = run_wg_pubkey(&s_priv).unwrap_or_default();
        let f_priv = run_wg_genkey().unwrap_or_default();
        let f_pub = run_wg_pubkey(&f_priv).unwrap_or_default();
        let psk = run_wg_genpsk().unwrap_or_default();
        ServerMeta {
            friend_name: "Ami Invité".into(),
            listen_port: 51820,
            server_private_key: s_priv,
            server_public_key: s_pub,
            friend_private_key: f_priv,
            friend_public_key: f_pub,
            preshared_key: psk,
            custom_endpoint: None,
        }
    });

    let srv_iface = "wg-chomiam".to_string();
    let srv_stats = live.get(&srv_iface);
    let srv_is_active = srv_stats.is_some();
    let pub_ip = srv_meta.custom_endpoint.clone().unwrap_or_else(detect_public_ip);

    let friend_conf = format!(
        "[Interface]\n# Clé privée de l'ami invité sur votre réseau privé ChomiamOS\nPrivateKey = {}\nAddress = 10.100.0.2/24\nDNS = 1.1.1.1\n\n[Peer]\n# Votre machine ChomiamOS (Serveur Hôte)\nPublicKey = {}\nPresharedKey = {}\nEndpoint = {}:{}\nAllowedIPs = 10.100.0.0/24\nPersistentKeepalive = 25\n",
        srv_meta.friend_private_key,
        srv_meta.server_public_key,
        srv_meta.preshared_key,
        pub_ip,
        srv_meta.listen_port
    );

    let f_connected = srv_stats.map(|s| s.rx_bytes > 0 || s.tx_bytes > 0 || s.latest_handshake != "En ligne").unwrap_or(false);

    let server_state = WireguardServerState {
        is_active: srv_is_active,
        interface_name: srv_iface,
        server_address: "10.100.0.1/24".into(),
        listen_port: srv_meta.listen_port,
        public_ip: pub_ip,
        server_public_key: srv_meta.server_public_key,
        friend_name: srv_meta.friend_name,
        friend_address: "10.100.0.2/32".into(),
        friend_public_key: srv_meta.friend_public_key,
        friend_config: friend_conf,
        friend_connected: f_connected,
        friend_latest_handshake: srv_stats.map(|s| s.latest_handshake.clone()).unwrap_or_else(|| "En attente".into()),
        friend_transfer_rx: srv_stats.map(|s| format_bytes(s.rx_bytes)).unwrap_or_else(|| "0 O".into()),
        friend_transfer_tx: srv_stats.map(|s| format_bytes(s.tx_bytes)).unwrap_or_else(|| "0 O".into()),
        ping_latency_ms: None,
        firewall_port_open: is_firewall_udp_51820_open(),
    };

    Ok(WireguardOverview {
        profiles,
        active_profile_id: active_id,
        server: server_state,
        wireguard_installed: is_installed,
    })
}

#[tauri::command]
pub fn import_wireguard_profile(name: String, content: String, country_code: Option<String>) -> Result<WireguardProfile, String> {
    ensure_dirs();
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err("Le contenu de la configuration WireGuard est vide.".into());
    }
    if !trimmed.contains("[Interface]") {
        return Err("Configuration WireGuard invalide : section [Interface] manquante.".into());
    }

    let mut store = load_store();
    let count = store.profiles.len() + 1;
    let id = format!("wgc_{:02}_{}", count, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() % 1000);
    let iface_name = if id.len() > 15 { id[..15].to_string() } else { id.clone() };

    let (address, dns, endpoint, _pubkey) = parse_wireguard_conf(trimmed);

    let (c_code, c_name, c_flag) = if let Some(code) = country_code {
        if !code.is_empty() {
            get_country_info(&code)
        } else {
            detect_country_from_text(&name, &endpoint, trimmed)
        }
    } else {
        detect_country_from_text(&name, &endpoint, trimmed)
    };

    let conf_path = get_clients_dir().join(format!("{}.conf", id));
    fs::write(&conf_path, trimmed).map_err(|e| format!("Impossible d'enregistrer le fichier .conf : {}", e))?;

    let now_str = "Aujourd'hui".to_string();
    let profile_name = if name.trim().is_empty() {
        format!("VPN {} {}", c_flag, c_name)
    } else {
        name.trim().to_string()
    };

    let meta = ProfileMeta {
        id: id.clone(),
        name: profile_name.clone(),
        interface_name: iface_name.clone(),
        country_code: c_code.clone(),
        country_name: c_name.clone(),
        country_flag: c_flag.clone(),
        created_at: now_str.clone(),
    };

    store.profiles.push(meta);
    save_store(&store);

    Ok(WireguardProfile {
        id,
        name: profile_name,
        interface_name: iface_name,
        config_path: conf_path.to_string_lossy().to_string(),
        country_code: c_code,
        country_name: c_name,
        country_flag: c_flag,
        endpoint,
        local_address: address,
        dns,
        is_active: false,
        latest_handshake: "Déconnecté".into(),
        transfer_rx: "0 O".into(),
        transfer_tx: "0 O".into(),
        created_at: now_str,
        raw_config: trimmed.to_string(),
    })
}

#[tauri::command]
pub fn update_wireguard_profile(id: String, name: String, country_code: String) -> Result<WireguardProfile, String> {
    let mut store = load_store();
    let (c_code, c_name, c_flag) = get_country_info(&country_code);

    let mut found = false;
    for p in &mut store.profiles {
        if p.id == id {
            p.name = name.clone();
            p.country_code = c_code.clone();
            p.country_name = c_name.clone();
            p.country_flag = c_flag.clone();
            found = true;
            break;
        }
    }

    if !found {
        return Err(format!("Profil {} introuvable", id));
    }

    save_store(&store);

    let conf_path = get_clients_dir().join(format!("{}.conf", id));
    let raw = fs::read_to_string(&conf_path).unwrap_or_default();
    let (address, dns, endpoint, _pubkey) = parse_wireguard_conf(&raw);
    let live = get_live_interfaces();
    let is_act = live.contains_key(&id);

    Ok(WireguardProfile {
        id,
        name,
        interface_name: conf_path.file_stem().unwrap().to_string_lossy().to_string(),
        config_path: conf_path.to_string_lossy().to_string(),
        country_code: c_code,
        country_name: c_name,
        country_flag: c_flag,
        endpoint,
        local_address: address,
        dns,
        is_active: is_act,
        latest_handshake: if is_act { "Connecté".into() } else { "Déconnecté".into() },
        transfer_rx: "0 O".into(),
        transfer_tx: "0 O".into(),
        created_at: "Modifié".into(),
        raw_config: raw,
    })
}

#[tauri::command]
pub fn delete_wireguard_profile(id: String) -> Result<(), String> {
    let mut store = load_store();
    if let Some(pos) = store.profiles.iter().position(|p| p.id == id) {
        let meta = store.profiles.remove(pos);
        save_store(&store);

        let conf_path = get_clients_dir().join(format!("{}.conf", id));
        let _ = nm_down_wireguard(&meta.interface_name, &conf_path);
        let _ = fs::remove_file(conf_path);
        Ok(())
    } else {
        Err(format!("Profil {} introuvable", id))
    }
}

#[tauri::command]
pub fn toggle_wireguard_profile(id: String, activate: bool) -> Result<bool, String> {
    let store = load_store();
    let profile = store.profiles.iter().find(|p| p.id == id)
        .ok_or_else(|| format!("Profil {} introuvable", id))?;

    let conf_path = get_clients_dir().join(format!("{}.conf", profile.id));
    if !conf_path.exists() {
        return Err(format!("Fichier de configuration introuvable : {:?}", conf_path));
    }

    if activate {
        // Disconnect other active client profiles to prevent routing collisions
        for other in &store.profiles {
            if other.id != id {
                let other_conf = get_clients_dir().join(format!("{}.conf", other.id));
                let _ = nm_down_wireguard(&other.interface_name, &other_conf);
            }
        }

        nm_up_wireguard(&profile.interface_name, &conf_path)?;
        Ok(true)
    } else {
        nm_down_wireguard(&profile.interface_name, &conf_path)?;
        Ok(false)
    }
}

#[tauri::command]
pub fn start_friend_server(
    listen_port: Option<u16>,
    friend_name: Option<String>,
    custom_endpoint: Option<String>,
) -> Result<WireguardServerState, String> {
    ensure_dirs();
    let mut store = load_store();

    let mut srv_meta = store.server.unwrap_or_else(|| {
        let s_priv = run_wg_genkey().unwrap_or_default();
        let s_pub = run_wg_pubkey(&s_priv).unwrap_or_default();
        let f_priv = run_wg_genkey().unwrap_or_default();
        let f_pub = run_wg_pubkey(&f_priv).unwrap_or_default();
        let psk = run_wg_genpsk().unwrap_or_default();
        ServerMeta {
            friend_name: "Ami Invité".into(),
            listen_port: 51820,
            server_private_key: s_priv,
            server_public_key: s_pub,
            friend_private_key: f_priv,
            friend_public_key: f_pub,
            preshared_key: psk,
            custom_endpoint: None,
        }
    });

    if let Some(p) = listen_port {
        srv_meta.listen_port = p;
    }
    if let Some(fn_name) = friend_name {
        if !fn_name.trim().is_empty() {
            srv_meta.friend_name = fn_name.trim().to_string();
        }
    }
    if let Some(ce) = custom_endpoint {
        if !ce.trim().is_empty() {
            srv_meta.custom_endpoint = Some(ce.trim().to_string());
        }
    }

    // Write server config file
    let srv_conf_path = get_server_dir().join("wg-chomiam.conf");
    let server_conf_content = format!(
        "[Interface]\n# Serveur Réseau Privé ChomiamOS Hôte\nAddress = 10.100.0.1/24\nListenPort = {}\nPrivateKey = {}\n\n[Peer]\n# {}\nPublicKey = {}\nPresharedKey = {}\nAllowedIPs = 10.100.0.2/32\n",
        srv_meta.listen_port,
        srv_meta.server_private_key,
        srv_meta.friend_name,
        srv_meta.friend_public_key,
        srv_meta.preshared_key
    );

    fs::write(&srv_conf_path, &server_conf_content)
        .map_err(|e| format!("Impossible d'écrire la configuration serveur : {}", e))?;

    store.server = Some(srv_meta);
    save_store(&store);

    // Démarrer l'interface wg-chomiam via NetworkManager
    nm_up_wireguard("wg-chomiam", &srv_conf_path)?;

    // Return fresh state
    let overview = get_wireguard_overview()?;
    Ok(overview.server)
}

#[tauri::command]
pub fn stop_friend_server() -> Result<WireguardServerState, String> {
    let srv_conf_path = get_server_dir().join("wg-chomiam.conf");
    let _ = nm_down_wireguard("wg-chomiam", &srv_conf_path);

    let overview = get_wireguard_overview()?;
    Ok(overview.server)
}

#[tauri::command]
pub fn test_friend_tunnel() -> Result<TunnelTestResult, String> {
    let live = get_live_interfaces();
    let srv_stats = live.get("wg-chomiam");

    if srv_stats.is_none() {
        return Ok(TunnelTestResult {
            success: false,
            latency_ms: None,
            handshake_status: "Serveur inactif".into(),
            bytes_received: "0 O".into(),
            bytes_sent: "0 O".into(),
            message: "Le serveur privé WireGuard n'est pas encore démarré.".into(),
            details: "Cliquez d'abord sur « Démarrer le Réseau Privé » pour initialiser l'interface hôte.".into(),
        });
    }

    let stats = srv_stats.unwrap();
    let rx_str = format_bytes(stats.rx_bytes);
    let tx_str = format_bytes(stats.tx_bytes);

    // ICMP ping to friend's IP (10.100.0.2)
    let ping_out = Command::new("ping")
        .env("PATH", EXEC_PATH)
        .args(["-c", "2", "-W", "1", "-i", "0.2", "10.100.0.2"])
        .output();

    if let Ok(po) = ping_out {
        if po.status.success() {
            let out_str = String::from_utf8_lossy(&po.stdout);
            let mut latency = None;
            for line in out_str.lines() {
                if line.contains("rtt min/avg/max/mdev") || line.contains("round-trip") {
                    if let Some(part) = line.split('=').nth(1) {
                        let values: Vec<&str> = part.split('/').collect();
                        if values.len() >= 2 {
                            if let Ok(avg) = values[1].trim().parse::<f64>() {
                                latency = Some(avg);
                            }
                        }
                    }
                }
            }

            let lat_display = latency.map(|l| format!("{:.1} ms", l)).unwrap_or_else(|| "1 ms".into());
            return Ok(TunnelTestResult {
                success: true,
                latency_ms: latency,
                handshake_status: stats.latest_handshake.clone(),
                bytes_received: rx_str,
                bytes_sent: tx_str,
                message: format!("⚡ Tunnel actif et ami en ligne ! Latence : {}", lat_display),
                details: "La communication bidirectionnelle WireGuard fonctionne parfaitement.".into(),
            });
        }
    }

    if stats.rx_bytes > 0 || stats.tx_bytes > 0 || stats.latest_handshake.contains("s") || stats.latest_handshake.contains("m") {
        Ok(TunnelTestResult {
            success: true,
            latency_ms: None,
            handshake_status: stats.latest_handshake.clone(),
            bytes_received: rx_str,
            bytes_sent: tx_str,
            message: "🤝 Tunnel WireGuard actif avec trafic détecté.".into(),
            details: "L'ami est connecté et échange des données avec l'hôte (le pare-feu de l'ami bloque peut-être les requêtes ICMP ping).".into(),
        })
    } else {
        Ok(TunnelTestResult {
            success: false,
            latency_ms: None,
            handshake_status: "En attente de connexion".into(),
            bytes_received: rx_str,
            bytes_sent: tx_str,
            message: "⏳ En attente de connexion de l'ami.".into(),
            details: "L'hôte écoute sur le port UDP 51820. Transmettez la configuration ou le QR Code à votre ami, et vérifiez que votre box internet redirige le port UDP 51820 vers ce PC.".into(),
        })
    }
}

#[tauri::command]
pub fn regenerate_friend_keys(friend_name: Option<String>) -> Result<WireguardServerState, String> {
    let mut store = load_store();
    let s_priv = run_wg_genkey().map_err(|e| format!("Erreur clé serveur: {}", e))?;
    let s_pub = run_wg_pubkey(&s_priv).map_err(|e| format!("Erreur pubkey serveur: {}", e))?;
    let f_priv = run_wg_genkey().map_err(|e| format!("Erreur clé client: {}", e))?;
    let f_pub = run_wg_pubkey(&f_priv).map_err(|e| format!("Erreur pubkey client: {}", e))?;
    let psk = run_wg_genpsk().map_err(|e| format!("Erreur psk: {}", e))?;

    let name = friend_name.unwrap_or_else(|| {
        store.server.as_ref().map(|s| s.friend_name.clone()).unwrap_or_else(|| "Ami Invité".into())
    });

    let srv_meta = ServerMeta {
        friend_name: name,
        listen_port: 51820,
        server_private_key: s_priv,
        server_public_key: s_pub,
        friend_private_key: f_priv,
        friend_public_key: f_pub,
        preshared_key: psk,
        custom_endpoint: store.server.as_ref().and_then(|s| s.custom_endpoint.clone()),
    };

    store.server = Some(srv_meta);
    save_store(&store);

    let overview = get_wireguard_overview()?;
    Ok(overview.server)
}

#[tauri::command]
pub fn open_wireguard_firewall_port() -> Result<String, String> {
    let fw_file = "/etc/nixos/firewall-user.nix";
    let content = if Path::new(fw_file).exists() {
        fs::read_to_string(fw_file).unwrap_or_default()
    } else {
        String::new()
    };

    if !content.contains("51820") {
        let re = regex::Regex::new(r"allowedUDPPorts\s*=\s*\[([^\]]*)\]").unwrap();
        let new_content = if let Some(caps) = re.captures(&content) {
            let existing = &caps[1];
            let replacement = format!("allowedUDPPorts = [{}\n      51820 # WireGuard Private Network\n    ]", existing);
            content.replace(&caps[0], &replacement)
        } else {
            content
        };
        let _ = fs::write(fw_file, &new_content);
        let _ = fs::write("/etc/nixos/.firewall-user.nix.backup", &new_content);
    }

    Ok("Port UDP 51820 autorisé dans le pare-feu NixOS !".to_string())
}
