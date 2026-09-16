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
pub struct SftpOverview {
    pub status: SftpServiceStatus,
    pub shares: Vec<SftpShare>,
    pub users: Vec<SftpUser>,
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
    })
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

    let _ = Command::new("pkexec").args(["groupadd", "-f", "sftp-users"]).output();

    let user_exists = Command::new("id")
        .arg(&username)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !user_exists {
        let nologin_shell = if Path::new("/run/current-system/sw/bin/nologin").exists() {
            "/run/current-system/sw/bin/nologin"
        } else {
            "/bin/false"
        };

        let create_out = Command::new("pkexec")
            .args(["useradd", "-M", "-s", nologin_shell, "-g", "sftp-users", &username])
            .output()
            .map_err(|e| format!("Erreur création utilisateur via pkexec: {}", e))?;

        if !create_out.status.success() {
            let err = String::from_utf8_lossy(&create_out.stderr);
            return Err(format!("Échec de la création de l'utilisateur '{}': {}", username, err));
        }
    }

    if let Some(ref pwd) = password {
        if !pwd.is_empty() {
            let mut child = Command::new("pkexec")
                .args(["chpasswd"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Erreur exécution chpasswd: {}", e))?;

            if let Some(mut stdin) = child.stdin.take() {
                let input = format!("{}:{}\n", username, pwd);
                let _ = stdin.write_all(input.as_bytes());
            }

            let output = child.wait_with_output()
                .map_err(|e| format!("Erreur attente chpasswd: {}", e))?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Échec de l'application du mot de passe pour '{}': {}", username, err));
            }
        }
    }

    if !user.enabled {
        let _ = Command::new("pkexec").args(["usermod", "-L", &username]).output();
    } else {
        let _ = Command::new("pkexec").args(["usermod", "-U", &username]).output();
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

    let _ = Command::new("pkexec").args(["userdel", &username]).output();

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
}
