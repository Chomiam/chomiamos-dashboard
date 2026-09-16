#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub github_has_updates: bool,
    pub github_remote_commit: Option<String>,
    pub github_local_commit: String,
    pub dashboard_has_updates: bool,
    pub dashboard_remote_commit: Option<String>,
    pub dashboard_locked_commit: Option<String>,
    pub dashboard_channel: String,
    pub dashboard_remote_version: Option<String>,
    pub system_needs_switch: bool,
    pub current_version: String,
    pub message: String,
}

pub fn check_system_updates() -> UpdateCheckResult {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    // 1. Commit local /etc/nixos
    let local_commit = Command::new("git")
        .args(["-C", "/etc/nixos", "rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "inconnu".to_string());

    let full_local_commit = Command::new("git")
        .args(["-C", "/etc/nixos", "rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    // 2. Vérification remote /etc/nixos (Chomiam/nix_config_gaming)
    let remote_nixos = Command::new("git")
        .args(["-C", "/etc/nixos", "ls-remote", "origin", "HEAD"])
        .output();

    let mut github_has_updates = false;
    let mut github_remote_commit = None;

    if let Ok(out) = remote_nixos {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(token) = text.split_whitespace().next() {
                github_remote_commit = Some(token[..7.min(token.len())].to_string());
                if !full_local_commit.is_empty() && token != full_local_commit {
                    let is_ancestor = Command::new("git")
                        .args(["-C", "/etc/nixos", "merge-base", "--is-ancestor", token, "HEAD"])
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);

                    if !is_ancestor {
                        github_has_updates = true;
                    }
                }
            }
        }
    }

    // 3. Détection du canal actuel du dashboard et du commit installé
    let mut dashboard_channel = "Stable".to_string();
    let mut installed_commit: Option<String> = None;

    // A. Priorité au profil utilisateur (~/.nix-profile/manifest.json)
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/chomiam".into());
    let manifest_path = format!("{}/.nix-profile/manifest.json", home);
    if let Ok(content) = fs::read_to_string(&manifest_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(dash) = json.get("elements").and_then(|e| e.get("chomiamos-dashboard")) {
                if let Some(orig) = dash.get("originalUrl").and_then(|u| u.as_str()) {
                    if orig.contains("/testing") {
                        dashboard_channel = "Testing".to_string();
                    }
                }
                if let Some(url) = dash.get("url").and_then(|u| u.as_str()) {
                    if let Some(rev_part) = url.split('/').nth(2) {
                        let clean_rev = rev_part.split('?').next().unwrap_or(rev_part);
                        if !clean_rev.is_empty() {
                            installed_commit = Some(clean_rev.to_string());
                        }
                    }
                }
            }
        }
    }

    // B. Si pas dans le profil utilisateur, lire /etc/nixos/flake.nix et flake.lock
    if installed_commit.is_none() {
        if let Ok(flake_nix) = fs::read_to_string("/etc/nixos/flake.nix") {
            for line in flake_nix.lines() {
                if line.contains("chomiamos-dashboard") && line.contains("/testing") {
                    dashboard_channel = "Testing".to_string();
                    break;
                }
            }
        }

        if let Ok(content) = fs::read_to_string("/etc/nixos/flake.lock") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(rev) = json
                    .get("nodes")
                    .and_then(|n| n.get("chomiamos-dashboard"))
                    .and_then(|d| d.get("locked"))
                    .and_then(|l| l.get("rev"))
                    .and_then(|r| r.as_str())
                {
                    installed_commit = Some(rev.to_string());
                }
            }
        }
    }

    // 4. Vérification de la disponibilité d'une mise à jour sur GitHub
    let mut dashboard_has_updates = false;
    let mut dashboard_locked_commit = None;
    let mut dashboard_remote_commit = None;
    let dashboard_remote_version = None;

    if let Some(rev) = installed_commit {
        dashboard_locked_commit = Some(rev[..7.min(rev.len())].to_string());

        let target_ref = if dashboard_channel == "Testing" {
            "refs/heads/testing"
        } else {
            "refs/heads/main"
        };

        let remote_dash = Command::new("git")
            .args([
                "ls-remote",
                "https://github.com/Chomiam/chomiamos-dashboard.git",
                target_ref,
            ])
            .output();

        if let Ok(out) = remote_dash {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                if let Some(token) = text.split_whitespace().next() {
                    dashboard_remote_commit = Some(token[..7.min(token.len())].to_string());
                    if token != rev {
                        dashboard_has_updates = true;
                    }
                }
            }
        }
    }

    // 5. Vérification si la configuration locale a des modifications non déployées
    let mut system_needs_switch = false;
    let sys_profile = std::path::Path::new("/nix/var/nix/profiles/system");
    if let Ok(sys_meta) = std::fs::symlink_metadata(sys_profile) {
        if let Ok(sys_time) = sys_meta.modified() {
            let head_time_output = Command::new("git")
                .args(["-C", "/etc/nixos", "log", "-1", "--format=%ct"])
                .output();

            if let Ok(out) = head_time_output {
                if let Ok(ts_str) = String::from_utf8(out.stdout) {
                    if let Ok(head_ts) = ts_str.trim().parse::<u64>() {
                        let head_duration = std::time::Duration::from_secs(head_ts);
                        if let Ok(sys_duration) = sys_time.duration_since(std::time::UNIX_EPOCH) {
                            if head_duration > sys_duration {
                                system_needs_switch = true;
                            }
                        }
                    }
                }
            }

            if let Ok(lock_meta) = std::fs::metadata("/etc/nixos/flake.lock") {
                if let Ok(lock_time) = lock_meta.modified() {
                    if lock_time > sys_time {
                        system_needs_switch = true;
                    }
                }
            }
        }
    }

    let message = if github_has_updates {
        "Modifications disponibles sur le dépôt GitHub de ChomiamOS".to_string()
    } else if dashboard_has_updates {
        format!("Nouvelle version du Dashboard disponible sur le canal {}", dashboard_channel)
    } else if system_needs_switch {
        "Nouvelle version prête à être déployée (nh os switch)".to_string()
    } else {
        "Votre système et votre tableau de bord sont à jour".to_string()
    };

    UpdateCheckResult {
        github_has_updates,
        github_remote_commit,
        github_local_commit: local_commit,
        dashboard_has_updates,
        dashboard_remote_commit,
        dashboard_locked_commit,
        dashboard_channel,
        dashboard_remote_version,
        system_needs_switch,
        current_version,
        message,
    }
}
