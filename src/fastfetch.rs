use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use std::time::SystemTime;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

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
pub struct FastfetchAssetReport {
    pub referenced_path: String,
    pub resolved_path: Option<String>,
    pub exists: bool,
    pub is_image: bool,
    pub file_size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FastfetchValidationReport {
    pub valid: bool,
    pub syntax_valid: bool,
    pub schema_valid: bool,
    pub can_preview: bool,
    pub can_apply: bool,
    pub errors: Vec<FastfetchError>,
    pub warnings: Vec<String>,
    pub asset_report: Option<FastfetchAssetReport>,
    pub cleaned_json: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FastfetchPreviewResult {
    pub stdout: String,
    pub has_image: bool,
    pub image_data_url: Option<String>,
    pub image_path: Option<String>,
    pub image_width: Option<u32>,
    pub image_height: Option<u32>,
    pub image_pad_left: Option<u32>,
    pub image_pad_top: Option<u32>,
    pub chafa_stdout: Option<String>,
    pub modules_stdout: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LogoImageInfo {
    pub path: PathBuf,
    pub data_url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pad_left: Option<u32>,
    pub pad_top: Option<u32>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FastfetchFileInfo {
    pub relative_path: String,
    pub file_size: u64,
    pub is_config: bool,
    pub is_image: bool,
    pub is_ascii: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FastfetchBundleInfo {
    pub staging_id: String,
    pub name: String,
    pub source_description: String,
    pub entry_config: String,
    pub available_configs: Vec<String>,
    pub config_content: String,
    pub files: Vec<FastfetchFileInfo>,
    pub total_size: u64,
    pub total_files: usize,
    pub has_images: bool,
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
            let mut k = j + 1;
            while k < len && (chars[k].is_whitespace() || chars[k] == '\r' || chars[k] == '\n') {
                k += 1;
            }
            if k < len && (chars[k] == '}' || chars[k] == ']') {
                chars[j] = ' ';
            }
        }
        j += 1;
    }

    let cleaned: String = chars.into_iter().collect();
    (cleaned, errors)
}

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

/// Obtient ou crée le répertoire de staging local ~/.config/fastfetch/staging/<staging_id>
pub fn get_staging_dir(staging_id: &str) -> Result<PathBuf, String> {
    let home_dir = std::env::var("HOME").map_err(|_| "Variable d'environnement HOME introuvable".to_string())?;
    let staging_base = PathBuf::from(&home_dir).join(".config").join("fastfetch").join("staging");
    let staging_dir = staging_base.join(staging_id);
    if !staging_dir.exists() {
        fs::create_dir_all(&staging_dir).map_err(|e| format!("Impossible de créer le dossier staging : {}", e))?;
    }
    Ok(staging_dir)
}

fn generate_staging_id() -> String {
    let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("stage_{}_{}", ts, std::process::id())
}

pub fn is_image_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "svg" | "gif" | "bmp" | "ico" | "avif"
        )
    } else {
        false
    }
}

pub fn is_ascii_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        matches!(ext.to_lowercase().as_str(), "txt" | "ascii" | "ans" | "art")
    } else {
        false
    }
}

pub fn mime_type_from_extension(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => "image/png",
    }
}

/// Résout un chemin de ressource Fastfetch (relatif, ~/ ou absolu)
pub fn resolve_path(raw_path: &str, staging_dir: Option<&Path>) -> Option<PathBuf> {
    let clean = raw_path.trim();
    if clean.is_empty() {
        return None;
    }

    // 1. Tester par rapport au dossier de staging s'il est fourni (chemins relatifs ./foo ou foo)
    if let Some(s_dir) = staging_dir {
        let clean_rel = clean.trim_start_matches("./");
        let candidate = s_dir.join(clean_rel);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 2. Expansion ~ ou $HOME
    let resolved_str = if clean.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            format!("{}/{}", home, &clean[2..])
        } else {
            clean.to_string()
        }
    } else if clean.starts_with("$HOME/") {
        if let Ok(home) = std::env::var("HOME") {
            format!("{}/{}", home, &clean[6..])
        } else {
            clean.to_string()
        }
    } else {
        clean.to_string()
    };

    let p = PathBuf::from(&resolved_str);
    if p.exists() {
        return Some(p);
    }

    // 3. Chemins standards dans ~/.config/fastfetch et ~/.config/fastfetch/logo
    if let Ok(home) = std::env::var("HOME") {
        let candidates = [
            PathBuf::from(&home).join(".config").join("fastfetch").join(clean),
            PathBuf::from(&home).join(".config").join("fastfetch").join("logo").join(clean),
            PathBuf::from("/etc/nixos/assets").join(clean),
        ];
        for c in &candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }
    }

    None
}

/// Scanne récursivement un dossier de bundle et extrait les fichiers, configurations et assets
pub fn scan_bundle_directory(dir: &Path) -> (Vec<FastfetchFileInfo>, Vec<String>, Option<String>, u64) {
    let mut files = Vec::new();
    let mut available_configs = Vec::new();
    let mut total_size = 0u64;

    fn walk(
        base: &Path,
        current: &Path,
        files: &mut Vec<FastfetchFileInfo>,
        configs: &mut Vec<String>,
        total_size: &mut u64,
    ) {
        if let Ok(entries) = fs::read_dir(current) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Ignorer les dossiers et fichiers parasites (.git, .DS_Store, etc.)
                if name.starts_with(".git") || name == ".DS_Store" || name.starts_with(".preview-") {
                    continue;
                }

                if path.is_dir() {
                    walk(base, &path, files, configs, total_size);
                } else if path.is_file() {
                    let rel_path = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    *total_size += size;

                    let lower = rel_path.to_lowercase();
                    let is_config = lower.ends_with(".jsonc") || lower.ends_with(".json");
                    let is_image = is_image_extension(&path);
                    let is_ascii = is_ascii_extension(&path);

                    if is_config {
                        configs.push(rel_path.clone());
                    }

                    files.push(FastfetchFileInfo {
                        relative_path: rel_path,
                        file_size: size,
                        is_config,
                        is_image,
                        is_ascii,
                    });
                }
            }
        }
    }

    walk(dir, dir, &mut files, &mut available_configs, &mut total_size);

    // Déterminer la configuration par défaut prioritaire
    // 1. config.jsonc à la racine ou le plus haut
    // 2. config.json
    // 3. Premier .jsonc trouvé
    // 4. Premier .json trouvé
    let mut entry_config = None;
    if let Some(c) = available_configs.iter().find(|c| *c == "config.jsonc") {
        entry_config = Some(c.clone());
    } else if let Some(c) = available_configs.iter().find(|c| c.ends_with("/config.jsonc")) {
        entry_config = Some(c.clone());
    } else if let Some(c) = available_configs.iter().find(|c| *c == "config.json") {
        entry_config = Some(c.clone());
    } else if let Some(c) = available_configs.iter().find(|c| c.ends_with(".jsonc")) {
        entry_config = Some(c.clone());
    } else if let Some(c) = available_configs.first() {
        entry_config = Some(c.clone());
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    available_configs.sort();

    (files, available_configs, entry_config, total_size)
}

/// Validation révisée en 3 paliers :
/// 1. Syntaxe JSONC stricte (erreurs fatales bloquantes)
/// 2. Schéma JSON officiel Fastfetch (avertissements informatifs non-bloquants)
/// 3. Intégrité des assets et logos (vérification d'existence locale/bundle)
pub fn validate_fastfetch(raw_content: &str, staging_id: Option<&str>) -> FastfetchValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Palier 1 : Nettoyage & syntaxe JSONC
    let (cleaned_json, mut jsonc_errors) = clean_jsonc(raw_content);
    if !jsonc_errors.is_empty() {
        errors.append(&mut jsonc_errors);
        return FastfetchValidationReport {
            valid: false,
            syntax_valid: false,
            schema_valid: false,
            can_preview: false,
            can_apply: false,
            errors,
            warnings,
            asset_report: None,
            cleaned_json: None,
        };
    }

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
                can_preview: false,
                can_apply: false,
                errors,
                warnings,
                asset_report: None,
                cleaned_json: None,
            };
        }
    };

    let syntax_valid = true;

    // Palier 2 : Validation JSON Schema officiel (non-bloquant pour l'exécution)
    let mut schema_errors = Vec::new();
    if let Ok(schema_val) = serde_json::from_str::<serde_json::Value>(FASTFETCH_SCHEMA_RAW) {
        if let Ok(validator) = jsonschema::validator_for(&schema_val) {
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
        } else {
            warnings.push("Schéma Fastfetch interne : compilation impossible.".into());
        }
    } else {
        warnings.push("Schéma Fastfetch interne : chargement impossible.".into());
    }

    let schema_valid = schema_errors.is_empty();
    errors.extend(schema_errors);

    // Palier 3 : Vérification des Assets et Logos référencés
    let mut asset_report = None;
    let staging_path = staging_id.and_then(|id| get_staging_dir(id).ok());

    if let Some(logo) = parsed_val.get("logo") {
        let mut logo_source: Option<String> = None;
        if let Some(s) = logo.as_str() {
            logo_source = Some(s.to_string());
        } else if let Some(obj) = logo.as_object() {
            if let Some(s) = obj.get("source").and_then(|v| v.as_str()) {
                logo_source = Some(s.to_string());
            }
        }

        if let Some(src) = logo_source {
            let is_builtin_name = [
                "arch", "ubuntu", "debian", "fedora", "nixos", "gentoo", "alpine", "void", "artix",
                "manjaro", "opensuse", "windows", "macos", "apple", "linux", "none", "small", "auto",
            ]
            .contains(&src.to_lowercase().as_str());

            if !is_builtin_name {
                let resolved = resolve_path(&src, staging_path.as_deref());
                let exists = resolved.is_some();
                let is_image = resolved.as_ref().map(|p| is_image_extension(p)).unwrap_or(false);
                let file_size = resolved.as_ref().and_then(|p| fs::metadata(p).ok()).map(|m| m.len());

                if !exists {
                    warnings.push(format!(
                        "Logo référencé « {} » introuvable dans le dossier importé ou le système.",
                        src
                    ));
                }

                asset_report = Some(FastfetchAssetReport {
                    referenced_path: src,
                    resolved_path: resolved.map(|p| p.to_string_lossy().to_string()),
                    exists,
                    is_image,
                    file_size,
                });
            }
        }
    }

    FastfetchValidationReport {
        valid: syntax_valid,
        syntax_valid,
        schema_valid,
        can_preview: syntax_valid,
        can_apply: syntax_valid,
        errors,
        warnings,
        asset_report,
        cleaned_json: Some(cleaned_json),
    }
}

pub fn extract_logo_image_info(raw_content: &str, staging_dir: Option<&Path>) -> Option<LogoImageInfo> {
    let (cleaned, _) = clean_jsonc(raw_content);
    let val: serde_json::Value = serde_json::from_str(&cleaned).ok()?;
    let logo = val.get("logo")?;

    let mut source_str: Option<String> = None;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut pad_left: Option<u32> = None;
    let mut pad_top: Option<u32> = None;

    if let Some(s) = logo.as_str() {
        source_str = Some(s.to_string());
    } else if let Some(obj) = logo.as_object() {
        if let Some(src) = obj.get("source").and_then(|v| v.as_str()) {
            source_str = Some(src.to_string());
        }
        width = obj.get("width").and_then(|v| v.as_u64()).map(|n| n as u32);
        height = obj.get("height").and_then(|v| v.as_u64()).map(|n| n as u32);
        if let Some(padding) = obj.get("padding").and_then(|v| v.as_object()) {
            pad_left = padding.get("left").and_then(|v| v.as_u64()).map(|n| n as u32);
            pad_top = padding.get("top").and_then(|v| v.as_u64()).map(|n| n as u32);
        }
    }

    let source = source_str?;
    let resolved_path = resolve_path(&source, staging_dir)?;

    if !is_image_extension(&resolved_path) {
        return None;
    }

    let bytes = fs::read(&resolved_path).ok()?;
    let mime = mime_type_from_extension(&resolved_path);
    let encoded = BASE64.encode(&bytes);
    let data_url = format!("data:{};base64,{}", mime, encoded);

    Some(LogoImageInfo {
        path: resolved_path,
        data_url,
        width,
        height,
        pad_left,
        pad_top,
    })
}

/// Exécute une prévisualisation isolée de fastfetch avec support du contexte de staging (assets relatifs)
pub fn preview_fastfetch(raw_content: &str, staging_id: Option<&str>) -> Result<FastfetchPreviewResult, String> {
    let report = validate_fastfetch(raw_content, staging_id);
    if !report.syntax_valid {
        let first_err = report.errors.iter()
            .find(|e| e.kind == "syntax")
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "Erreur inconnue".into());
        return Err(format!("Validation syntaxique échouée. Impossible de prévisualiser : {}", first_err));
    }

    let pid = std::process::id();
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);

    let staging_dir = staging_id.and_then(|id| get_staging_dir(id).ok());
    let temp_file = if let Some(ref s_dir) = staging_dir {
        s_dir.join(format!(".preview-{}-{}.jsonc", pid, timestamp))
    } else {
        PathBuf::from(format!("/tmp/chomiamos-ff-preview-{}-{}.jsonc", pid, timestamp))
    };

    if let Err(e) = fs::write(&temp_file, raw_content) {
        return Err(format!("Impossible d'écrire le fichier temporaire de prévisualisation : {}", e));
    }

    let user_profile = std::env::var("USER").ok().map(|u| format!("/etc/profiles/per-user/{}/bin/fastfetch", u));
    let mut candidates = vec![
        "/run/current-system/sw/bin/fastfetch".to_string(),
    ];
    if let Some(up) = user_profile {
        candidates.push(up);
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{}/.nix-profile/bin/fastfetch", home));
    }
    let fastfetch_bin = candidates.iter()
        .find(|p| Path::new(p).exists())
        .map(|s| s.as_str())
        .unwrap_or("fastfetch");

    let logo_info = extract_logo_image_info(raw_content, staging_dir.as_deref());

    let res = if let Some(ref info) = logo_info {
        let mut cmd_modules = Command::new(fastfetch_bin);
        cmd_modules.args(["-c", temp_file.to_str().unwrap(), "--logo", "none", "--show-errors", "--pipe", "false"])
            .envs([
                ("COLUMNS", "120"),
                ("LINES", "60"),
                ("TERM", "xterm-256color"),
            ]);
        if let Some(ref s_dir) = staging_dir {
            cmd_modules.current_dir(s_dir);
        }
        let modules_res = cmd_modules.output();

        let mut cmd_chafa = Command::new(fastfetch_bin);
        cmd_chafa.args(["-c", temp_file.to_str().unwrap(), "--logo-type", "chafa", "--show-errors", "--pipe", "false"])
            .envs([
                ("COLUMNS", "120"),
                ("LINES", "60"),
                ("TERM", "xterm-256color"),
            ]);
        if let Some(ref s_dir) = staging_dir {
            cmd_chafa.current_dir(s_dir);
        }
        let chafa_res = cmd_chafa.output();

        let modules_stdout = modules_res.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string());
        let chafa_stdout = chafa_res.ok().map(|o| String::from_utf8_lossy(&o.stdout).to_string());
        let default_stdout = modules_stdout.clone().or_else(|| chafa_stdout.clone()).unwrap_or_default();

        Ok(FastfetchPreviewResult {
            stdout: default_stdout,
            has_image: true,
            image_data_url: Some(info.data_url.clone()),
            image_path: Some(info.path.to_string_lossy().to_string()),
            image_width: info.width,
            image_height: info.height,
            image_pad_left: info.pad_left,
            image_pad_top: info.pad_top,
            chafa_stdout,
            modules_stdout,
            warning: None,
        })
    } else {
        let mut cmd = Command::new(fastfetch_bin);
        cmd.args(["-c", temp_file.to_str().unwrap(), "--show-errors", "--pipe", "false"])
            .envs([
                ("COLUMNS", "120"),
                ("LINES", "60"),
                ("TERM", "xterm-256color"),
            ]);
        if let Some(ref s_dir) = staging_dir {
            cmd.current_dir(s_dir);
        }
        let output_res = cmd.output();

        match output_res {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();

                let final_stdout = if !stderr.is_empty() {
                    if stdout.is_empty() {
                        format!("\x1b[31mErreur fastfetch :\x1b[0m\n{}", stderr)
                    } else {
                        format!("{}\n\x1b[33mAvertissements fastfetch :\x1b[0m\n{}", stdout, stderr)
                    }
                } else {
                    stdout
                };

                Ok(FastfetchPreviewResult {
                    stdout: final_stdout,
                    has_image: false,
                    image_data_url: None,
                    image_path: None,
                    image_width: None,
                    image_height: None,
                    image_pad_left: None,
                    image_pad_top: None,
                    chafa_stdout: None,
                    modules_stdout: None,
                    warning: None,
                })
            }
            Err(e) => Err(format!("Erreur lors de l'exécution de fastfetch : {}", e)),
        }
    };

    let _ = fs::remove_file(&temp_file);
    res
}

/// Importe une configuration ou bundle Fastfetch complet depuis GitHub
pub fn import_fastfetch_from_github(url_input: &str) -> Result<FastfetchBundleInfo, String> {
    let clean_url = url_input.trim();
    if clean_url.is_empty() {
        return Err("L'URL GitHub ne peut pas être vide.".into());
    }

    // Parsing GitHub URL
    let (clone_url, branch, subpath, repo_name) = parse_github_url(clean_url)?;

    let staging_id = generate_staging_id();
    let staging_dir = get_staging_dir(&staging_id)?;

    // Cloner superficiellement avec git clone --depth 1
    let mut git_args = vec!["clone", "--depth", "1"];
    if let Some(ref b) = branch {
        git_args.push("--branch");
        git_args.push(b);
    }
    git_args.push(&clone_url);
    git_args.push(staging_dir.to_str().unwrap());

    let clone_status = Command::new("git").args(&git_args).output();

    let clone_success = match clone_status {
        Ok(out) => {
            if out.status.success() {
                true
            } else {
                // Si l'échec est lié à une branche spécifique, réessayer sans --branch sur la branche par défaut
                if branch.is_some() {
                    let _ = fs::remove_dir_all(&staging_dir);
                    let _ = fs::create_dir_all(&staging_dir);
                    let fallback_out = Command::new("git")
                        .args(["clone", "--depth", "1", &clone_url, staging_dir.to_str().unwrap()])
                        .output();
                    fallback_out.map(|o| o.status.success()).unwrap_or(false)
                } else {
                    false
                }
            }
        }
        Err(e) => return Err(format!("Impossible d'exécuter git : {}", e)),
    };

    if !clone_success {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!("Échec du clonage depuis GitHub ({})", clean_url));
    }

    // Si un sous-dossier a été spécifié dans l'URL (ex: tree/main/.config/fastfetch)
    if let Some(ref sub) = subpath {
        let sub_target = staging_dir.join(sub);
        if sub_target.exists() {
            let temp_extract = staging_dir.join(".temp_extract");
            let _ = fs::create_dir_all(&temp_extract);
            copy_dir_contents(&sub_target, &temp_extract)?;

            // Vider staging_dir
            for entry in fs::read_dir(&staging_dir).map_err(|e| e.to_string())?.flatten() {
                let p = entry.path();
                if p != temp_extract {
                    if p.is_dir() {
                        let _ = fs::remove_dir_all(&p);
                    } else {
                        let _ = fs::remove_file(&p);
                    }
                }
            }
            // Déplacer le contenu extrait à la racine de staging_dir
            copy_dir_contents(&temp_extract, &staging_dir)?;
            let _ = fs::remove_dir_all(&temp_extract);
        }
    } else {
        // Détection automatique si les configurations sont dans un sous-dossier fastfetch ou .config/fastfetch
        let auto_subfolders = [".config/fastfetch", "fastfetch", "presets"];
        for auto_sub in &auto_subfolders {
            let candidate = staging_dir.join(auto_sub);
            if candidate.exists() && candidate.is_dir() {
                // Vérifier s'il contient des fichiers jsonc
                if let Ok(entries) = fs::read_dir(&candidate) {
                    let has_jsonc = entries.flatten().any(|e| {
                        let n = e.file_name().to_string_lossy().to_lowercase();
                        n.ends_with(".jsonc") || n.ends_with(".json")
                    });
                    if has_jsonc {
                        let temp_extract = staging_dir.join(".temp_extract");
                        let _ = fs::create_dir_all(&temp_extract);
                        let _ = copy_dir_contents(&candidate, &temp_extract);
                        for entry in fs::read_dir(&staging_dir).map_err(|e| e.to_string())?.flatten() {
                            let p = entry.path();
                            if p != temp_extract {
                                if p.is_dir() {
                                    let _ = fs::remove_dir_all(&p);
                                } else {
                                    let _ = fs::remove_file(&p);
                                }
                            }
                        }
                        let _ = copy_dir_contents(&temp_extract, &staging_dir);
                        let _ = fs::remove_dir_all(&temp_extract);
                        break;
                    }
                }
            }
        }
    }

    // Supprimer le dossier .git pour alléger et sécuriser le bundle
    let git_dir = staging_dir.join(".git");
    if git_dir.exists() {
        let _ = fs::remove_dir_all(&git_dir);
    }

    // Scanner le dossier final
    let (files, available_configs, entry_config_opt, total_size) = scan_bundle_directory(&staging_dir);

    let entry_config = match entry_config_opt {
        Some(c) => c,
        None => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err("Aucun fichier de configuration Fastfetch (.jsonc ou .json) n'a été trouvé dans ce dépôt.".into());
        }
    };

    let entry_path = staging_dir.join(&entry_config);
    let config_content = fs::read_to_string(&entry_path)
        .map_err(|e| format!("Impossible de lire la configuration principale '{}' : {}", entry_config, e))?;

    let has_images = files.iter().any(|f| f.is_image);

    Ok(FastfetchBundleInfo {
        staging_id,
        name: repo_name,
        source_description: format!("GitHub : {}", clean_url),
        entry_config,
        available_configs,
        config_content,
        total_files: files.len(),
        files,
        total_size,
        has_images,
    })
}

/// Importe une configuration ou bundle Fastfetch depuis un dossier local
pub fn import_fastfetch_from_local_folder(folder_path: &str) -> Result<FastfetchBundleInfo, String> {
    let src = Path::new(folder_path);
    if !src.exists() || !src.is_dir() {
        return Err(format!("Le dossier source '{}' n'existe pas ou n'est pas un dossier valide.", folder_path));
    }

    let staging_id = generate_staging_id();
    let staging_dir = get_staging_dir(&staging_id)?;

    copy_dir_contents(src, &staging_dir)?;

    let (files, available_configs, entry_config_opt, total_size) = scan_bundle_directory(&staging_dir);

    let entry_config = match entry_config_opt {
        Some(c) => c,
        None => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err("Aucun fichier de configuration Fastfetch (.jsonc ou .json) n'a été trouvé dans ce dossier.".into());
        }
    };

    let entry_path = staging_dir.join(&entry_config);
    let config_content = fs::read_to_string(&entry_path)
        .map_err(|e| format!("Impossible de lire la configuration principale '{}' : {}", entry_config, e))?;

    let folder_name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dossier_local".into());
    let has_images = files.iter().any(|f| f.is_image);

    Ok(FastfetchBundleInfo {
        staging_id,
        name: folder_name,
        source_description: format!("Dossier local : {}", folder_path),
        entry_config,
        available_configs,
        config_content,
        total_files: files.len(),
        files,
        total_size,
        has_images,
    })
}

/// Importe une archive ZIP locale
pub fn import_fastfetch_from_archive(archive_path: &str) -> Result<FastfetchBundleInfo, String> {
    let src = Path::new(archive_path);
    if !src.exists() || !src.is_file() {
        return Err(format!("L'archive '{}' n'existe pas.", archive_path));
    }

    let staging_id = generate_staging_id();
    let staging_dir = get_staging_dir(&staging_id)?;

    let unzip_out = Command::new("unzip")
        .args(["-q", "-o", archive_path, "-d", staging_dir.to_str().unwrap()])
        .output();

    match unzip_out {
        Ok(out) if out.status.success() => {},
        Ok(out) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(format!("Erreur lors de la décompression ZIP : {}", String::from_utf8_lossy(&out.stderr)));
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(format!("Commande unzip introuvable : {}", e));
        }
    }

    // Si le zip contient un seul sous-dossier racine, déballer son contenu à la racine
    if let Ok(entries) = fs::read_dir(&staging_dir) {
        let valid_entries: Vec<_> = entries.flatten().collect();
        if valid_entries.len() == 1 && valid_entries[0].path().is_dir() {
            let single_sub = valid_entries[0].path();
            let temp_extract = staging_dir.join(".temp_extract");
            let _ = fs::create_dir_all(&temp_extract);
            let _ = copy_dir_contents(&single_sub, &temp_extract);
            let _ = fs::remove_dir_all(&single_sub);
            let _ = copy_dir_contents(&temp_extract, &staging_dir);
            let _ = fs::remove_dir_all(&temp_extract);
        }
    }

    let (files, available_configs, entry_config_opt, total_size) = scan_bundle_directory(&staging_dir);

    let entry_config = match entry_config_opt {
        Some(c) => c,
        None => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err("Aucun fichier de configuration Fastfetch (.jsonc ou .json) n'a été trouvé dans cette archive.".into());
        }
    };

    let entry_path = staging_dir.join(&entry_config);
    let config_content = fs::read_to_string(&entry_path)
        .map_err(|e| format!("Impossible de lire la configuration principale '{}' : {}", entry_config, e))?;

    let archive_name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "archive.zip".into());
    let has_images = files.iter().any(|f| f.is_image);

    Ok(FastfetchBundleInfo {
        staging_id,
        name: archive_name,
        source_description: format!("Archive ZIP : {}", archive_path),
        entry_config,
        available_configs,
        config_content,
        total_files: files.len(),
        files,
        total_size,
        has_images,
    })
}

/// Bascule la configuration sélectionnée dans un bundle multi-presets (ex: presets/examples/10.jsonc)
pub fn switch_fastfetch_bundle_config(staging_id: &str, config_rel_path: &str) -> Result<FastfetchBundleInfo, String> {
    let staging_dir = get_staging_dir(staging_id)?;
    let target_file = staging_dir.join(config_rel_path);

    if !target_file.exists() {
        return Err(format!("Le fichier de configuration '{}' n'existe pas dans le bundle.", config_rel_path));
    }

    let config_content = fs::read_to_string(&target_file)
        .map_err(|e| format!("Impossible de lire la configuration '{}' : {}", config_rel_path, e))?;

    let (files, available_configs, _, total_size) = scan_bundle_directory(&staging_dir);
    let has_images = files.iter().any(|f| f.is_image);

    Ok(FastfetchBundleInfo {
        staging_id: staging_id.to_string(),
        name: staging_id.to_string(),
        source_description: format!("Preset : {}", config_rel_path),
        entry_config: config_rel_path.to_string(),
        available_configs,
        config_content,
        total_files: files.len(),
        files,
        total_size,
        has_images,
    })
}

/// Copie récursivement tous les fichiers et dossiers d'une source vers une destination
fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst).map_err(|e| format!("Impossible de créer le dossier {} : {}", dst.display(), e))?;
    }

    let entries = fs::read_dir(src).map_err(|e| format!("Impossible de lire le dossier {} : {}", src.display(), e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let target = dst.join(name);

        if path.is_dir() {
            copy_dir_contents(&path, &target)?;
        } else if path.is_file() {
            fs::copy(&path, &target).map_err(|e| format!("Impossible de copier {} vers {} : {}", path.display(), target.display(), e))?;
        }
    }
    Ok(())
}

/// Copie sécurisée des assets du bundle vers ~/.config/fastfetch/ en protégeant les liens symboliques NixOS
fn copy_bundle_assets(staging_dir: &Path, ff_dir: &Path) -> Result<usize, String> {
    let mut copied_count = 0;

    fn walk_copy(base: &Path, current: &Path, target_base: &Path, count: &mut usize) -> Result<(), String> {
        let entries = fs::read_dir(current).map_err(|e| format!("Erreur lecture dossier : {}", e))?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with(".git") || name.starts_with(".preview-") || name == "config.jsonc" {
                continue;
            }

            let rel_path = path.strip_prefix(base).unwrap_or(&path);
            let dest_path = target_base.join(rel_path);

            if path.is_dir() {
                if !dest_path.exists() {
                    fs::create_dir_all(&dest_path).map_err(|e| format!("Impossible de créer {} : {}", dest_path.display(), e))?;
                }
                walk_copy(base, &path, target_base, count)?;
            } else if path.is_file() {
                // Sécurité : Ne JAMAIS écraser un lien symbolique Nix Store !
                if let Ok(meta) = fs::symlink_metadata(&dest_path) {
                    if meta.file_type().is_symlink() {
                        if let Ok(link) = fs::read_link(&dest_path) {
                            if link.to_string_lossy().contains("/nix/store") {
                                // Fichier officiel NixOS protégé
                                continue;
                            }
                        }
                    } else if meta.is_file() {
                        // Fichier régulier existant -> Sauvegarde horodatée de sécurité
                        let ts = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                        let backup = format!("{}.bak.{}", dest_path.to_string_lossy(), ts);
                        let _ = fs::rename(&dest_path, &backup);
                    }
                }

                if let Some(parent) = dest_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                fs::copy(&path, &dest_path).map_err(|e| format!("Erreur copie {} vers {} : {}", path.display(), dest_path.display(), e))?;
                *count += 1;
            }
        }
        Ok(())
    }

    walk_copy(staging_dir, staging_dir, ff_dir, &mut copied_count)?;
    Ok(copied_count)
}

fn parse_github_url(url: &str) -> Result<(String, Option<String>, Option<String>, String), String> {
    let u = url.trim();

    // Raccourci owner/repo ou owner/repo@branch
    if !u.starts_with("http://") && !u.starts_with("https://") {
        if u.contains('/') {
            let parts: Vec<&str> = u.split('@').collect();
            let repo_full = parts[0].trim();
            let branch = if parts.len() > 1 { Some(parts[1].trim().to_string()) } else { None };
            let name_parts: Vec<&str> = repo_full.split('/').collect();
            let repo_name = name_parts.last().unwrap_or(&"repo").to_string();
            return Ok((format!("https://github.com/{}.git", repo_full), branch, None, repo_name));
        }
    }

    let clean = u.trim_end_matches('/');
    let re = regex::Regex::new(r"^https?://github\.com/([^/]+)/([^/]+?)(?:\.git)?(?:/(?:tree|blob)/([^/]+)/(.+))?/?$")
        .map_err(|e| format!("Erreur regex : {}", e))?;

    if let Some(caps) = re.captures(clean) {
        let owner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let repo = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let branch = caps.get(3).map(|m| m.as_str().to_string());
        let subpath = caps.get(4).map(|m| m.as_str().to_string());

        let clone_url = format!("https://github.com/{}/{}.git", owner, repo);
        let repo_name = repo.to_string();
        return Ok((clone_url, branch, subpath, repo_name));
    }

    Err(format!("Format d'URL GitHub non reconnu : '{}'. Exemples valides : https://github.com/owner/repo ou https://github.com/owner/repo/tree/branch/subfolder", u))
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

/// Applique de manière sécurisée le profil Fastfetch validé (avec ses assets compagnons éventuels)
pub fn apply_fastfetch_profile(raw_content: &str, staging_id: Option<&str>) -> Result<String, String> {
    let report = validate_fastfetch(raw_content, staging_id);
    if !report.syntax_valid {
        let first_err = report.errors.iter()
            .find(|e| e.kind == "syntax")
            .map(|e| e.message.clone())
            .unwrap_or_else(|| "Erreur de syntaxe JSON inconnue".into());
        return Err(format!("Syntaxe JSONC invalide. Impossible d'appliquer le profil : {}", first_err));
    }

    let home_dir = std::env::var("HOME").map_err(|_| "Variable d'environnement HOME introuvable".to_string())?;
    let ff_dir = PathBuf::from(&home_dir).join(".config").join("fastfetch");
    let profiles_dir = ff_dir.join("profiles");
    fs::create_dir_all(&profiles_dir).map_err(|e| format!("Impossible de créer le dossier de profils : {}", e))?;

    let dashboard_profile = profiles_dir.join("dashboard.jsonc");
    fs::write(&dashboard_profile, raw_content)
        .map_err(|e| format!("Impossible d'écrire le profil personnalisé : {}", e))?;

    let mut installed_assets_count = 0;
    if let Some(s_id) = staging_id {
        if let Ok(staging_dir) = get_staging_dir(s_id) {
            if staging_dir.exists() {
                installed_assets_count = copy_bundle_assets(&staging_dir, &ff_dir)?;
            }
        }
    }

    let active_path = ff_dir.join("config.jsonc");
    let mut backup_msg = String::new();

    if let Ok(meta) = fs::symlink_metadata(&active_path) {
        if meta.file_type().is_symlink() {
            let _ = fs::remove_file(&active_path);
        } else if meta.is_file() {
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

    let assets_msg = if installed_assets_count > 0 {
        format!(" ({} fichier(s) compagnons et dossiers d'assets installés)", installed_assets_count)
    } else {
        String::new()
    };

    Ok(format!("Profil Fastfetch appliqué avec succès !{}{}", assets_msg, backup_msg))
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
    fn test_extract_logo_image_info() {
        let profile = r#"{
            "logo": {
                "type": "kitty-direct",
                "source": "~/.config/fastfetch/logo/chomiamos_logo.png",
                "width": 40,
                "height": 20,
                "padding": { "left": 2, "top": 1 }
            }
        }"#;
        if let Some(info) = extract_logo_image_info(profile, None) {
            assert!(info.data_url.starts_with("data:image/"));
            assert_eq!(info.width, Some(40));
            assert_eq!(info.height, Some(20));
        }
    }

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

        let report = validate_fastfetch(valid_profile, None);
        assert!(report.valid, "Le rapport devrait être valide : {:?}", report.errors);
        assert!(report.syntax_valid);
        assert!(report.schema_valid);
    }

    #[test]
    fn test_schema_warning_does_not_block_execution() {
        let profile_with_unknown_module = r#"{
            "modules": [
                {
                    "type": "non_existent_crazy_module_xyz"
                }
            ]
        }"#;

        let report = validate_fastfetch(profile_with_unknown_module, None);
        // La syntaxe JSONC est valide, donc can_preview et can_apply sont autorisés !
        assert!(report.syntax_valid, "La syntaxe JSONC doit être valide");
        assert!(report.can_preview, "La prévisualisation doit être permise");
        assert!(!report.schema_valid, "Le schéma officiel doit signaler l'incompatibilité");
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn test_bundle_scan_and_switch() {
        let tmp = std::env::temp_dir().join(format!("test_bundle_{}", std::process::id()));
        let _ = fs::create_dir_all(&tmp);
        let sub = tmp.join("images");
        let _ = fs::create_dir_all(&sub);

        let cfg1 = tmp.join("config.jsonc");
        let _ = fs::write(&cfg1, r#"{"modules": ["os"]}"#);
        let cfg2 = tmp.join("alt.json");
        let _ = fs::write(&cfg2, r#"{"modules": ["host"]}"#);
        let img = sub.join("logo.png");
        let _ = fs::write(&img, b"fake png bytes");

        let (files, configs, entry, _size) = scan_bundle_directory(&tmp);
        assert_eq!(files.len(), 3);
        assert_eq!(configs.len(), 2);
        assert_eq!(entry, Some("config.jsonc".into()));
        assert!(files.iter().any(|f| f.is_image && f.relative_path == "images/logo.png"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_parse_github_urls() {
        let res1 = parse_github_url("https://github.com/Chick207/fastfetch-presets");
        assert!(res1.is_ok());
        let (clone_url, branch, subpath, name) = res1.unwrap();
        assert_eq!(clone_url, "https://github.com/Chick207/fastfetch-presets.git");
        assert!(branch.is_none());
        assert!(subpath.is_none());
        assert_eq!(name, "fastfetch-presets");

        let res2 = parse_github_url("https://github.com/user/dotfiles/tree/main/.config/fastfetch");
        assert!(res2.is_ok());
        let (clone_url2, branch2, subpath2, name2) = res2.unwrap();
        assert_eq!(clone_url2, "https://github.com/user/dotfiles.git");
        assert_eq!(branch2, Some("main".into()));
        assert_eq!(subpath2, Some(".config/fastfetch".into()));
        assert_eq!(name2, "dotfiles");

        let res3 = parse_github_url("catppuccin/fastfetch@v0.2");
        assert!(res3.is_ok());
        let (clone_url3, branch3, _, name3) = res3.unwrap();
        assert_eq!(clone_url3, "https://github.com/catppuccin/fastfetch.git");
        assert_eq!(branch3, Some("v0.2".into()));
        assert_eq!(name3, "fastfetch");
    }
}
