use serde::Serialize;
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
    let output = Command::new("nixos-rebuild")
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
    if let Ok(output) = Command::new("df").args(["-h", "/nix/store"]).output() {
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
