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
                    // Vérifier si le commit distant est déjà ancêtre du HEAD local
                    // (ex: si l'utilisateur a des commits locaux d'avance)
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

    // 3. Vérification de la version du dashboard dans /etc/nixos/flake.lock vs GitHub
    let mut dashboard_has_updates = false;
    let mut dashboard_locked_commit = None;
    let mut dashboard_remote_commit = None;

    if let Ok(content) = fs::read_to_string("/etc/nixos/flake.lock") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(rev) = json
                .get("nodes")
                .and_then(|n| n.get("chomiamos-dashboard"))
                .and_then(|d| d.get("locked"))
                .and_then(|l| l.get("rev"))
                .and_then(|r| r.as_str())
            {
                dashboard_locked_commit = Some(rev[..7.min(rev.len())].to_string());

                let remote_dash = Command::new("git")
                    .args([
                        "ls-remote",
                        "https://github.com/Chomiam/chomiamos-dashboard.git",
                        "HEAD",
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
        }
    }

    let message = if github_has_updates {
        "Modifications disponibles sur le dépôt GitHub de ChomiamOS".to_string()
    } else if dashboard_has_updates {
        "Nouvelle version du Dashboard ChomiamOS disponible sur GitHub".to_string()
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
        current_version,
        message,
    }
}
