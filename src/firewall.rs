use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

const FIREWALL_FILE: &str = "/etc/nixos/modules/core/firewall.nix";
const FIREWALL_BACKUP: &str = "/etc/nixos/modules/core/.firewall.nix.backup";
const VARS_FILE: &str = "/etc/nixos/vars.nix";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FirewallPortRule {
    pub id: String,
    pub rule_type: String, // "single" | "range"
    pub protocol: String,  // "tcp" | "udp" | "both"
    pub port: Option<u16>,
    pub from_port: Option<u16>,
    pub to_port: Option<u16>,
    pub label: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallState {
    pub enabled: bool,
    pub rules: Vec<FirewallPortRule>,
}

/// Charge l'état complet du pare-feu depuis vars.nix et modules/core/firewall.nix
pub fn load_firewall_state() -> Result<FirewallState, String> {
    let enabled = read_firewall_enabled_from_vars();

    let content = if Path::new(FIREWALL_FILE).exists() {
        fs::read_to_string(FIREWALL_FILE)
            .map_err(|e| format!("Impossible de lire {}: {}", FIREWALL_FILE, e))?
    } else if Path::new(FIREWALL_BACKUP).exists() {
        fs::read_to_string(FIREWALL_BACKUP)
            .map_err(|e| format!("Impossible de lire {}: {}", FIREWALL_BACKUP, e))?
    } else {
        default_firewall_nix_content()
    };

    let rules = parse_firewall_rules(&content);

    Ok(FirewallState { enabled, rules })
}

/// Sauvegarde l'état complet du pare-feu dans vars.nix et modules/core/firewall.nix
pub fn save_firewall_state(enabled: bool, rules: Vec<FirewallPortRule>) -> Result<(), String> {
    // 1. Mettre à jour firewall = true/false dans vars.nix
    update_firewall_enabled_in_vars(enabled)?;

    // 2. Générer le contenu Nix de firewall.nix
    let nix_content = generate_firewall_nix_content(&rules);

    // 3. Écriture atomique dans modules/core/firewall.nix
    let path = Path::new(FIREWALL_FILE);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let tmp_path = path.with_extension("nix.tmp");
    fs::write(&tmp_path, &nix_content)
        .map_err(|e| format!("Impossible d'écrire temporairement dans {}: {}", tmp_path.display(), e))?;

    fs::rename(&tmp_path, path)
        .map_err(|e| format!("Impossible de remplacer {}: {}", path.display(), e))?;

    // 4. Mettre à jour la sauvegarde inviolable locale
    let _ = fs::write(FIREWALL_BACKUP, &nix_content);

    Ok(())
}

fn read_firewall_enabled_from_vars() -> bool {
    let path = Path::new(VARS_FILE);
    if !path.exists() {
        return false;
    }
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Cherche firewall = true; ou firewall = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("firewall") {
            if let Some((_, val_part)) = trimmed.split_once('=') {
                let val = val_part.trim().trim_end_matches(';').trim();
                if val == "true" {
                    return true;
                } else if val == "false" {
                    return false;
                }
            }
        }
    }
    false
}

fn update_firewall_enabled_in_vars(enabled: bool) -> Result<(), String> {
    let path = Path::new(VARS_FILE);
    if !path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Impossible de lire {}: {}", VARS_FILE, e))?;

    let mut found = false;
    let mut new_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("firewall") && trimmed.contains('=') {
            new_lines.push(format!("  firewall = {};", enabled));
            found = true;
        } else {
            new_lines.push(line.to_string());
        }
    }

    if !found {
        // Insérer avant la dernière accolade fermante
        if let Some(pos) = new_lines.iter().rposition(|l| l.trim() == "}") {
            new_lines.insert(pos, format!("  # Pare-feu réseau\n  firewall = {};\n", enabled));
        } else {
            new_lines.push(format!("  firewall = {};", enabled));
        }
    }

    let updated_content = new_lines.join("\n") + "\n";
    let tmp_path = path.with_extension("nix.tmp");
    fs::write(&tmp_path, &updated_content)
        .map_err(|e| format!("Impossible d'écrire temporairement dans {}: {}", tmp_path.display(), e))?;

    fs::rename(&tmp_path, path)
        .map_err(|e| format!("Impossible de remplacer {}: {}", path.display(), e))?;

    // Mettre à jour la sauvegarde de vars.nix si présente
    let _ = fs::write("/etc/nixos/.vars.nix.backup", &updated_content);

    Ok(())
}

fn parse_firewall_rules(content: &str) -> Vec<FirewallPortRule> {
    let tcp_ports = parse_single_ports_list(content, "allowedTCPPorts");
    let udp_ports = parse_single_ports_list(content, "allowedUDPPorts");
    let tcp_ranges = parse_port_ranges_list(content, "allowedTCPPortRanges");
    let udp_ranges = parse_port_ranges_list(content, "allowedUDPPortRanges");

    let mut rules = Vec::new();
    let mut processed_udp_single: HashSet<u16> = HashSet::new();

    // Fusionner les ports simples
    let udp_map: HashMap<u16, String> = udp_ports.into_iter().collect();
    for (port, tcp_label) in tcp_ports {
        if let Some(udp_label) = udp_map.get(&port) {
            processed_udp_single.insert(port);
            let label = if !tcp_label.is_empty() {
                tcp_label
            } else {
                udp_label.clone()
            };
            rules.push(FirewallPortRule {
                id: format!("single-both-{}", port),
                rule_type: "single".to_string(),
                protocol: "both".to_string(),
                port: Some(port),
                from_port: None,
                to_port: None,
                label,
                is_default: is_default_port(port, "both"),
            });
        } else {
            rules.push(FirewallPortRule {
                id: format!("single-tcp-{}", port),
                rule_type: "single".to_string(),
                protocol: "tcp".to_string(),
                port: Some(port),
                from_port: None,
                to_port: None,
                label: tcp_label,
                is_default: is_default_port(port, "tcp"),
            });
        }
    }

    for (port, udp_label) in udp_map {
        if !processed_udp_single.contains(&port) {
            rules.push(FirewallPortRule {
                id: format!("single-udp-{}", port),
                rule_type: "single".to_string(),
                protocol: "udp".to_string(),
                port: Some(port),
                from_port: None,
                to_port: None,
                label: udp_label,
                is_default: is_default_port(port, "udp"),
            });
        }
    }

    // Fusionner les plages de ports
    let mut processed_udp_ranges: HashSet<(u16, u16)> = HashSet::new();
    let udp_range_map: HashMap<(u16, u16), String> = udp_ranges
        .into_iter()
        .map(|(from, to, lbl)| ((from, to), lbl))
        .collect();

    for (from, to, tcp_label) in tcp_ranges {
        if let Some(udp_label) = udp_range_map.get(&(from, to)) {
            processed_udp_ranges.insert((from, to));
            let label = if !tcp_label.is_empty() {
                tcp_label
            } else {
                udp_label.clone()
            };
            rules.push(FirewallPortRule {
                id: format!("range-both-{}-{}", from, to),
                rule_type: "range".to_string(),
                protocol: "both".to_string(),
                port: None,
                from_port: Some(from),
                to_port: Some(to),
                label,
                is_default: is_default_range(from, to, "both"),
            });
        } else {
            rules.push(FirewallPortRule {
                id: format!("range-tcp-{}-{}", from, to),
                rule_type: "range".to_string(),
                protocol: "tcp".to_string(),
                port: None,
                from_port: Some(from),
                to_port: Some(to),
                label: tcp_label,
                is_default: is_default_range(from, to, "tcp"),
            });
        }
    }

    for ((from, to), udp_label) in udp_range_map {
        if !processed_udp_ranges.contains(&(from, to)) {
            rules.push(FirewallPortRule {
                id: format!("range-udp-{}-{}", from, to),
                rule_type: "range".to_string(),
                protocol: "udp".to_string(),
                port: None,
                from_port: Some(from),
                to_port: Some(to),
                label: udp_label,
                is_default: is_default_range(from, to, "udp"),
            });
        }
    }

    rules
}

fn parse_single_ports_list(content: &str, list_name: &str) -> Vec<(u16, String)> {
    let mut results = Vec::new();
    let block = match extract_list_block(content, list_name) {
        Some(b) => b,
        None => return results,
    };

    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (num_part, comment_part) = match trimmed.split_once('#') {
            Some((n, c)) => (n.trim(), c.trim().to_string()),
            None => (trimmed.trim_end_matches(';').trim(), String::new()),
        };

        if let Ok(port) = num_part.parse::<u16>() {
            results.push((port, comment_part));
        }
    }

    results
}

fn parse_port_ranges_list(content: &str, list_name: &str) -> Vec<(u16, u16, String)> {
    let mut results = Vec::new();
    let block = match extract_list_block(content, list_name) {
        Some(b) => b,
        None => return results,
    };

    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (brace_part, comment_part) = match trimmed.split_once('#') {
            Some((b, c)) => (b.trim(), c.trim().to_string()),
            None => (trimmed, String::new()),
        };

        let brace_inner = brace_part
            .trim_start_matches('{')
            .trim_end_matches('}')
            .trim();

        let mut from_opt: Option<u16> = None;
        let mut to_opt: Option<u16> = None;

        for part in brace_inner.split(';') {
            let part_trim = part.trim();
            if let Some((k, v)) = part_trim.split_once('=') {
                let key = k.trim();
                let val = v.trim().parse::<u16>().ok();
                if key == "from" {
                    from_opt = val;
                } else if key == "to" {
                    to_opt = val;
                }
            }
        }

        if let (Some(from), Some(to)) = (from_opt, to_opt) {
            results.push((from, to, comment_part));
        }
    }

    results
}

fn extract_list_block<'a>(content: &'a str, list_name: &str) -> Option<&'a str> {
    let pattern = format!("{list_name} = [");
    let start_pos = content.find(&pattern)?;
    let after_bracket = &content[start_pos + pattern.len()..];
    let end_bracket = after_bracket.find("];")?;
    Some(&after_bracket[..end_bracket])
}

fn is_default_port(port: u16, _proto: &str) -> bool {
    port == 53317 // LocalSend
}

fn is_default_range(from: u16, to: u16, _proto: &str) -> bool {
    (from == 27015 && to == 27030) || (from == 3000 && to == 3010) || (from == 27000 && to == 27100)
}

fn generate_firewall_nix_content(rules: &[FirewallPortRule]) -> String {
    let mut tcp_singles = Vec::new();
    let mut udp_singles = Vec::new();
    let mut tcp_ranges = Vec::new();
    let mut udp_ranges = Vec::new();

    for r in rules {
        if r.rule_type == "single" {
            if let Some(port) = r.port {
                let comment = if r.label.trim().is_empty() {
                    String::new()
                } else {
                    format!(" # {}", r.label.trim())
                };
                let entry = format!("      {port}{comment}");

                if r.protocol == "tcp" || r.protocol == "both" {
                    tcp_singles.push((port, entry.clone()));
                }
                if r.protocol == "udp" || r.protocol == "both" {
                    udp_singles.push((port, entry));
                }
            }
        } else if r.rule_type == "range" {
            if let (Some(from), Some(to)) = (r.from_port, r.to_port) {
                let comment = if r.label.trim().is_empty() {
                    String::new()
                } else {
                    format!(" # {}", r.label.trim())
                };
                let entry = format!("      {{ from = {from}; to = {to}; }}{comment}");

                if r.protocol == "tcp" || r.protocol == "both" {
                    tcp_ranges.push(((from, to), entry.clone()));
                }
                if r.protocol == "udp" || r.protocol == "both" {
                    udp_ranges.push(((from, to), entry));
                }
            }
        }
    }

    tcp_singles.sort_by_key(|k| k.0);
    tcp_singles.dedup_by_key(|k| k.0);

    udp_singles.sort_by_key(|k| k.0);
    udp_singles.dedup_by_key(|k| k.0);

    tcp_ranges.sort_by_key(|k| k.0);
    tcp_ranges.dedup_by_key(|k| k.0);

    udp_ranges.sort_by_key(|k| k.0);
    udp_ranges.dedup_by_key(|k| k.0);

    let tcp_singles_str = if tcp_singles.is_empty() {
        String::new()
    } else {
        tcp_singles.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n") + "\n"
    };

    let udp_singles_str = if udp_singles.is_empty() {
        String::new()
    } else {
        udp_singles.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n") + "\n"
    };

    let tcp_ranges_str = if tcp_ranges.is_empty() {
        String::new()
    } else {
        tcp_ranges.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n") + "\n"
    };

    let udp_ranges_str = if udp_ranges.is_empty() {
        String::new()
    } else {
        udp_ranges.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("\n") + "\n"
    };

    format!(
r#"{{ config, pkgs, ... }}:

{{
  # =========================================================================
  # 🛡️ GESTION DU PARE-FEU & RÈGLES DE SÉCURITÉ RÉSEAU
  # Modifié via le Dashboard ChomiamOS
  # =========================================================================

  networking.firewall = {{
    enable = config.chomiamos.firewall.enable;

    # Ports TCP autorisés
    allowedTCPPorts = [
{tcp_singles_str}    ];

    # Ports UDP autorisés
    allowedUDPPorts = [
{udp_singles_str}    ];

    # Plages de ports TCP autorisées
    allowedTCPPortRanges = [
{tcp_ranges_str}    ];

    # Plages de ports UDP autorisées
    allowedUDPPortRanges = [
{udp_ranges_str}    ];
  }};
}}
"#
    )
}

fn default_firewall_nix_content() -> String {
    r#"{ config, pkgs, ... }:

{
  # =========================================================================
  # 🛡️ GESTION DU PARE-FEU & RÈGLES DE SÉCURITÉ RÉSEAU
  # =========================================================================

  networking.firewall = {
    enable = config.chomiamos.firewall.enable;

    # Ports TCP autorisés
    allowedTCPPorts = [
      53317 # LocalSend (Partage de fichiers local)
    ];

    # Ports UDP autorisés
    allowedUDPPorts = [
      53317 # LocalSend (Découverte d'appareils réseau local)
    ];

    # Plages de ports TCP autorisées
    allowedTCPPortRanges = [
      { from = 27015; to = 27030; } # Jeux Paradox (Stellaris) & Steam session
    ];

    # Plages de ports UDP autorisées
    allowedUDPPortRanges = [
      { from = 3000; to = 3010; }   # Moteur Clausewitz (Multi direct Paradox)
      { from = 27000; to = 27100; } # Jeux Paradox (Stellaris / Matchmaking P2P)
    ];
  };
}
"#.to_string()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_firewall() {
        let content = default_firewall_nix_content();
        let rules = parse_firewall_rules(&content);
        assert!(!rules.is_empty());
        let localsend = rules.iter().find(|r| r.port == Some(53317));
        assert!(localsend.is_some());
        assert_eq!(localsend.unwrap().protocol, "both");
        let stellaris = rules.iter().find(|r| r.from_port == Some(27015));
        assert!(stellaris.is_some());
    }
}
