use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSystemdUnit {
    #[serde(default)]
    pub unit: String,
    #[serde(default)]
    pub load: String,
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub sub: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdServiceUnit {
    pub unit: String,
    pub name: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub description: String,
    pub is_user: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdOverview {
    pub total: usize,
    pub active_count: usize,
    pub inactive_count: usize,
    pub failed_count: usize,
    pub scope: String,
    pub services: Vec<SystemdServiceUnit>,
}

pub fn list_systemd_services(scope: &str) -> Result<SystemdOverview, String> {
    let is_user = scope == "user";
    let mut cmd = Command::new("systemctl");
    if is_user {
        cmd.arg("--user");
    }
    cmd.args(["list-units", "--type=service", "--all", "--output=json"]);

    let output = cmd.output().map_err(|e| format!("Erreur exécution systemctl: {}", e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("systemctl a échoué: {}", err));
    }

    let raw_json = String::from_utf8_lossy(&output.stdout);
    parse_systemd_json(&raw_json, is_user)
}

pub fn parse_systemd_json(raw_json: &str, is_user: bool) -> Result<SystemdOverview, String> {
    if raw_json.trim().is_empty() {
        return Ok(SystemdOverview {
            total: 0,
            active_count: 0,
            inactive_count: 0,
            failed_count: 0,
            scope: if is_user { "user".to_string() } else { "system".to_string() },
            services: Vec::new(),
        });
    }

    let units: Vec<RawSystemdUnit> = serde_json::from_str(raw_json)
        .map_err(|e| format!("Erreur parsing JSON systemd: {}", e))?;

    let mut services: Vec<SystemdServiceUnit> = Vec::new();
    let mut active_count = 0;
    let mut inactive_count = 0;
    let mut failed_count = 0;

    for u in units {
        // Ignorer les unités non trouvées sur le disque
        if u.load == "not-found" {
            continue;
        }

        let is_active = u.active == "active";
        let is_failed = u.active == "failed" || u.sub == "failed";

        if is_failed {
            failed_count += 1;
        } else if is_active {
            active_count += 1;
        } else {
            inactive_count += 1;
        }

        let name = u.unit.strip_suffix(".service").unwrap_or(&u.unit).to_string();

        services.push(SystemdServiceUnit {
            unit: u.unit,
            name,
            load: u.load,
            active: u.active,
            sub: u.sub,
            description: u.description,
            is_user,
        });
    }

    // Trier : Les services en échec en premier, puis les actifs, puis par nom alphabétique
    services.sort_by(|a, b| {
        let score_a = if a.active == "failed" || a.sub == "failed" {
            0
        } else if a.active == "active" {
            1
        } else {
            2
        };
        let score_b = if b.active == "failed" || b.sub == "failed" {
            0
        } else if b.active == "active" {
            1
        } else {
            2
        };

        score_a.cmp(&score_b).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let total = services.len();

    Ok(SystemdOverview {
        total,
        active_count,
        inactive_count,
        failed_count,
        scope: if is_user { "user".to_string() } else { "system".to_string() },
        services,
    })
}

fn validate_unit_name(unit: &str) -> Result<(), String> {
    if unit.is_empty() || unit.len() > 256 {
        return Err("Nom d'unité invalide".to_string());
    }
    // Caractères autorisés pour une unité systemd
    if !unit.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '@' || c == '\\') {
        return Err(format!("Nom d'unité invalide: {}", unit));
    }
    Ok(())
}

pub fn control_systemd_service(unit_name: &str, action: &str, is_user: bool) -> Result<String, String> {
    validate_unit_name(unit_name)?;

    let valid_actions = ["start", "stop", "restart", "reload"];
    if !valid_actions.contains(&action) {
        return Err(format!("Action invalide: {}. Actions valides: start, stop, restart, reload", action));
    }

    let mut cmd = Command::new("systemctl");
    if is_user {
        cmd.arg("--user");
    }
    cmd.args([action, unit_name]);

    let output = cmd.output().map_err(|e| format!("Erreur système lors de l'exécution de systemctl: {}", e))?;

    if output.status.success() {
        let action_fr = match action {
            "start" => "démarré",
            "stop" => "arrêté",
            "restart" => "redémarré",
            "reload" => "rechargé",
            _ => action,
        };
        Ok(format!("Service {} {} avec succès !", unit_name, action_fr))
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        if err.trim().is_empty() {
            let out = String::from_utf8_lossy(&output.stdout).to_string();
            Err(format!("Échec de l'action {} sur {}: {}", action, unit_name, out))
        } else {
            Err(format!("Échec de l'action {} sur {}: {}", action, unit_name, err))
        }
    }
}

pub fn get_systemd_logs(unit_name: &str, lines: u32, is_user: bool) -> Result<String, String> {
    validate_unit_name(unit_name)?;

    let line_str = lines.min(1000).to_string();
    let mut cmd = Command::new("journalctl");
    if is_user {
        cmd.arg("--user");
    }
    cmd.args(["-u", unit_name, "-n", &line_str, "--no-pager"]);

    let output = cmd.output().map_err(|e| format!("Erreur exécution journalctl: {}", e))?;

    if output.status.success() {
        let logs = String::from_utf8_lossy(&output.stdout).to_string();
        if logs.trim().is_empty() {
            Ok(format!("Aucun journal récent disponible pour {}", unit_name))
        } else {
            Ok(logs)
        }
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("Erreur récupération des logs: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_systemd_json() {
        let sample = r#"[
            {
                "unit": "bluetooth.service",
                "load": "loaded",
                "active": "active",
                "sub": "running",
                "description": "Bluetooth service"
            },
            {
                "unit": "cups.service",
                "load": "loaded",
                "active": "inactive",
                "sub": "dead",
                "description": "CUPS Scheduler"
            },
            {
                "unit": "broken.service",
                "load": "loaded",
                "active": "failed",
                "sub": "failed",
                "description": "Broken Service"
            },
            {
                "unit": "notfound.service",
                "load": "not-found",
                "active": "inactive",
                "sub": "dead",
                "description": "Not Found"
            }
        ]"#;

        let overview = parse_systemd_json(sample, false).expect("parse failed");
        assert_eq!(overview.total, 3); // notfound filtered out
        assert_eq!(overview.active_count, 1);
        assert_eq!(overview.inactive_count, 1);
        assert_eq!(overview.failed_count, 1);
        assert_eq!(overview.services[0].name, "broken"); // failed first
        assert_eq!(overview.services[1].name, "bluetooth"); // active second
        assert_eq!(overview.services[2].name, "cups"); // inactive third
    }

    #[test]
    fn test_validate_unit_name() {
        assert!(validate_unit_name("bluetooth.service").is_ok());
        assert!(validate_unit_name("app-chrome@123.service").is_ok());
        assert!(validate_unit_name("").is_err());
        assert!(validate_unit_name("service; rm -rf /").is_err());
        assert!(validate_unit_name("service | ls").is_err());
    }
}
