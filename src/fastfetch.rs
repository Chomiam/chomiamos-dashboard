use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::time::SystemTime;

const FASTFETCH_SCHEMA_RAW: &str = include_str!("fastfetch_schema.json");

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FastfetchError {
    pub message: String,
    pub instance_path: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub kind: String, // "syntax" | "schema"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FastfetchValidationReport {
    pub valid: bool,
    pub syntax_valid: bool,
    pub schema_valid: bool,
    pub errors: Vec<FastfetchError>,
    pub warnings: Vec<String>,
    pub cleaned_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FastfetchConfigState {
    pub active_path: String,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
    pub is_nix_store: bool,
    pub is_custom_dashboard: bool,
    pub current_content: Option<String>,
    pub backup_files: Vec<String>,
}

/// Nettoie les commentaires (// et /* */) et les virgules traînantes du JSONC
/// tout en préservant le nombre exact de lignes et de colonnes.
pub fn clean_jsonc(input: &str) -> (String, Vec<FastfetchError>) {
    let mut chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut errors = Vec::new();

    while i < len {
        let c = chars[i];

        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = true;
            i += 1;
            continue;
        }

        // Détection commentaire sur une seule ligne : // ...
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            chars[i] = ' ';
            chars[i + 1] = ' ';
            i += 2;
            while i < len && chars[i] != '\n' {
                chars[i] = ' ';
                i += 1;
            }
            continue;
        }

        // Détection commentaire multi-lignes : /* ... */
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            chars[i] = ' ';
            chars[i + 1] = ' ';
            i += 2;
            let mut closed = false;
            while i < len {
                if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                    chars[i] = ' ';
                    chars[i + 1] = ' ';
                    i += 2;
                    closed = true;
                    break;
                }
                if chars[i] != '\n' {
                    chars[i] = ' ';
                }
                i += 1;
            }
            if !closed {
                errors.push(FastfetchError {
                    message: "Commentaire multi-lignes non fermé (/* sans */)".into(),
                    instance_path: None,
                    line: None,
                    column: None,
                    kind: "syntax".into(),
                });
            }
            continue;
        }

        i += 1;
    }

    // Deuxième passe : suppression des virgules traînantes avant '}' ou ']'
    // Par exemple: ", }" -> "  }"
    let mut j = 0;
    let mut in_str2 = false;
    let mut esc2 = false;
    while j < len {
        let c = chars[j];
        if in_str2 {
            if esc2 {
                esc2 = false;
            } else if c == '\\' {
                esc2 = true;
            } else if c == '"' {
                in_str2 = false;
            }
            j += 1;
            continue;
        }

        if c == '"' {
            in_str2 = true;
            j += 1;
            continue;
        }

        if c == ',' {
            // Regarder les caractères suivants non-espaces
            let mut k = j + 1;
            while k < len && (chars[k].is_whitespace() || chars[k] == '\r' || chars[k] == '\n') {
                k += 1;
            }
            if k < len && (chars[k] == '}' || chars[k] == ']') {
                chars[j] = ' '; // Remplacer la virgule traînante par un espace
            }
        }
        j += 1;
    }

    let cleaned: String = chars.into_iter().collect();
    (cleaned, errors)
}

/// Tente de localiser un instance_path JSON Pointer dans le texte brut pour estimer ligne et colonne
fn locate_instance_path(raw_content: &str, instance_path: &str) -> (Option<usize>, Option<usize>) {
    let parts: Vec<&str> = instance_path.trim_start_matches('/').split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return (None, None);
    }

    let target_key = match parts.last() {
        Some(&k) => k,
        None => return (None, None),
    };

    let target_needle = format!("\"{}\"", target_key);
    for (line_idx, line) in raw_content.lines().enumerate() {
        if let Some(col_idx) = line.find(&target_needle) {
            return (Some(line_idx + 1), Some(col_idx + 1));
        }
    }

    (None, None)
}

/// Validation rigoureuse en 2 passes : Syntaxe JSONC + Schéma officiel Fastfetch
pub fn validate_fastfetch(raw_content: &str) -> FastfetchValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Passe 1 : Nettoyage JSONC
    let (cleaned_json, mut jsonc_errors) = clean_jsonc(raw_content);
    if !jsonc_errors.is_empty() {
        errors.append(&mut jsonc_errors);
        return FastfetchValidationReport {
            valid: false,
            syntax_valid: false,
            schema_valid: false,
            errors,
            warnings,
            cleaned_json: None,
        };
    }

    // Parsing JSON via serde_json
    let parsed_val: serde_json::Value = match serde_json::from_str(&cleaned_json) {
        Ok(v) => v,
        Err(err) => {
            let line = err.line();
            let col = err.column();
            errors.push(FastfetchError {
                message: format!("Erreur de syntaxe JSON : {}", err),
                instance_path: None,
                line: Some(line),
                column: Some(col),
                kind: "syntax".into(),
            });
            return FastfetchValidationReport {
                valid: false,
                syntax_valid: false,
                schema_valid: false,
                errors,
                warnings,
                cleaned_json: None,
            };
        }
    };

    let syntax_valid = true;

    // Passe 2 : Validation JSON Schema officiel
    let schema_val: serde_json::Value = match serde_json::from_str(FASTFETCH_SCHEMA_RAW) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("Erreur interne chargement schéma officiel : {}", e));
            return FastfetchValidationReport {
                valid: true,
                syntax_valid: true,
                schema_valid: true,
                errors,
                warnings,
                cleaned_json: Some(cleaned_json),
            };
        }
    };

    let validator = match jsonschema::validator_for(&schema_val) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("Erreur compilation schéma : {}", e));
            return FastfetchValidationReport {
                valid: true,
                syntax_valid: true,
                schema_valid: true,
                errors,
                warnings,
                cleaned_json: Some(cleaned_json),
            };
        }
    };

    let mut schema_errors = Vec::new();
    for err in validator.iter_errors(&parsed_val) {
        let path_str = err.instance_path().to_string();
        let (line, col) = locate_instance_path(raw_content, &path_str);
        schema_errors.push(FastfetchError {
            message: format!("{}", err),
            instance_path: if path_str.is_empty() { None } else { Some(path_str) },
            line,
            column: col,
            kind: "schema".into(),
        });
    }

    let schema_valid = schema_errors.is_empty();
    errors.extend(schema_errors);

    FastfetchValidationReport {
        valid: syntax_valid && schema_valid,
        syntax_valid,
        schema_valid,
        errors,
        warnings,
        cleaned_json: Some(cleaned_json),
    }
}

/// Exécute une prévisualisation isolée de fastfetch dans /tmp
pub fn preview_fastfetch(raw_content: &str) -> Result<String, String> {
    let report = validate_fastfetch(raw_content);
    if !report.valid {
        let first_err = report.errors.first().map(|e| e.message.clone()).unwrap_or_else(|| "Erreur inconnue".into());
        return Err(format!("Validation échouée. Impossible de prévisualiser : {}", first_err));
    }

    let pid = std::process::id();
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let temp_file = PathBuf::from(format!("/tmp/chomiamos-ff-preview-{}-{}.jsonc", pid, timestamp));

    if let Err(e) = fs::write(&temp_file, raw_content) {
        return Err(format!("Impossible d'écrire le fichier temporaire de prévisualisation : {}", e));
    }

    let fastfetch_bin = if Path::new("/run/current-system/sw/bin/fastfetch").exists() {
        "/run/current-system/sw/bin/fastfetch"
    } else {
        "fastfetch"
    };

    let output_res = Command::new(fastfetch_bin)
        .args(["-c", temp_file.to_str().unwrap(), "--show-errors", "--pipe", "false"])
        .output();

    // Nettoyage immédiat du fichier temporaire
    let _ = fs::remove_file(&temp_file);

    match output_res {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();

            if !stderr.is_empty() {
                if stdout.is_empty() {
                    Ok(format!("\x1b[31mErreur fastfetch :\x1b[0m\n{}", stderr))
                } else {
                    Ok(format!("{}\n\x1b[33mAvertissements fastfetch :\x1b[0m\n{}", stdout, stderr))
                }
            } else {
                Ok(stdout)
            }
        }
        Err(e) => Err(format!("Erreur lors de l'exécution de fastfetch : {}", e)),
    }
}

/// Récupère l'état actuel de la configuration Fastfetch de l'utilisateur
pub fn get_fastfetch_state() -> Result<FastfetchConfigState, String> {
    let home_dir = std::env::var("HOME").map_err(|_| "Variable d'environnement HOME introuvable".to_string())?;
    let ff_dir = PathBuf::from(&home_dir).join(".config").join("fastfetch");
    let active_path = ff_dir.join("config.jsonc");
    let active_path_str = active_path.to_string_lossy().to_string();

    let mut is_symlink = false;
    let mut symlink_target = None;
    let mut is_nix_store = false;
    let mut is_custom_dashboard = false;

    if let Ok(meta) = fs::symlink_metadata(&active_path) {
        if meta.file_type().is_symlink() {
            is_symlink = true;
            if let Ok(target) = fs::read_link(&active_path) {
                let target_str = target.to_string_lossy().to_string();
                if target_str.contains("/nix/store") {
                    is_nix_store = true;
                }
                if target_str.contains("dashboard.jsonc") {
                    is_custom_dashboard = true;
                }
                symlink_target = Some(target_str);
            }
        }
    }

    let current_content = if active_path.exists() {
        fs::read_to_string(&active_path).ok()
    } else {
        None
    };

    let mut backup_files = Vec::new();
    if let Ok(entries) = fs::read_dir(&ff_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("config.jsonc.bak.") {
                backup_files.push(name);
            }
        }
    }
    backup_files.sort_by(|a, b| b.cmp(a));

    Ok(FastfetchConfigState {
        active_path: active_path_str,
        is_symlink,
        symlink_target,
        is_nix_store,
        is_custom_dashboard,
        current_content,
        backup_files,
    })
}

/// Applique de manière sécurisée le profil Fastfetch validé
pub fn apply_fastfetch_profile(raw_content: &str) -> Result<String, String> {
    let report = validate_fastfetch(raw_content);
    if !report.valid {
        let first_err = report.errors.first().map(|e| e.message.clone()).unwrap_or_else(|| "Erreur inconnue".into());
        return Err(format!("Validation échouée. Impossible d'appliquer le profil : {}", first_err));
    }

    let home_dir = std::env::var("HOME").map_err(|_| "Variable d'environnement HOME introuvable".to_string())?;
    let ff_dir = PathBuf::from(&home_dir).join(".config").join("fastfetch");
    let profiles_dir = ff_dir.join("profiles");
    fs::create_dir_all(&profiles_dir).map_err(|e| format!("Impossible de créer le dossier de profils : {}", e))?;

    let dashboard_profile = profiles_dir.join("dashboard.jsonc");
    fs::write(&dashboard_profile, raw_content)
        .map_err(|e| format!("Impossible d'écrire le profil personnalisé : {}", e))?;

    let active_path = ff_dir.join("config.jsonc");
    let mut backup_msg = String::new();

    if let Ok(meta) = fs::symlink_metadata(&active_path) {
        if meta.file_type().is_symlink() {
            // Lien symbolique existant (Nix Store, default, etc.) -> suppression propre du lien
            let _ = fs::remove_file(&active_path);
        } else if meta.is_file() {
            // Fichier régulier existant -> sauvegarde horodatée obligatoire !
            let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let backup_path = ff_dir.join(format!("config.jsonc.bak.{}", ts));
            fs::rename(&active_path, &backup_path)
                .map_err(|e| format!("Impossible de créer la sauvegarde horodatée : {}", e))?;
            backup_msg = format!(" (Sauvegarde créée dans {})", backup_path.file_name().unwrap().to_string_lossy());
        }
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&dashboard_profile, &active_path)
            .map_err(|e| format!("Impossible de créer le lien symbolique vers le profil : {}", e))?;
    }

    Ok(format!("Profil Fastfetch appliqué avec succès !{}", backup_msg))
}

/// Rétablit le profil par défaut officiel ChomiamOS
pub fn restore_fastfetch_default() -> Result<String, String> {
    let home_dir = std::env::var("HOME").map_err(|_| "Variable d'environnement HOME introuvable".to_string())?;
    let ff_dir = PathBuf::from(&home_dir).join(".config").join("fastfetch");
    let default_profile = ff_dir.join("profiles").join("default.jsonc");
    let active_path = ff_dir.join("config.jsonc");

    if !default_profile.exists() {
        return Err("Le profil par défaut (~/.config/fastfetch/profiles/default.jsonc) est introuvable.".into());
    }

    if let Ok(meta) = fs::symlink_metadata(&active_path) {
        if meta.file_type().is_symlink() {
            let _ = fs::remove_file(&active_path);
        } else if meta.is_file() {
            let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let backup_path = ff_dir.join(format!("config.jsonc.bak.{}", ts));
            let _ = fs::rename(&active_path, &backup_path);
        }
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&default_profile, &active_path)
            .map_err(|e| format!("Impossible de créer le lien symbolique : {}", e))?;
    }

    Ok("Profil officiel ChomiamOS rétabli avec succès.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_jsonc_comments_and_trailing_commas() {
        let raw = r#"{
            // Commentaire 1
            "key": "value", // Commentaire en ligne
            /* Commentaire
               multi-lignes */
            "arr": [1, 2, 3,],
            "obj": { "nested": true, },
        }"#;

        let (cleaned, errs) = clean_jsonc(raw);
        assert!(errs.is_empty(), "Erreurs inattendues : {:?}", errs);
        let val: serde_json::Value = serde_json::from_str(&cleaned).expect("Le JSON nettoyé doit être valide");
        assert_eq!(val["key"], "value");
        assert_eq!(val["arr"].as_array().unwrap().len(), 3);
        assert_eq!(val["obj"]["nested"], true);
    }

    #[test]
    fn test_schema_validation_valid() {
        let valid_profile = r#"{
            "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
            "display": {
                "separator": "  "
            },
            "modules": [
                "title",
                "os",
                {
                    "type": "cpu",
                    "keyColor": "blue"
                }
            ]
        }"#;

        let report = validate_fastfetch(valid_profile);
        assert!(report.valid, "Le rapport devrait être valide : {:?}", report.errors);
    }

    #[test]
    #[ignore = "requires fastfetch binary (disabled in nix sandbox)"]
    fn test_preview_fastfetch_execution() {
        let profile = r#"{
            "modules": [
                "os"
            ]
        }"#;
        let res = preview_fastfetch(profile);
        assert!(res.is_ok(), "Prévisualisation doit réussir : {:?}", res.err());
        let output = res.unwrap();
        assert!(!output.is_empty(), "La sortie ne doit pas être vide");
    }

    #[test]
    fn test_schema_validation_invalid_module_type() {
        let invalid_profile = r#"{
            "modules": [
                {
                    "type": "non_existent_crazy_module_xyz"
                }
            ]
        }"#;

        let report = validate_fastfetch(invalid_profile);
        assert!(!report.valid, "Le rapport devrait échouer");
        assert!(!report.schema_valid);
        assert!(!report.errors.is_empty());
    }
}
