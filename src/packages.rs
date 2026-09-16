use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub const CUSTOM_PACKAGES_PATH: &str = "/etc/nixos/custom-packages.nix";
const ES_STABLE_INDEX: &str = "latest-51-nixos-26.05";
const ES_UNSTABLE_INDEX: &str = "latest-51-nixos-unstable";
const ES_AUTH: &str = "aWVSALXpZv:X8gPHnzL52wFEekuxsfQ9cSh";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomPackages {
    pub stable: Vec<String>,
    pub unstable: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEntry {
    pub attr_name: String,
    pub pname: String,
    pub description: String,
    pub stable_version: Option<String>,
    pub unstable_version: Option<String>,
    pub is_custom_stable: bool,
    pub is_custom_unstable: bool,
    pub is_system_conflict: bool,
    pub conflict_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagesState {
    pub custom: CustomPackages,
    pub custom_details: Vec<PackageEntry>,
    pub system_conflicts: HashMap<String, String>,
}

pub fn get_custom_packages_path() -> PathBuf {
    PathBuf::from(CUSTOM_PACKAGES_PATH)
}

/// S'assure que le fichier /etc/nixos/custom-packages.nix existe.
/// Si vierge/inexistant, il est créé avec un template propre.
/// Si déjà présent, il n'est absolument pas écrasé.
pub fn ensure_custom_packages_file() -> Result<PathBuf, String> {
    let path = get_custom_packages_path();
    if !path.exists() {
        let virgin_content = r#"{
  # =========================================================================
  # 📦 PAQUETS NIX PERSONNALISÉS (CHOMIAMOS)
  # =========================================================================
  # Ce fichier est géré par l'onglet Logithèque du Dashboard ChomiamOS.
  # Vous pouvez également y ajouter ou supprimer des paquets manuellement.
  #
  # - stable   : Paquets issus de la branche stable (NixOS 26.05)
  # - unstable : Paquets issus de la branche unstable (dernières nouveautés)
  # =========================================================================

  # Paquets issus de la branche Stable (NixOS 26.05)
  stable = [
  ];

  # Paquets issus de la branche Unstable (Dernières versions)
  unstable = [
  ];
}
"#;
        if let Err(e) = fs::write(&path, virgin_content) {
            let status = Command::new("pkexec")
                .args(["tee", path.to_str().unwrap()])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .and_then(|mut child| {
                    if let Some(mut stdin) = child.stdin.take() {
                        use std::io::Write;
                        let _ = stdin.write_all(virgin_content.as_bytes());
                    }
                    child.wait()
                });
            if status.is_err() || !status.unwrap().success() {
                return Err(format!("Impossible de créer {}: {}", path.display(), e));
            }
        }
    }
    Ok(path)
}

/// Lit la liste des paquets stable et unstable configurés dans custom-packages.nix
pub fn read_custom_packages() -> Result<CustomPackages, String> {
    let path = ensure_custom_packages_file()?;
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Impossible de lire {}: {}", path.display(), e))?;

    let stable = extract_package_list(&content, "stable");
    let unstable = extract_package_list(&content, "unstable");

    Ok(CustomPackages { stable, unstable })
}

fn extract_package_list(content: &str, list_name: &str) -> Vec<String> {
    let pattern = format!("{} = [", list_name);
    let start_pos = match content.find(&pattern) {
        Some(pos) => pos + pattern.len(),
        None => return Vec::new(),
    };

    let remaining = &content[start_pos..];
    let end_pos = match remaining.find(']') {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let block = &remaining[..end_pos];
    let mut packages = Vec::new();

    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let clean = if let Some(idx) = trimmed.find('#') {
            trimmed[..idx].trim()
        } else {
            trimmed
        };
        let unquoted = clean.trim_matches(|c| c == '"' || c == '\'' || c == ';' || c == ',');
        if !unquoted.is_empty() {
            packages.push(unquoted.to_string());
        }
    }

    packages
}

/// Enregistre les listes custom dans /etc/nixos/custom-packages.nix
pub fn save_custom_packages(custom: &CustomPackages) -> Result<(), String> {
    let path = ensure_custom_packages_file()?;

    let mut stable_clean = custom.stable.clone();
    stable_clean.retain(|s| !s.trim().is_empty());
    stable_clean.sort();
    stable_clean.dedup();

    let mut unstable_clean = custom.unstable.clone();
    unstable_clean.retain(|s| !s.trim().is_empty());
    unstable_clean.sort();
    unstable_clean.dedup();

    let stable_lines = if stable_clean.is_empty() {
        String::new()
    } else {
        stable_clean
            .iter()
            .map(|s| format!("    \"{}\"\n", s))
            .collect::<String>()
    };

    let unstable_lines = if unstable_clean.is_empty() {
        String::new()
    } else {
        unstable_clean
            .iter()
            .map(|s| format!("    \"{}\"\n", s))
            .collect::<String>()
    };

    let new_content = format!(
r#"{{
  # =========================================================================
  # 📦 PAQUETS NIX PERSONNALISÉS (CHOMIAMOS)
  # =========================================================================
  # Ce fichier est géré par l'onglet Logithèque du Dashboard ChomiamOS.
  # Vous pouvez également y ajouter ou supprimer des paquets manuellement.
  #
  # - stable   : Paquets issus de la branche stable (NixOS 26.05)
  # - unstable : Paquets issus de la branche unstable (dernières nouveautés)
  # =========================================================================

  # Paquets issus de la branche Stable (NixOS 26.05)
  stable = [
{}  ];

  # Paquets issus de la branche Unstable (Dernières versions)
  unstable = [
{}  ];
}}
"#,
        stable_lines, unstable_lines
    );

    let tmp_path = path.with_extension("nix.tmp");
    if let Err(_) = fs::write(&tmp_path, &new_content) {
        let mut child = Command::new("pkexec")
            .args(["tee", path.to_str().unwrap()])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Impossible d'écrire avec privilèges: {}", e))?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = stdin.write_all(new_content.as_bytes());
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Échec de l'écriture du fichier custom-packages.nix".into());
        }
    } else {
        fs::rename(&tmp_path, &path)
            .map_err(|e| format!("Impossible de remplacer {}: {}", path.display(), e))?;
    }

    Ok(())
}

/// Identifie l'ensemble des paquets système déjà actifs pour prévenir les conflits / doublons
pub fn get_system_conflicts() -> HashMap<String, String> {
    let mut conflicts = HashMap::new();

    let core_tools = [
        ("kitty", "Terminal officiel préinstallé de ChomiamOS"),
        ("fastfetch", "Utilitaire d'informations système officiel"),
        ("git", "Outil de gestion de versions de base"),
        ("gh", "Outil CLI GitHub officiel"),
        ("nh", "Gestionnaire de paquets et mises à jour ChomiamOS"),
        ("nvd", "Outil de comparaison de versions NixOS"),
        ("btop", "Moniteur système de base"),
        ("pciutils", "Outil de diagnostic matériel (lspci)"),
        ("usbutils", "Outil de diagnostic matériel (lsusb)"),
        ("util-linux", "Utilitaires système fondamentaux Linux"),
        ("coreutils", "Utilitaires de base GNU Coreutils"),
        ("xdg-utils", "Intégration d'environnement de bureau XDG"),
        ("bash", "Interpréteur de commandes de base"),
        ("zsh", "Shell alternatif préconfiguré"),
        ("curl", "Outil de transfert réseau"),
        ("wget", "Outil de téléchargement réseau"),
        ("gzip", "Utilitaire d'archive système"),
        ("tar", "Utilitaire d'archive système"),
        ("unzip", "Utilitaire d'archive système"),
    ];
    for (pkg, desc) in core_tools {
        conflicts.insert(pkg.to_string(), desc.to_string());
    }

    let vars_path = crate::config::get_vars_path();
    if let Ok(config) = crate::config::read_vars_nix(&vars_path) {
        if config.media.vlc {
            conflicts.insert("vlc".into(), "Déjà actif dans /etc/nixos/vars.nix (Multimédia)".into());
        }
        if config.media.mpv {
            conflicts.insert("mpv".into(), "Déjà actif dans /etc/nixos/vars.nix (Multimédia)".into());
        }
        if config.media.stremio {
            conflicts.insert("stremio".into(), "Déjà actif dans /etc/nixos/vars.nix (Multimédia)".into());
        }
        if config.media.tailscale {
            conflicts.insert("tailscale".into(), "Déjà actif dans /etc/nixos/vars.nix (Réseau & VPN)".into());
        }
        if config.media.localsend {
            conflicts.insert("localsend".into(), "Déjà actif dans /etc/nixos/vars.nix (Partage)".into());
        }
        if config.media.motrix {
            conflicts.insert("motrix".into(), "Déjà actif dans /etc/nixos/vars.nix (Téléchargement)".into());
        }
        if config.creation.blender {
            conflicts.insert("blender".into(), "Déjà actif dans /etc/nixos/vars.nix (Création 3D)".into());
        }
        if config.creation.godot {
            conflicts.insert("godot".into(), "Déjà actif dans /etc/nixos/vars.nix (Moteur Godot)".into());
            conflicts.insert("godot_4".into(), "Déjà actif dans /etc/nixos/vars.nix (Moteur Godot 4)".into());
        }
        if config.creation.kdenlive {
            conflicts.insert("kdenlive".into(), "Déjà actif dans /etc/nixos/vars.nix (Montage vidéo)".into());
        }
        if config.creation.obs_studio {
            conflicts.insert("obs-studio".into(), "Déjà actif dans /etc/nixos/vars.nix (Streaming OBS)".into());
        }
        if config.gaming.steam {
            conflicts.insert("steam".into(), "Déjà actif dans /etc/nixos/vars.nix (Gaming Steam)".into());
        }
        if config.gaming.lutris {
            conflicts.insert("lutris".into(), "Déjà actif dans /etc/nixos/vars.nix (Gaming Lutris)".into());
        }
        if config.gaming.heroic {
            conflicts.insert("heroic".into(), "Déjà actif dans /etc/nixos/vars.nix (Gaming Heroic)".into());
        }
        if config.browser == "chrome" {
            conflicts.insert("google-chrome".into(), "Navigateur par défaut configuré dans vars.nix".into());
        } else if config.browser == "firefox" {
            conflicts.insert("firefox".into(), "Navigateur par défaut configuré dans vars.nix".into());
        } else if config.browser == "librewolf" {
            conflicts.insert("librewolf".into(), "Navigateur par défaut configuré dans vars.nix".into());
        }
        if config.discord_client == "discord" {
            conflicts.insert("discord".into(), "Client Discord configuré dans vars.nix".into());
        }
        if config.emulation.dolphin {
            conflicts.insert("dolphin-emu".into(), "Émulateur Dolphin configuré dans vars.nix".into());
        }
        if config.emulation.duckstation {
            conflicts.insert("duckstation".into(), "Émulateur PlayStation 1 DuckStation configuré dans vars.nix".into());
        }
        if config.emulation.pcsx2 {
            conflicts.insert("pcsx2".into(), "Émulateur PCSX2 configuré dans vars.nix".into());
        }
        if config.emulation.ppsspp {
            conflicts.insert("ppsspp".into(), "Émulateur PPSSPP configuré dans vars.nix".into());
        }
        if config.emulation.melonds {
            conflicts.insert("melonds".into(), "Émulateur melonDS configuré dans vars.nix".into());
        }
        if config.emulation.mgba {
            conflicts.insert("mgba".into(), "Émulateur mGBA configuré dans vars.nix".into());
        }
        if config.emulation.rpcs3 {
            conflicts.insert("rpcs3".into(), "Émulateur RPCS3 configuré dans vars.nix".into());
        }
        if config.emulation.xemu {
            conflicts.insert("xemu".into(), "Émulateur xemu (Xbox) configuré dans vars.nix".into());
        }
        if config.emulation.eden {
            conflicts.insert("eden".into(), "Émulateur Switch Eden configuré dans vars.nix".into());
        }
        if config.emulation.azahar {
            conflicts.insert("azahar".into(), "Émulateur 3DS Azahar configuré dans vars.nix".into());
        }
        if config.creation.antigravity {
            conflicts.insert("antigravity-ide".into(), "IDE Antigravity configuré dans vars.nix".into());
        }
        if config.creation.zed {
            conflicts.insert("zed-editor".into(), "IDE Zed configuré dans vars.nix".into());
        }
        if config.creation.vscode {
            conflicts.insert("vscode".into(), "IDE Visual Studio Code configuré dans vars.nix".into());
        }
    }

    conflicts
}

/// Envoi direct d'une requête HTTP vers l'Elasticsearch officiel de NixOS Search
fn es_query(index: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    let url = format!("https://search.nixos.org/backend/{}/_search", index);
    let body_str = body.to_string();

    let output = Command::new("curl")
        .args([
            "-s",
            "--max-time", "6",
            "-u", ES_AUTH,
            "-X", "POST",
            &url,
            "-H", "Content-Type: application/json",
            "-d", &body_str,
        ])
        .output()
        .map_err(|e| format!("Erreur lors de l'appel à curl : {}", e))?;

    if !output.status.success() {
        return Err(format!("Échec requête NixOS Search ({})", output.status));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Réponse JSON invalide de NixOS Search : {}", e))?;

    Ok(json)
}

#[derive(Default)]
struct RawPackageHit {
    attr_name: String,
    pname: String,
    version: String,
    description: String,
}

fn parse_hits(json: &serde_json::Value) -> Vec<RawPackageHit> {
    let mut results = Vec::new();
    if let Some(hits) = json.pointer("/hits/hits").and_then(|h| h.as_array()) {
        for hit in hits {
            if let Some(source) = hit.get("_source") {
                let attr_name = source
                    .get("package_attr_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let pname = source
                    .get("package_pname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let version = source
                    .get("package_pversion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = source
                    .get("package_description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !attr_name.is_empty() {
                    results.push(RawPackageHit {
                        attr_name,
                        pname,
                        version,
                        description,
                    });
                }
            }
        }
    }
    results
}

/// Recherche en direct dans Nixpkgs (Stable & Unstable) et compare les versions
pub async fn search_nixpkgs(query: String) -> Result<Vec<PackageEntry>, String> {
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let custom = read_custom_packages().unwrap_or_default();
    let conflicts = get_system_conflicts();

    let search_body = serde_json::json!({
        "size": 35,
        "query": {
            "bool": {
                "should": [
                    {
                        "term": {
                            "package_attr_name": {
                                "value": trimmed,
                                "boost": 10.0
                            }
                        }
                    },
                    {
                        "multi_match": {
                            "query": trimmed,
                            "fields": [
                                "package_attr_name^4",
                                "package_pname^3",
                                "package_description"
                            ],
                            "fuzziness": "AUTO"
                        }
                    }
                ]
            }
        },
        "_source": [
            "package_pname",
            "package_attr_name",
            "package_pversion",
            "package_description"
        ]
    });

    // Requêtes parallèles sur Stable (26.05) et Unstable
    let body_stable = search_body.clone();
    let body_unstable = search_body;

    let stable_task = tokio::task::spawn_blocking(move || {
        es_query(ES_STABLE_INDEX, &body_stable)
    });
    let unstable_task = tokio::task::spawn_blocking(move || {
        es_query(ES_UNSTABLE_INDEX, &body_unstable)
    });

    let (stable_res, unstable_res) = tokio::join!(stable_task, unstable_task);

    let stable_hits = stable_res
        .map_err(|e| e.to_string())?
        .map(|j| parse_hits(&j))
        .unwrap_or_default();

    let unstable_hits = unstable_res
        .map_err(|e| e.to_string())?
        .map(|j| parse_hits(&j))
        .unwrap_or_default();

    // Indexation des versions stables et unstables par attr_name
    let mut stable_map: HashMap<String, RawPackageHit> = HashMap::new();
    for hit in &stable_hits {
        stable_map.entry(hit.attr_name.clone()).or_insert_with(|| RawPackageHit {
            attr_name: hit.attr_name.clone(),
            pname: hit.pname.clone(),
            version: hit.version.clone(),
            description: hit.description.clone(),
        });
    }

    let mut unstable_map: HashMap<String, RawPackageHit> = HashMap::new();
    for hit in &unstable_hits {
        unstable_map.entry(hit.attr_name.clone()).or_insert_with(|| RawPackageHit {
            attr_name: hit.attr_name.clone(),
            pname: hit.pname.clone(),
            version: hit.version.clone(),
            description: hit.description.clone(),
        });
    }

    // Récupération ordonnée des noms de paquets
    let mut ordered_names = Vec::new();
    let mut seen = HashSet::new();

    // 1. D'abord les hits stables dans l'ordre exact de pertinence retourné par ES
    for hit in &stable_hits {
        if seen.insert(hit.attr_name.clone()) {
            ordered_names.push(hit.attr_name.clone());
        }
    }
    // 2. Ensuite les hits unstables (paquets qui n'existeraient qu'en unstable)
    for hit in &unstable_hits {
        if seen.insert(hit.attr_name.clone()) {
            ordered_names.push(hit.attr_name.clone());
        }
    }

    // Recherche de versions complémentaires si un paquet manque dans l'un des deux canaux
    let missing_in_unstable: Vec<String> = ordered_names
        .iter()
        .filter(|name| !unstable_map.contains_key(*name))
        .cloned()
        .collect();

    if !missing_in_unstable.is_empty() {
        let terms_body = serde_json::json!({
            "size": missing_in_unstable.len(),
            "query": {
                "terms": {
                    "package_attr_name": missing_in_unstable
                }
            },
            "_source": ["package_pname", "package_attr_name", "package_pversion", "package_description"]
        });
        if let Ok(json) = es_query(ES_UNSTABLE_INDEX, &terms_body) {
            for hit in parse_hits(&json) {
                unstable_map.insert(hit.attr_name.clone(), hit);
            }
        }
    }

    let missing_in_stable: Vec<String> = ordered_names
        .iter()
        .filter(|name| !stable_map.contains_key(*name))
        .cloned()
        .collect();

    if !missing_in_stable.is_empty() {
        let terms_body = serde_json::json!({
            "size": missing_in_stable.len(),
            "query": {
                "terms": {
                    "package_attr_name": missing_in_stable
                }
            },
            "_source": ["package_pname", "package_attr_name", "package_pversion", "package_description"]
        });
        if let Ok(json) = es_query(ES_STABLE_INDEX, &terms_body) {
            for hit in parse_hits(&json) {
                stable_map.insert(hit.attr_name.clone(), hit);
            }
        }
    }

    // Construction de la liste finale avec enrichissement des conflits et du statut d'installation
    let mut results = Vec::new();
    for name in ordered_names {
        let stable_info = stable_map.get(&name);
        let unstable_info = unstable_map.get(&name);

        let pname = stable_info
            .map(|h| h.pname.clone())
            .or_else(|| unstable_info.map(|h| h.pname.clone()))
            .unwrap_or_else(|| name.clone());

        let description = stable_info
            .map(|h| h.description.clone())
            .filter(|d| !d.is_empty())
            .or_else(|| unstable_info.map(|h| h.description.clone()))
            .unwrap_or_default();

        let stable_version = stable_info.map(|h| h.version.clone()).filter(|v| !v.is_empty());
        let unstable_version = unstable_info.map(|h| h.version.clone()).filter(|v| !v.is_empty());

        let is_custom_stable = custom.stable.contains(&name);
        let is_custom_unstable = custom.unstable.contains(&name);

        let system_conflict = conflicts.get(&name).cloned();
        let is_system_conflict = system_conflict.is_some();

        let conflict_reason = if let Some(reason) = system_conflict {
            Some(reason)
        } else if is_custom_stable && is_custom_unstable {
            Some("Doublon : présent simultanément en stable et unstable".into())
        } else {
            None
        };

        results.push(PackageEntry {
            attr_name: name,
            pname,
            description,
            stable_version,
            unstable_version,
            is_custom_stable,
            is_custom_unstable,
            is_system_conflict,
            conflict_reason,
        });
    }

    Ok(results)
}

/// Récupère l'état complet des paquets personnalisés avec détails (versions, description)
pub async fn get_packages_state() -> Result<PackagesState, String> {
    let custom = read_custom_packages()?;
    let conflicts = get_system_conflicts();

    let mut all_names = custom.stable.clone();
    all_names.extend(custom.unstable.clone());
    all_names.sort();
    all_names.dedup();

    let mut custom_details = Vec::new();

    if !all_names.is_empty() {
        let names_clone1 = all_names.clone();
        
        let terms_body = serde_json::json!({
            "size": all_names.len() + 10,
            "query": {
                "terms": {
                    "package_attr_name": all_names
                }
            },
            "_source": ["package_pname", "package_attr_name", "package_pversion", "package_description"]
        });

        let body_stable = terms_body.clone();
        let body_unstable = terms_body;

        let stable_task = tokio::task::spawn_blocking(move || {
            es_query(ES_STABLE_INDEX, &body_stable)
        });
        let unstable_task = tokio::task::spawn_blocking(move || {
            es_query(ES_UNSTABLE_INDEX, &body_unstable)
        });

        let (stable_res, unstable_res) = tokio::join!(stable_task, unstable_task);

        let mut stable_map: HashMap<String, RawPackageHit> = HashMap::new();
        if let Ok(Ok(j)) = stable_res {
            for hit in parse_hits(&j) {
                stable_map.insert(hit.attr_name.clone(), hit);
            }
        }

        let mut unstable_map: HashMap<String, RawPackageHit> = HashMap::new();
        if let Ok(Ok(j)) = unstable_res {
            for hit in parse_hits(&j) {
                unstable_map.insert(hit.attr_name.clone(), hit);
            }
        }

        for name in names_clone1 {
            let stable_info = stable_map.get(&name);
            let unstable_info = unstable_map.get(&name);

            let pname = stable_info
                .map(|h| h.pname.clone())
                .or_else(|| unstable_info.map(|h| h.pname.clone()))
                .unwrap_or_else(|| name.clone());

            let description = stable_info
                .map(|h| h.description.clone())
                .filter(|d| !d.is_empty())
                .or_else(|| unstable_info.map(|h| h.description.clone()))
                .unwrap_or_default();

            let stable_version = stable_info.map(|h| h.version.clone()).filter(|v| !v.is_empty());
            let unstable_version = unstable_info.map(|h| h.version.clone()).filter(|v| !v.is_empty());

            let is_custom_stable = custom.stable.contains(&name);
            let is_custom_unstable = custom.unstable.contains(&name);

            let conflict_reason = conflicts.get(&name).cloned();
            let is_system_conflict = conflict_reason.is_some();

            custom_details.push(PackageEntry {
                attr_name: name,
                pname,
                description,
                stable_version,
                unstable_version,
                is_custom_stable,
                is_custom_unstable,
                is_system_conflict,
                conflict_reason,
            });
        }
    }

    Ok(PackagesState {
        custom,
        custom_details,
        system_conflicts: conflicts,
    })
}

/// Ajoute un paquet personnalisé dans le fichier custom-packages.nix
pub fn add_custom_package(name: String, channel: String) -> Result<(), String> {
    let clean_name = name.trim().to_string();
    if clean_name.is_empty() {
        return Err("Nom de paquet vide.".into());
    }

    let conflicts = get_system_conflicts();
    if let Some(reason) = conflicts.get(&clean_name) {
        return Err(format!(
            "Conflit détecté : le paquet '{}' est déjà utilisé par le système ({})",
            clean_name, reason
        ));
    }

    let mut custom = read_custom_packages()?;

    match channel.as_str() {
        "stable" => {
            if !custom.stable.contains(&clean_name) {
                custom.stable.push(clean_name.clone());
            }
            // Retirer de l'autre branche pour éviter tout doublon
            custom.unstable.retain(|x| x != &clean_name);
        }
        "unstable" => {
            if !custom.unstable.contains(&clean_name) {
                custom.unstable.push(clean_name.clone());
            }
            // Retirer de l'autre branche pour éviter tout doublon
            custom.stable.retain(|x| x != &clean_name);
        }
        _ => return Err(format!("Branche inconnue: '{}'. Utilisez 'stable' ou 'unstable'.", channel)),
    }

    save_custom_packages(&custom)
}

/// Supprime un paquet personnalisé de la configuration
pub fn remove_custom_package(name: String, channel: Option<String>) -> Result<(), String> {
    let clean_name = name.trim().to_string();
    if clean_name.is_empty() {
        return Err("Nom de paquet vide.".into());
    }

    let mut custom = read_custom_packages()?;

    match channel.as_deref() {
        Some("stable") => {
            custom.stable.retain(|x| x != &clean_name);
        }
        Some("unstable") => {
            custom.unstable.retain(|x| x != &clean_name);
        }
        _ => {
            custom.stable.retain(|x| x != &clean_name);
            custom.unstable.retain(|x| x != &clean_name);
        }
    }

    save_custom_packages(&custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_package_list() {
        let sample = r#"{
  stable = [
    "htop" # monitor
    "neovim"
  ];
  unstable = [
    "spotify"
  ];
}
"#;
        assert_eq!(extract_package_list(sample, "stable"), vec!["htop", "neovim"]);
        assert_eq!(extract_package_list(sample, "unstable"), vec!["spotify"]);
    }

    #[tokio::test]
    #[ignore = "requires network access (disabled in nix sandbox)"]
    async fn test_search_nixpkgs_live() {
        let results = search_nixpkgs("htop".to_string()).await.expect("search failed");
        assert!(!results.is_empty());
        let first = &results[0];
        assert_eq!(first.attr_name, "htop");
        assert!(first.stable_version.is_some());
        assert!(first.unstable_version.is_some());
    }
}
