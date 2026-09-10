use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct NixGeneration {
    pub id: u32,
    pub date: String,
    pub nixos_version: String,
    pub kernel: String,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GenerationsSummary {
    pub count: usize,
    pub active_generation: Option<u32>,
    pub store_size: String,
    pub generations: Vec<NixGeneration>,
}

pub fn list_generations() -> Result<GenerationsSummary, String> {
    let cmd_name = if Path::new("/run/current-system/sw/bin/nixos-rebuild").exists() {
        "/run/current-system/sw/bin/nixos-rebuild"
    } else {
        "nixos-rebuild"
    };

    let output = Command::new(cmd_name)
        .arg("list-generations")
        .output()
        .map_err(|e| format!("Erreur lors de l'exécution de nixos-rebuild: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut gens = Vec::new();
    let mut active = None;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Generation") || trimmed.starts_with("---") {
            continue;
        }

        // Columns: ID Date Time Version Kernel Config Specialisation Current
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 6 {
            if let Ok(id) = parts[0].parse::<u32>() {
                let date = format!("{} {}", parts[1], parts[2]);
                let nixos_version = parts[3].to_string();
                let kernel = parts[4].to_string();
                let current = parts.last().map(|&s| s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("current")).unwrap_or(false);

                if current {
                    active = Some(id);
                }

                gens.push(NixGeneration {
                    id,
                    date,
                    nixos_version,
                    kernel,
                    current,
                });
            }
        }
    }

    // Sort descending by ID
    gens.sort_by(|a, b| b.id.cmp(&a.id));

    // Get /nix/store disk size
    let store_size = get_store_size();

    Ok(GenerationsSummary {
        count: gens.len(),
        active_generation: active,
        store_size,
        generations: gens,
    })
}

fn get_store_size() -> String {
    let df_cmd = if Path::new("/run/current-system/sw/bin/df").exists() {
        "/run/current-system/sw/bin/df"
    } else {
        "df"
    };

    if let Ok(output) = Command::new(df_cmd).args(["-h", "/nix/store"]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                return format!("{} utilisés (sur {})", parts[2], parts[1]);
            }
        }
    }
    "N/A".to_string()
}

/// Supprime les générations NixOS spécifiées par leurs IDs.
/// Utilise `nix-env --profile /nix/var/nix/profiles/system --delete-generations`.
pub fn delete_generations(ids: Vec<u32>) -> Result<String, String> {
    if ids.is_empty() {
        return Err("Aucune génération sélectionnée.".to_string());
    }

    // Vérifier qu'on ne supprime pas la génération active
    if let Ok(summary) = list_generations() {
        if let Some(active_id) = summary.active_generation {
            if ids.contains(&active_id) {
                return Err(format!(
                    "Impossible de supprimer la génération active (#{}).",
                    active_id
                ));
            }
        }
    }

    let ids_str: Vec<String> = ids.iter().map(|id| id.to_string()).collect();

    let nix_env = if Path::new("/run/current-system/sw/bin/nix-env").exists() {
        "/run/current-system/sw/bin/nix-env"
    } else {
        "nix-env"
    };

    let switch_cmd = if Path::new("/run/current-system/bin/switch-to-configuration").exists() {
        "/run/current-system/bin/switch-to-configuration"
    } else {
        "switch-to-configuration"
    };

    // Élévation des privilèges via pkexec pour modifier le profil système et actualiser le bootloader
    let script = format!(
        "'{nix_env}' --profile /nix/var/nix/profiles/system --delete-generations {ids} && '{switch_cmd}' boot",
        nix_env = nix_env,
        ids = ids_str.join(" "),
        switch_cmd = switch_cmd
    );

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let trimmed = stderr.trim();
        if trimmed.contains("dismissed") || trimmed.contains("canceled") || trimmed.contains("annulé") {
            return Err("Action annulée par l'utilisateur.".to_string());
        }
        return Err(format!("Échec de la suppression : {}", trimmed));
    }

    let count = ids.len();
    Ok(format!(
        "{} génération{} supprimée{} avec succès.",
        count,
        if count > 1 { "s" } else { "" },
        if count > 1 { "s" } else { "" }
    ))
}

/// Active une génération NixOS spécifique pour le prochain reboot.
/// Utilise le lien du profil système pour exécuter `switch-to-configuration boot`.
pub fn switch_to_generation(id: u32) -> Result<String, String> {
    // Vérifier que la génération existe
    let profile_link = format!("/nix/var/nix/profiles/system-{}-link", id);
    if !Path::new(&profile_link).exists() {
        return Err(format!(
            "La génération #{} n'existe pas (profil introuvable : {}).",
            id, profile_link
        ));
    }

    // Pointer le profil système vers cette génération
    let nix_env = if Path::new("/run/current-system/sw/bin/nix-env").exists() {
        "/run/current-system/sw/bin/nix-env"
    } else {
        "nix-env"
    };

    // Activer la configuration pour le prochain reboot
    let switch_cmd = format!("{}/bin/switch-to-configuration", profile_link);

    // Élévation des privilèges via pkexec en une seule demande pour le profil système et le bootloader
    let script = format!(
        "'{nix_env}' --profile /nix/var/nix/profiles/system --switch-generation {id} && '{switch_cmd}' boot",
        nix_env = nix_env,
        id = id,
        switch_cmd = switch_cmd
    );

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let trimmed = stderr.trim();
        if trimmed.contains("dismissed") || trimmed.contains("canceled") || trimmed.contains("annulé") {
            return Err("Action annulée par l'utilisateur.".to_string());
        }
        return Err(format!("Échec du changement de génération : {}", trimmed));
    }

    Ok(format!(
        "Génération #{} activée. Elle sera chargée au prochain redémarrage.",
        id
    ))
}
