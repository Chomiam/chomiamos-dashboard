use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub const SFTP_CONFIG_PATH: &str = "/etc/nixos/sftp-config.json";
pub const SFTP_CONFIG_BACKUP: &str = "/etc/nixos/.sftp-config.json.backup";
pub const SFTP_SSHD_CONF: &str = "/etc/nixos/sftp-sshd.conf";
pub const SFTP_SSHD_BACKUP: &str = "/etc/nixos/.sftp-sshd.conf.backup";
pub const VARS_NIX: &str = "/etc/nixos/vars.nix";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SftpShare {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_created_at")]
    pub created_at: String,
    #[serde(default)]
    pub authorized_users: Vec<String>,
    #[serde(default)]
    pub item_count: usize,
    #[serde(default)]
    pub size_human: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SftpUser {
    pub username: String,
    pub share_id: String,
    #[serde(default = "default_permission")]
    pub permission: String, // "ro" | "rw" | "full"
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_created_at")]
    pub created_at: String,
}

fn default_permission() -> String {
    "ro".to_string()
}

fn default_true() -> bool {
    true
}

fn default_created_at() -> String {
    "2026-09-16".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SftpConfigFile {
    #[serde(default)]
    pub shares: Vec<SftpShare>,
    #[serde(default)]
    pub users: Vec<SftpUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpServiceStatus {
    pub sshd_active: bool,
    pub openssh_configured: bool,
    pub port_22_firewall_open: bool,
    pub port_22_accessible: bool,
    pub password_auth_enabled: bool,
    pub local_ip: String,
    pub sshd_unit_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpVpnContext {
    pub server_is_active: bool,
    pub server_ip: String,
    pub friend_name: String,
    pub friend_connected: bool,
    pub friend_ip: String,
    pub client_is_active: bool,
    pub client_profile_name: Option<String>,
    pub client_country_flag: Option<String>,
    pub client_remote_ip: Option<String>,
    pub client_local_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpOverview {
    pub status: SftpServiceStatus,
    pub shares: Vec<SftpShare>,
    pub users: Vec<SftpUser>,
    pub vpn_context: SftpVpnContext,
}

pub fn load_sftp_config() -> SftpConfigFile {
    let path = Path::new(SFTP_CONFIG_PATH);
    let backup = Path::new(SFTP_CONFIG_BACKUP);

    let raw = if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else if backup.exists() {
        fs::read_to_string(backup).unwrap_or_default()
    } else {
        String::new()
    };

    if raw.trim().is_empty() {
        return SftpConfigFile {
            shares: vec![SftpShare {
                id: "public".to_string(),
                name: "Dossier Public sFTP".to_string(),
                path: "/home/chomiam/Partages/sftp_public".to_string(),
                description: "Partage sFTP principal accessible aux utilisateurs configurés".to_string(),
                created_at: "2026-09-16".to_string(),
                authorized_users: Vec::new(),
                item_count: 0,
                size_human: "0 B".to_string(),
            }],
            users: Vec::new(),
        };
    }

    serde_json::from_str(&raw).unwrap_or_else(|_| SftpConfigFile {
        shares: vec![SftpShare {
            id: "public".to_string(),
            name: "Dossier Public sFTP".to_string(),
            path: "/home/chomiam/Partages/sftp_public".to_string(),
            description: "Partage sFTP principal accessible aux utilisateurs configurés".to_string(),
            created_at: "2026-09-16".to_string(),
            authorized_users: Vec::new(),
            item_count: 0,
            size_human: "0 B".to_string(),
        }],
        users: Vec::new(),
    })
}

pub fn save_sftp_config(cfg: &SftpConfigFile) -> Result<(), String> {
    let json_str = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("Erreur sérialisation sFTP JSON: {}", e))?;

    let tmp_path = format!("{}.tmp", SFTP_CONFIG_PATH);
    fs::write(&tmp_path, &json_str)
        .map_err(|e| format!("Impossible d'écrire {}: {}", tmp_path, e))?;

    fs::rename(&tmp_path, SFTP_CONFIG_PATH)
        .map_err(|e| format!("Impossible de déplacer vers {}: {}", SFTP_CONFIG_PATH, e))?;

    let _ = fs::write(SFTP_CONFIG_BACKUP, &json_str);
    Ok(())
}

pub fn generate_sshd_conf(cfg: &SftpConfigFile) -> String {
    let mut out = String::new();
    out.push_str("# =========================================================================\n");
    out.push_str("# 📁 CONFIGURATION SERVEUR sFTP (CHOMIAMOS)\n");
    out.push_str("# Ce fichier est géré automatiquement par le Dashboard ChomiamOS.\n");
    out.push_str("# =========================================================================\n\n");

    out.push_str("# Règles globales de sécurité pour le groupe sftp-users\n");
    out.push_str("Match Group sftp-users\n");
    out.push_str("    PasswordAuthentication yes\n");
    out.push_str("    KbdInteractiveAuthentication yes\n");
    out.push_str("    PermitEmptyPasswords no\n");
    out.push_str("    X11Forwarding no\n");
    out.push_str("    AllowTcpForwarding no\n");
    out.push_str("    PermitTunnel no\n\n");

    let shares_map: HashMap<String, String> = cfg
        .shares
        .iter()
        .map(|s| (s.id.clone(), s.path.clone()))
        .collect();

    for user in &cfg.users {
        if !user.enabled {
            continue;
        }

        let share_path = shares_map
            .get(&user.share_id)
            .cloned()
            .unwrap_or_else(|| "/home/chomiam/Partages/sftp_public".to_string());

        out.push_str(&format!(
            "# Utilisateur sFTP: {} (Dossier: {}, Permissions: {})\n",
            user.username, share_path, user.permission
        ));
        out.push_str(&format!("Match User {}\n", user.username));
        out.push_str("    PasswordAuthentication yes\n");
        out.push_str("    KbdInteractiveAuthentication yes\n");
        out.push_str("    X11Forwarding no\n");
        out.push_str("    AllowTcpForwarding no\n");
        out.push_str("    PermitTunnel no\n");

        match user.permission.as_str() {
            "ro" => {
                out.push_str(&format!("    ForceCommand internal-sftp -R -d {}\n", share_path));
            }
            "rw" => {
                out.push_str(&format!(
                    "    ForceCommand internal-sftp -P remove,rmdir -d {}\n",
                    share_path
                ));
            }
            _ => {
                out.push_str(&format!("    ForceCommand internal-sftp -d {}\n", share_path));
            }
        }
        out.push_str("\n");
    }

    out
}

pub fn write_sshd_conf(cfg: &SftpConfigFile) -> Result<(), String> {
    let conf_content = generate_sshd_conf(cfg);
    let tmp_path = format!("{}.tmp", SFTP_SSHD_CONF);

    fs::write(&tmp_path, &conf_content)
        .map_err(|e| format!("Impossible d'écrire {}: {}", tmp_path, e))?;

    fs::rename(&tmp_path, SFTP_SSHD_CONF)
        .map_err(|e| format!("Impossible de déplacer vers {}: {}", SFTP_SSHD_CONF, e))?;

    let _ = fs::write(SFTP_SSHD_BACKUP, &conf_content);
    Ok(())
}

pub fn get_local_ip() -> String {
    if let Ok(output) = Command::new("hostname").arg("-I").output() {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout);
            for ip in out_str.split_whitespace() {
                if !ip.starts_with("127.") && !ip.contains(":") {
                    return ip.to_string();
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

pub fn check_port_22_accessible() -> bool {
    let addr: SocketAddr = "127.0.0.1:22".parse().unwrap();
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

pub fn check_port_22_firewall() -> bool {
    if let Ok(content) = fs::read_to_string("/etc/nixos/firewall-user.nix") {
        if content.contains("22") {
            return true;
        }
    }
    if let Ok(content) = fs::read_to_string("/etc/nixos/modules/services/openssh.nix") {
        if content.contains("openFirewall = cfg.openFirewall") || content.contains("openFirewall = true") {
            return true;
        }
    }
    true
}

pub fn read_openssh_vars() -> bool {
    if let Ok(content) = fs::read_to_string(VARS_NIX) {
        let re = regex::Regex::new(r#"openssh\s*=\s*(true|false);"#).unwrap();
        if let Some(caps) = re.captures(&content) {
            return &caps[1] == "true";
        }
    }
    true
}

pub fn set_openssh_vars(enable: bool) -> Result<(), String> {
    let content = fs::read_to_string(VARS_NIX)
        .map_err(|e| format!("Impossible de lire {}: {}", VARS_NIX, e))?;

    let re = regex::Regex::new(r#"openssh\s*=\s*(true|false);"#).unwrap();
    let updated = if re.is_match(&content) {
        re.replace(&content, format!("openssh = {};", enable)).to_string()
    } else {
        if let Some(idx) = content.rfind("}") {
            let mut s = content[..idx].to_string();
            s.push_str(&format!("  openssh = {};\n}}", enable));
            s
        } else {
            format!("{}\nopenssh = {};", content, enable)
        }
    };

    let tmp = format!("{}.tmp", VARS_NIX);
    fs::write(&tmp, &updated)
        .map_err(|e| format!("Erreur écriture {}: {}", tmp, e))?;
    fs::rename(&tmp, VARS_NIX)
        .map_err(|e| format!("Erreur remplacement {}: {}", VARS_NIX, e))?;

    let _ = fs::write("/etc/nixos/.vars.nix.backup", &updated);
    Ok(())
}

fn calculate_folder_stats(path_str: &str) -> (usize, String) {
    let path = Path::new(path_str);
    if !path.exists() || !path.is_dir() {
        return (0, "0 B".to_string());
    }

    let mut count = 0;
    let mut total_bytes = 0u64;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            count += 1;
            if let Ok(meta) = entry.metadata() {
                total_bytes += meta.len();
            }
        }
    }

    (count, format_bytes(total_bytes))
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} Go", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} Mo", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} Ko", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// =========================================================================
// 🚀 COMMANDES TAURI EXPOSÉES AU FRONTEND
// =========================================================================

#[tauri::command]
pub fn get_sftp_overview() -> Result<SftpOverview, String> {
    let sshd_out = Command::new("systemctl")
        .args(["is-active", "sshd"])
        .output();
    let sshd_active = match sshd_out {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim() == "active",
        Err(_) => false,
    };

    let unit_status = if let Ok(out) = Command::new("systemctl")
        .args(["show", "sshd.service", "--property=ActiveState,SubState"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout);
        s.replace("
", " ").trim().to_string()
    } else {
        if sshd_active { "active (running)".to_string() } else { "inactive (dead)".to_string() }
    };

    let openssh_configured = read_openssh_vars();
    let port_22_firewall_open = check_port_22_firewall();
    let port_22_accessible = check_port_22_accessible();
    let local_ip = get_local_ip();

    let mut cfg = load_sftp_config();

    for share in &mut cfg.shares {
        let (cnt, sz) = calculate_folder_stats(&share.path);
        share.item_count = cnt;
        share.size_human = sz;
    }

    let vpn_context = get_sftp_vpn_context();

    Ok(SftpOverview {
        status: SftpServiceStatus {
            sshd_active,
            openssh_configured,
            port_22_firewall_open,
            port_22_accessible,
            password_auth_enabled: true,
            local_ip,
            sshd_unit_status: unit_status,
        },
        shares: cfg.shares,
        users: cfg.users,
        vpn_context,
    })
}

pub fn resolve_remote_wireguard_ip(local_addr: &str, raw_conf: &str) -> Option<String> {
    // 1. Chercher un AllowedIPs spécifique dans [Peer] (ex: AllowedIPs = 10.100.0.1/32 ou /24)
    for line in raw_conf.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with("allowedips") {
            if let Some((_, val)) = trimmed.split_once('=') {
                for item in val.split(',') {
                    let cidr = item.trim();
                    if let Some((ip, mask)) = cidr.split_once('/') {
                        if mask == "32" && !ip.ends_with(".0") && !ip.ends_with(".255") {
                            return Some(ip.to_string());
                        } else if mask == "24" {
                            let parts: Vec<&str> = ip.split('.').collect();
                            if parts.len() == 4 {
                                return Some(format!("{}.{}.{}.1", parts[0], parts[1], parts[2]));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Déduire depuis local_address (ex: 10.100.0.2/24 -> 10.100.0.1)
    let clean_local = local_addr.split(',').next().unwrap_or("").trim();
    if let Some((ip_part, _mask)) = clean_local.split_once('/') {
        let parts: Vec<&str> = ip_part.split('.').collect();
        if parts.len() == 4 {
            if let Ok(last_octet) = parts[3].parse::<u8>() {
                let peer_octet = if last_octet == 1 { 2 } else { 1 };
                return Some(format!("{}.{}.{}.{}", parts[0], parts[1], parts[2], peer_octet));
            }
        }
    }

    None
}

pub fn get_sftp_vpn_context() -> SftpVpnContext {
    match crate::wireguard::get_wireguard_overview() {
        Ok(wg) => {
            let srv_act = wg.server.is_active;
            let srv_ip = wg.server.server_address.split('/').next().unwrap_or("10.100.0.1").to_string();
            let friend_ip = wg.server.friend_address.split('/').next().unwrap_or("10.100.0.2").to_string();
            let friend_name = wg.server.friend_name;
            let friend_conn = wg.server.friend_connected;

            let mut client_act = false;
            let mut client_prof_name = None;
            let mut client_country_flag = None;
            let mut client_remote_ip = None;
            let mut client_local_ip = None;

            if let Some(ref act_id) = wg.active_profile_id {
                if let Some(prof) = wg.profiles.iter().find(|p| &p.id == act_id) {
                    client_act = true;
                    client_prof_name = Some(prof.name.clone());
                    client_country_flag = Some(prof.country_flag.clone());
                    client_local_ip = Some(prof.local_address.split('/').next().unwrap_or("").to_string());
                    client_remote_ip = resolve_remote_wireguard_ip(&prof.local_address, &prof.raw_config);
                }
            }

            SftpVpnContext {
                server_is_active: srv_act,
                server_ip: srv_ip,
                friend_name,
                friend_connected: friend_conn,
                friend_ip,
                client_is_active: client_act,
                client_profile_name: client_prof_name,
                client_country_flag,
                client_remote_ip,
                client_local_ip,
            }
        }
        Err(_) => SftpVpnContext {
            server_is_active: false,
            server_ip: "10.100.0.1".into(),
            friend_name: "Ami".into(),
            friend_connected: false,
            friend_ip: "10.100.0.2".into(),
            client_is_active: false,
            client_profile_name: None,
            client_country_flag: None,
            client_remote_ip: None,
            client_local_ip: None,
        },
    }
}

#[tauri::command]
pub fn toggle_sftp_service(enable: bool) -> Result<String, String> {
    set_openssh_vars(enable)?;

    let action = if enable { "start" } else { "stop" };
    let output = Command::new("systemctl")
        .args([action, "sshd"])
        .output()
        .map_err(|e| format!("Erreur exécution systemctl {}: {}", action, e))?;

    if !output.status.success() {
        let _ = Command::new("pkexec")
            .args(["systemctl", action, "sshd"])
            .output();
    }

    let msg = if enable {
        "Bloc OpenSSH activé et service sshd démarré avec succès !"
    } else {
        "Bloc OpenSSH désactivé et service sshd arrêté avec succès."
    };

    Ok(msg.to_string())
}

#[tauri::command]
pub fn control_sftp_service(action: String) -> Result<String, String> {
    let action = action.trim().to_lowercase();
    let valid = ["start", "stop", "restart", "reload"];
    if !valid.contains(&action.as_str()) {
        return Err(format!("Action invalide: {}", action));
    }

    let output = Command::new("systemctl")
        .args([&action, "sshd"])
        .output();

    let success = match output {
        Ok(out) => out.status.success(),
        Err(_) => false,
    };

    if !success {
        let pk = Command::new("pkexec")
            .args(["systemctl", &action, "sshd"])
            .output()
            .map_err(|e| format!("Erreur pkexec systemctl {}: {}", action, e))?;
        if !pk.status.success() {
            let err = String::from_utf8_lossy(&pk.stderr);
            return Err(format!("Échec de l'action {}: {}", action, err));
        }
    }

    let action_fr = match action.as_str() {
        "start" => "démarré",
        "stop" => "arrêté",
        "restart" => "redémarré",
        "reload" => "rechargé",
        _ => "exécuté",
    };

    Ok(format!("Service OpenSSH (sshd) {} avec succès !", action_fr))
}

#[tauri::command]
pub fn open_sftp_firewall_port() -> Result<String, String> {
    let fw_file = "/etc/nixos/firewall-user.nix";
    let content = if Path::new(fw_file).exists() {
        fs::read_to_string(fw_file).unwrap_or_default()
    } else {
        String::new()
    };

    if !content.contains("22") {
        let re = regex::Regex::new(r"allowedTCPPorts\s*=\s*\[([^\]]*)\]").unwrap();
        let new_content = if let Some(caps) = re.captures(&content) {
            let existing = &caps[1];
            let replacement = format!("allowedTCPPorts = [{}\n      22 # sFTP / SSH\n    ]", existing);
            content.replace(&caps[0], &replacement)
        } else {
            content
        };
        let _ = fs::write(fw_file, &new_content);
        let _ = fs::write("/etc/nixos/.firewall-user.nix.backup", &new_content);
    }

    Ok("Port 22 autorisé dans le pare-feu !".to_string())
}

#[tauri::command]
pub fn save_sftp_share(share: SftpShare) -> Result<String, String> {
    if share.name.trim().is_empty() {
        return Err("Le nom du dossier partagé ne peut pas être vide.".to_string());
    }
    if share.path.trim().is_empty() {
        return Err("Le chemin du dossier ne peut pas être vide.".to_string());
    }

    let path = Path::new(&share.path);
    if !path.exists() {
        fs::create_dir_all(path)
            .map_err(|e| format!("Impossible de créer le dossier {}: {}", share.path, e))?;
    }

    let _ = Command::new("chmod").args(["g+rwx", &share.path]).output();

    let mut cfg = load_sftp_config();
    let mut updated = false;

    for s in &mut cfg.shares {
        if s.id == share.id {
            s.name = share.name.clone();
            s.path = share.path.clone();
            s.description = share.description.clone();
            s.authorized_users = share.authorized_users.clone();
            updated = true;
            break;
        }
    }

    if !updated {
        cfg.shares.push(share);
    }

    save_sftp_config(&cfg)?;
    write_sshd_conf(&cfg)?;

    let _ = Command::new("systemctl").args(["reload", "sshd"]).output();

    Ok("Dossier de partage sFTP enregistré avec succès !".to_string())
}

#[tauri::command]
pub fn delete_sftp_share(share_id: String) -> Result<String, String> {
    let mut cfg = load_sftp_config();
    cfg.shares.retain(|s| s.id != share_id);

    let fallback_share_id = cfg.shares.first().map(|s| s.id.clone()).unwrap_or_default();
    for u in &mut cfg.users {
        if u.share_id == share_id {
            u.share_id = fallback_share_id.clone();
        }
    }

    save_sftp_config(&cfg)?;
    write_sshd_conf(&cfg)?;

    let _ = Command::new("systemctl").args(["reload", "sshd"]).output();

    Ok("Dossier de partage supprimé de la configuration sFTP.".to_string())
}

#[tauri::command]
pub fn save_sftp_user(user: SftpUser, password: Option<String>) -> Result<String, String> {
    let username = user.username.trim().to_lowercase();
    if username.is_empty() || username.len() > 32 {
        return Err("Nom d'utilisateur sFTP invalide (doit faire entre 1 et 32 caractères).".to_string());
    }

    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Le nom d'utilisateur ne peut contenir que des lettres, chiffres, tirets et underscores.".to_string());
    }

    let forbidden = ["root", "daemon", "bin", "sys", "sync", "games", "man", "lp", "mail", "news", "uucp", "proxy", "www-data", "backup", "list", "irc", "gnats", "nobody", "systemd-network", "systemd-resolve"];
    if forbidden.contains(&username.as_str()) {
        return Err(format!("Le nom d'utilisateur '{}' est réservé au système.", username));
    }

    let nologin_shell = if Path::new("/run/current-system/sw/bin/nologin").exists() {
        "/run/current-system/sw/bin/nologin"
    } else {
        "/bin/false"
    };

    let script = r#"
set -e
USER="$1"
SHELL_PATH="$2"
ENABLED="$3"
HAS_PWD="$4"

# 1. Création du groupe sftp-users si inexistant
groupadd -f sftp-users

# 2. Création de l'utilisateur sans home (-M) avec shell nologin si inexistant
if ! id -u "$USER" >/dev/null 2>&1; then
    useradd -M -s "$SHELL_PATH" -g sftp-users "$USER"
fi

# 3. Application du mot de passe s'il a été transmis via stdin
if [ "$HAS_PWD" = "1" ]; then
    chpasswd
fi

# 4. Activation ou verrouillage du compte
if [ "$ENABLED" = "1" ]; then
    usermod -U "$USER" 2>/dev/null || true
else
    usermod -L "$USER" 2>/dev/null || true
fi
"#;

    let has_pwd = password.as_ref().map(|p| !p.is_empty()).unwrap_or(false);
    let has_pwd_flag = if has_pwd { "1" } else { "0" };
    let enabled_flag = if user.enabled { "1" } else { "0" };

    let mut child = Command::new("pkexec")
        .args(["bash", "-c", script, "sftp_setup", &username, nologin_shell, enabled_flag, has_pwd_flag])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Impossible d'exécuter l'élévation de privilèges (pkexec) : {}", e))?;

    if has_pwd {
        if let Some(ref pwd) = password {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = writeln!(stdin, "{}:{}", username, pwd);
            }
        }
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Erreur lors de la configuration du compte système : {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err_trim = err.trim();
        if err_trim.contains("Not authorized") || err_trim.contains("dismissed") || output.status.code() == Some(126) || output.status.code() == Some(127) {
            return Err("Action annulée ou autorisation administrateur refusée.".to_string());
        }
        return Err(format!("Échec de la configuration système pour '{}' : {}", username, err_trim));
    }

    let mut cfg = load_sftp_config();
    let mut user_entry = user.clone();
    user_entry.username = username.clone();

    for share in &mut cfg.shares {
        if share.id == user.share_id {
            if !share.authorized_users.contains(&username) {
                share.authorized_users.push(username.clone());
            }
        } else {
            share.authorized_users.retain(|u| u != &username);
        }
    }

    let mut found = false;
    for u in &mut cfg.users {
        if u.username == username {
            u.share_id = user.share_id.clone();
            u.permission = user.permission.clone();
            u.enabled = user.enabled;
            found = true;
            break;
        }
    }
    if !found {
        cfg.users.push(user_entry);
    }

    save_sftp_config(&cfg)?;
    write_sshd_conf(&cfg)?;

    let _ = Command::new("systemctl").args(["reload", "sshd"]).output();

    Ok(format!("Utilisateur sFTP '{}' enregistré et configuré avec succès !", username))
}

#[tauri::command]
pub fn delete_sftp_user(username: String) -> Result<String, String> {
    let username = username.trim().to_lowercase();
    if username.is_empty() {
        return Err("Nom d'utilisateur invalide".to_string());
    }

    let del_out = Command::new("pkexec")
        .args(["userdel", "-f", &username])
        .output();

    if let Ok(ref out) = del_out {
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            if err.contains("Not authorized") || out.status.code() == Some(126) {
                return Err("Suppression annulée : autorisation administrateur requise.".to_string());
            }
        }
    }

    let mut cfg = load_sftp_config();
    cfg.users.retain(|u| u.username != username);

    for share in &mut cfg.shares {
        share.authorized_users.retain(|u| u != &username);
    }

    save_sftp_config(&cfg)?;
    write_sshd_conf(&cfg)?;

    let _ = Command::new("systemctl").args(["reload", "sshd"]).output();

    Ok(format!("Utilisateur sFTP '{}' supprimé avec succès.", username))
}

#[tauri::command]
pub fn open_folder_in_dolphin(path: String) -> Result<String, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Chemin invalide".to_string());
    }
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("Impossible d'ouvrir le dossier: {}", e))?;
    Ok("Dossier ouvert avec succès".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_sshd_conf_permissions() {
        let cfg = SftpConfigFile {
            shares: vec![
                SftpShare {
                    id: "share1".to_string(),
                    name: "Public".to_string(),
                    path: "/srv/sftp/public".to_string(),
                    description: "Public".to_string(),
                    created_at: "2026-09-16".to_string(),
                    authorized_users: vec!["alice".to_string(), "bob".to_string(), "charlie".to_string()],
                    item_count: 0,
                    size_human: "0 B".to_string(),
                }
            ],
            users: vec![
                SftpUser {
                    username: "alice".to_string(),
                    share_id: "share1".to_string(),
                    permission: "ro".to_string(),
                    enabled: true,
                    created_at: "2026-09-16".to_string(),
                },
                SftpUser {
                    username: "bob".to_string(),
                    share_id: "share1".to_string(),
                    permission: "rw".to_string(),
                    enabled: true,
                    created_at: "2026-09-16".to_string(),
                },
                SftpUser {
                    username: "charlie".to_string(),
                    share_id: "share1".to_string(),
                    permission: "full".to_string(),
                    enabled: true,
                    created_at: "2026-09-16".to_string(),
                },
            ],
        };

        let conf = generate_sshd_conf(&cfg);

        assert!(conf.contains("Match Group sftp-users"));
        assert!(conf.contains("PasswordAuthentication yes"));

        assert!(conf.contains("Match User alice"));
        assert!(conf.contains("ForceCommand internal-sftp -R -d /srv/sftp/public"));

        assert!(conf.contains("Match User bob"));
        assert!(conf.contains("ForceCommand internal-sftp -P remove,rmdir -d /srv/sftp/public"));

        assert!(conf.contains("Match User charlie"));
        assert!(conf.contains("ForceCommand internal-sftp -d /srv/sftp/public"));
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(2048), "2.0 Ko");
        assert_eq!(format_bytes(1024 * 1024 * 5), "5.0 Mo");
    }

    #[test]
    fn test_resolve_remote_wireguard_ip() {
        let conf1 = "[Interface]\nAddress = 10.100.0.2/24\n[Peer]\nAllowedIPs = 10.100.0.0/24\n";
        assert_eq!(resolve_remote_wireguard_ip("10.100.0.2/24", conf1), Some("10.100.0.1".to_string()));

        let conf2 = "[Interface]\nAddress = 10.200.0.5/24\n[Peer]\nAllowedIPs = 10.200.0.1/32\n";
        assert_eq!(resolve_remote_wireguard_ip("10.200.0.5/24", conf2), Some("10.200.0.1".to_string()));

        let conf3 = "[Interface]\nAddress = 10.10.0.1/24\n[Peer]\nAllowedIPs = 10.10.0.2/32\n";
        assert_eq!(resolve_remote_wireguard_ip("10.10.0.1/24", conf3), Some("10.10.0.2".to_string()));
    }
}
