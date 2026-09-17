use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
    pub dashboard_stable_version: Option<String>,
    pub dashboard_stable_commit: Option<String>,
    pub dashboard_testing_version: Option<String>,
    pub dashboard_testing_commit: Option<String>,
    pub system_needs_switch: bool,
    pub current_version: String,
    pub message: String,
}

fn fetch_remote_cargo_version(branch: &str) -> Option<String> {
    let url = format!(
        "https://raw.githubusercontent.com/Chomiam/chomiamos-dashboard/{}/Cargo.toml",
        branch
    );
    let output = Command::new("curl")
        .args(["-s", "--connect-timeout", "3", "--max-time", "5", &url])
        .output()
        .ok()?;

    if output.status.success() {
        let content = String::from_utf8_lossy(&output.stdout);
        for line in content.lines() {
            let l = line.trim();
            if l.starts_with("version =") {
                if let Some((_, v)) = l.split_once('=') {
                    let ver = v.trim().trim_matches('"').trim_matches('\'').trim();
                    if !ver.is_empty() {
                        return Some(ver.to_string());
                    }
                }
            }
        }
    }
    None
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

    // 4. Récupération des versions distantes pour Stable (main) ET Testing (testing)
    let ls_remote_out = Command::new("git")
        .args([
            "ls-remote",
            "https://github.com/Chomiam/chomiamos-dashboard.git",
            "refs/heads/main",
            "refs/heads/testing",
        ])
        .output();

    let mut dashboard_stable_commit = None;
    let mut dashboard_testing_commit = None;
    let mut full_stable_commit = String::new();
    let mut full_testing_commit = String::new();

    if let Ok(out) = ls_remote_out {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let sha = parts[0];
                    let r = parts[1];
                    let short_sha = sha[..7.min(sha.len())].to_string();
                    if r == "refs/heads/main" {
                        dashboard_stable_commit = Some(short_sha);
                        full_stable_commit = sha.to_string();
                    } else if r == "refs/heads/testing" {
                        dashboard_testing_commit = Some(short_sha);
                        full_testing_commit = sha.to_string();
                    }
                }
            }
        }
    }

    let dashboard_stable_version = fetch_remote_cargo_version("main");
    let dashboard_testing_version = fetch_remote_cargo_version("testing");

    let mut dashboard_has_updates = false;
    let mut dashboard_locked_commit = None;

    if let Some(ref rev) = installed_commit {
        dashboard_locked_commit = Some(rev[..7.min(rev.len())].to_string());

        let target_full_commit = if dashboard_channel == "Testing" {
            &full_testing_commit
        } else {
            &full_stable_commit
        };

        if !target_full_commit.is_empty() && rev != target_full_commit {
            dashboard_has_updates = true;
        }
    }

    let dashboard_remote_commit = if dashboard_channel == "Testing" {
        dashboard_testing_commit.clone()
    } else {
        dashboard_stable_commit.clone()
    };

    let dashboard_remote_version = if dashboard_channel == "Testing" {
        dashboard_testing_version.clone()
    } else {
        dashboard_stable_version.clone()
    };

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
        dashboard_stable_version,
        dashboard_stable_commit,
        dashboard_testing_version,
        dashboard_testing_commit,
        system_needs_switch,
        current_version,
        message,
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageUpdateSummary {
    pub count: u32,
    pub has_updates: bool,
    pub status_text: String,
    pub details: Vec<String>,
    pub last_checked: String,
}

static CACHED_PACKAGE_UPDATES: Mutex<Option<(Instant, PackageUpdateSummary)>> = Mutex::new(None);

fn current_time_str() -> String {
    Command::new("date")
        .args(["+%H:%M:%S"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "maintenant".to_string())
}

fn fetch_atom_commits(owner: &str, repo: &str, branch: &str, locked_sha: &str) -> (u32, Vec<String>) {
    let url = format!(
        "https://github.com/{}/{}/commits/{}.atom",
        owner, repo, branch
    );
    let output = Command::new("curl")
        .args(["-s", "--connect-timeout", "4", "--max-time", "6", &url])
        .output();

    let mut count = 0;
    let mut details = Vec::new();

    if let Ok(out) = output {
        if out.status.success() {
            let body = String::from_utf8_lossy(&out.stdout);
            if body.contains("<feed") {
                if let Ok(re) = Regex::new(
                    r"(?s)<entry>.*?<id>[^<]*/([0-9a-f]{40})</id>.*?<title>\s*(.*?)\s*</title>.*?</entry>",
                ) {
                    for cap in re.captures_iter(&body) {
                        let sha = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                        let raw_title = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                        if sha == locked_sha {
                            break;
                        }
                        count += 1;
                        let clean_title = raw_title
                            .replace("&gt;", ">")
                            .replace("&lt;", "<")
                            .replace("&amp;", "&")
                            .replace("&quot;", "\"")
                            .replace("&#39;", "'")
                            .trim()
                            .to_string();

                        if !clean_title.is_empty() {
                            details.push(format!("{}: {}", repo, clean_title));
                        }
                    }
                }
            }
        }
    }
    (count, details)
}

fn check_git_input_has_update(
    owner: &str,
    repo: &str,
    branch: Option<&str>,
    locked_sha: &str,
) -> bool {
    let url = format!("https://github.com/{}/{}.git", owner, repo);
    let remote_ref = branch
        .map(|b| format!("refs/heads/{}", b))
        .unwrap_or_else(|| "HEAD".to_string());

    let output = Command::new("git")
        .args([
            "-c",
            "http.connectTimeout=4",
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=5",
            "ls-remote",
            &url,
            &remote_ref,
        ])
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(remote_sha) = text.split_whitespace().next() {
                return remote_sha != locked_sha;
            }
        }
    }
    false
}

pub fn get_pending_package_updates(force_refresh: bool) -> PackageUpdateSummary {
    if !force_refresh {
        if let Ok(guard) = CACHED_PACKAGE_UPDATES.lock() {
            if let Some((instant, ref cached)) = *guard {
                if instant.elapsed() < Duration::from_secs(60) {
                    return cached.clone();
                }
            }
        }
    }

    let flake_lock_path = "/etc/nixos/flake.lock";
    let lock_content = match fs::read_to_string(flake_lock_path) {
        Ok(c) => c,
        Err(_) => {
            return PackageUpdateSummary {
                count: 0,
                has_updates: false,
                status_text: "nh os switch -u".to_string(),
                details: vec!["Impossible de lire /etc/nixos/flake.lock".to_string()],
                last_checked: current_time_str(),
            };
        }
    };

    let lock_json: serde_json::Value = match serde_json::from_str(&lock_content) {
        Ok(j) => j,
        Err(_) => {
            return PackageUpdateSummary {
                count: 0,
                has_updates: false,
                status_text: "nh os switch -u".to_string(),
                details: vec!["Format JSON invalide dans flake.lock".to_string()],
                last_checked: current_time_str(),
            };
        }
    };

    let nodes = match lock_json.get("nodes").and_then(|n| n.as_object()) {
        Some(n) => n,
        None => {
            return PackageUpdateSummary {
                count: 0,
                has_updates: false,
                status_text: "nh os switch -u".to_string(),
                details: vec![],
                last_checked: current_time_str(),
            };
        }
    };

    struct InputToCheck {
        name: String,
        owner: String,
        repo: String,
        branch: Option<String>,
        locked_rev: String,
        is_nixpkgs: bool,
    }

    let mut inputs_to_check = Vec::new();
    let root_inputs = lock_json
        .get("nodes")
        .and_then(|n| n.get("root"))
        .and_then(|r| r.get("inputs"))
        .and_then(|i| i.as_object());

    if let Some(root_map) = root_inputs {
        for (input_name, node_target) in root_map {
            let target_key = node_target.as_str().unwrap_or(input_name.as_str());
            if let Some(node) = nodes.get(target_key) {
                let locked = node.get("locked");
                let original = node.get("original");

                let owner = locked
                    .and_then(|l| l.get("owner"))
                    .or_else(|| original.and_then(|o| o.get("owner")))
                    .and_then(|v| v.as_str());

                let repo = locked
                    .and_then(|l| l.get("repo"))
                    .or_else(|| original.and_then(|o| o.get("repo")))
                    .and_then(|v| v.as_str());

                let branch = original
                    .and_then(|o| o.get("ref"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let locked_rev = locked
                    .and_then(|l| l.get("rev"))
                    .and_then(|v| v.as_str());

                if let (Some(o), Some(r), Some(rev)) = (owner, repo, locked_rev) {
                    let is_nixpkgs = r.to_lowercase() == "nixpkgs";
                    inputs_to_check.push(InputToCheck {
                        name: input_name.clone(),
                        owner: o.to_string(),
                        repo: r.to_string(),
                        branch,
                        locked_rev: rev.to_string(),
                        is_nixpkgs,
                    });
                }
            }
        }
    }

    let results: Vec<(u32, Vec<String>)> = std::thread::scope(|s| {
        let mut handles = Vec::new();
        for input in &inputs_to_check {
            let handle = s.spawn(move || {
                if input.is_nixpkgs {
                    let branch = input.branch.as_deref().unwrap_or("nixos-26.05");
                    let (atom_count, atom_details) =
                        fetch_atom_commits(&input.owner, &input.repo, branch, &input.locked_rev);
                    if atom_count > 0 || !atom_details.is_empty() {
                        (atom_count, atom_details)
                    } else {
                        let changed = check_git_input_has_update(
                            &input.owner,
                            &input.repo,
                            input.branch.as_deref(),
                            &input.locked_rev,
                        );
                        if changed {
                            (1, vec![format!("{}: mises à jour disponibles", input.name)])
                        } else {
                            (0, vec![])
                        }
                    }
                } else {
                    let changed = check_git_input_has_update(
                        &input.owner,
                        &input.repo,
                        input.branch.as_deref(),
                        &input.locked_rev,
                    );
                    if changed {
                        (1, vec![format!("{}: mise à jour disponible", input.name)])
                    } else {
                        (0, vec![])
                    }
                }
            });
            handles.push(handle);
        }

        handles.into_iter().map(|h| h.join().unwrap_or((0, vec![]))).collect()
    });

    let mut total_count: u32 = 0;
    let mut all_details: Vec<String> = Vec::new();

    for (count, mut details) in results {
        total_count += count;
        all_details.append(&mut details);
    }

    let mut system_needs_switch = false;
    let sys_profile = std::path::Path::new("/nix/var/nix/profiles/system");
    if let Ok(sys_meta) = std::fs::symlink_metadata(sys_profile) {
        if let Ok(sys_time) = sys_meta.modified() {
            if let Ok(lock_meta) = std::fs::metadata(flake_lock_path) {
                if let Ok(lock_time) = lock_meta.modified() {
                    if lock_time > sys_time {
                        system_needs_switch = true;
                    }
                }
            }
        }
    }

    let (status_text, has_updates) = if total_count == 0 {
        if system_needs_switch {
            ("⚡ 1 configuration à déployer".to_string(), true)
        } else {
            ("✨ Système à jour".to_string(), false)
        }
    } else if total_count == 1 {
        ("⚡ 1 paquet à mettre à jour".to_string(), true)
    } else {
        (format!("⚡ {} paquets à mettre à jour", total_count), true)
    };

    let summary = PackageUpdateSummary {
        count: if total_count == 0 && system_needs_switch { 1 } else { total_count },
        has_updates,
        status_text,
        details: all_details,
        last_checked: current_time_str(),
    };

    if let Ok(mut guard) = CACHED_PACKAGE_UPDATES.lock() {
        *guard = Some((Instant::now(), summary.clone()));
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires network"]
    fn test_updates() {
        let res = check_system_updates();
        println!("Updates result: {:#?}", res);
        assert!(!res.current_version.is_empty());
    }

    #[test]
    #[ignore = "requires network"]
    fn test_package_updates() {
        let res = get_pending_package_updates(true);
        println!("Package updates result: {:#?}", res);
        assert!(!res.status_text.is_empty());
    }

}
