use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

const DNS_USER_FILE: &str = "/etc/nixos/dns-user.nix";
const DNS_USER_BACKUP: &str = "/etc/nixos/.dns-user.nix.backup";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsProvider {
    pub id: String,
    pub name: String,
    pub category: String, // fast, security, family, privacy, neutral, custom, default
    pub description: String,
    pub primary_ip: String,
    pub secondary_ip: Option<String>,
    pub tags: Vec<String>,
    pub icon: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsCatalog {
    pub current_dns: String,
    pub active_provider_id: String,
    pub is_custom: bool,
    pub custom_primary: String,
    pub custom_secondary: String,
    pub providers: Vec<DnsProvider>,
}



pub fn get_default_providers() -> Vec<DnsProvider> {
    vec![
        DnsProvider {
            id: "cloudflare".to_string(),
            name: "Cloudflare Standard".to_string(),
            category: "fast".to_string(),
            description: "Résolveur ultra-rapide (1.1.1.1) axé sur la performance, avec chiffrement DNS-over-HTTPS/TLS et politique zéro-log.".to_string(),
            primary_ip: "1.1.1.1".to_string(),
            secondary_ip: Some("1.0.0.1".to_string()),
            tags: vec!["Ultra-rapide".to_string(), "Chiffré".to_string(), "Populaire".to_string()],
            icon: "⚡".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "cloudflare-security".to_string(),
            name: "Cloudflare Sécurité".to_string(),
            category: "security".to_string(),
            description: "Filtre automatiquement les logiciels malveillants, botnets et sites de hameçonnage (phishing) connus.".to_string(),
            primary_ip: "1.1.1.2".to_string(),
            secondary_ip: Some("1.0.0.2".to_string()),
            tags: vec!["Sécurité".to_string(), "Anti-malware".to_string(), "Anti-phishing".to_string()],
            icon: "🛡️".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "cloudflare-family".to_string(),
            name: "Cloudflare Famille".to_string(),
            category: "family".to_string(),
            description: "Protection renforcée pour toute la famille : blocage automatique des malwares et filtrage des contenus adultes.".to_string(),
            primary_ip: "1.1.1.3".to_string(),
            secondary_ip: Some("1.0.0.3".to_string()),
            tags: vec!["Famille".to_string(), "Contrôle parental".to_string(), "Sécurisé".to_string()],
            icon: "👨‍👩‍👧".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "quad9".to_string(),
            name: "Quad9 Sécurisé (Suisse)".to_string(),
            category: "security".to_string(),
            description: "Organisation suisse à but non lucratif. Bloque les cybermenaces avec renseignements qualifiés et respect strict de la vie privée.".to_string(),
            primary_ip: "9.9.9.9".to_string(),
            secondary_ip: Some("149.112.112.112".to_string()),
            tags: vec!["Vie privée".to_string(), "Sécurité".to_string(), "Suisse".to_string(), "RGPD".to_string()],
            icon: "🇨🇭".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "quad9-unfiltered".to_string(),
            name: "Quad9 Neutre (Non-filtré)".to_string(),
            category: "neutral".to_string(),
            description: "Version Quad9 sans blocage de sécurité. Idéale si vous souhaitez une résolution totalement brute sans faux positifs.".to_string(),
            primary_ip: "9.9.9.10".to_string(),
            secondary_ip: Some("149.112.112.10".to_string()),
            tags: vec!["Neutre".to_string(), "Sans censure".to_string()],
            icon: "🌐".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "adguard".to_string(),
            name: "AdGuard DNS Bloqueur Pubs".to_string(),
            category: "security".to_string(),
            description: "Bloque les bannières publicitaires, vidéos sponsorisées, traceurs web et domaines de télémétrie au niveau réseau.".to_string(),
            primary_ip: "94.140.14.14".to_string(),
            secondary_ip: Some("94.140.15.15".to_string()),
            tags: vec!["Anti-pub".to_string(), "Anti-trackers".to_string(), "Vie privée".to_string()],
            icon: "🚫".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "adguard-family".to_string(),
            name: "AdGuard Famille & Pubs".to_string(),
            category: "family".to_string(),
            description: "Combine le blocage de publicités AdGuard avec un contrôle parental strict et le mode Recherche Sécurisée (SafeSearch).".to_string(),
            primary_ip: "94.140.14.15".to_string(),
            secondary_ip: Some("94.140.15.16".to_string()),
            tags: vec!["Anti-pub".to_string(), "Contrôle parental".to_string()],
            icon: "🏡".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "adguard-unfiltered".to_string(),
            name: "AdGuard Non-filtré".to_string(),
            category: "neutral".to_string(),
            description: "Serveurs AdGuard haute vitesse sans filtrage publicitaire ni blocage de domaine.".to_string(),
            primary_ip: "94.140.14.140".to_string(),
            secondary_ip: Some("94.140.14.141".to_string()),
            tags: vec!["Neutre".to_string(), "Rapide".to_string()],
            icon: "⚡".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "google".to_string(),
            name: "Google Public DNS".to_string(),
            category: "fast".to_string(),
            description: "L'un des plus anciens résolveurs publics au monde. Infrastructure mondiale distribuée pour une disponibilité maximale.".to_string(),
            primary_ip: "8.8.8.8".to_string(),
            secondary_ip: Some("8.8.4.4".to_string()),
            tags: vec!["Mondial".to_string(), "Fiabilité".to_string(), "Anycast".to_string()],
            icon: "🌍".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "mullvad".to_string(),
            name: "Mullvad DNS Standard (Suède)".to_string(),
            category: "privacy".to_string(),
            description: "Opéré par les créateurs de Mullvad VPN en Suède. Zéro logs, aucune conservation d'IP et respect total de la confidentialité.".to_string(),
            primary_ip: "194.242.2.2".to_string(),
            secondary_ip: Some("194.242.2.3".to_string()),
            tags: vec!["No-log".to_string(), "Suède".to_string(), "Confidentialité".to_string()],
            icon: "🔒".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "mullvad-adblock".to_string(),
            name: "Mullvad Anti-pub (Suède)".to_string(),
            category: "privacy".to_string(),
            description: "Résolveur suédois sans logs enrichi d'un filtre DNS bloquant les serveurs publicitaires et traqueurs.".to_string(),
            primary_ip: "194.242.2.4".to_string(),
            secondary_ip: None,
            tags: vec!["Anti-pub".to_string(), "No-log".to_string(), "Suède".to_string()],
            icon: "🛡️".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "fdn".to_string(),
            name: "FDN (French Data Network)".to_string(),
            category: "neutral".to_string(),
            description: "Fournisseur d'accès internet associatif militant en France depuis 1992. DNS neutre, souverain, sans log et sans censure.".to_string(),
            primary_ip: "80.67.169.12".to_string(),
            secondary_ip: Some("80.67.169.40".to_string()),
            tags: vec!["Associatif".to_string(), "France".to_string(), "Neutre".to_string(), "Libre".to_string()],
            icon: "🇫🇷".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "dnssb".to_string(),
            name: "DNS.SB (Anycast No-Log)".to_string(),
            category: "privacy".to_string(),
            description: "Réseau DNS indépendant présent mondialement avec support DNSSEC natif et engagement strict de non-conservation des journaux.".to_string(),
            primary_ip: "185.222.222.222".to_string(),
            secondary_ip: Some("45.11.45.11".to_string()),
            tags: vec!["DNSSEC".to_string(), "No-log".to_string(), "Rapide".to_string()],
            icon: "🚀".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "default".to_string(),
            name: "Box Internet / Passerelle (DHCP)".to_string(),
            category: "default".to_string(),
            description: "Utilise les adresses DNS attribuées automatiquement par votre box opérateur (Livebox, Freebox, Bbox, etc.) via DHCP.".to_string(),
            primary_ip: "Automatique".to_string(),
            secondary_ip: None,
            tags: vec!["Box Internet".to_string(), "Par défaut".to_string(), "Automatique".to_string()],
            icon: "🏠".to_string(),
            is_active: false,
        },
        DnsProvider {
            id: "custom".to_string(),
            name: "DNS Personnalisé".to_string(),
            category: "custom".to_string(),
            description: "Saisissez manuellement vos propres adresses IPv4 de résolveurs DNS préférés.".to_string(),
            primary_ip: "".to_string(),
            secondary_ip: None,
            tags: vec!["Personnalisé".to_string(), "Manuel".to_string()],
            icon: "⚙️".to_string(),
            is_active: false,
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodmanPortInfo {
    pub host_ip: String,
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
    pub is_http: bool,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodmanContainerInfo {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub unit_name: String,
    pub is_active: bool,
    pub status_text: String,
    pub uptime: String,
    pub memory_bytes: u64,
    pub memory_human: String,
    pub cpu_usage_sec: f64,
    pub image: String,
    pub ports: Vec<PodmanPortInfo>,
    pub env_summary: HashMap<String, String>,
    pub volumes: Vec<String>,
    pub network_mode: String,
    pub has_logs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodmanOverview {
    pub engine_version: String,
    pub docker_compat: bool,
    pub dns_enabled: bool,
    pub total_containers: usize,
    pub running_containers: usize,
    pub total_memory_human: String,
    pub containers: Vec<PodmanContainerInfo>,
}

#[tauri::command]
pub async fn get_dns_catalog() -> Result<DnsCatalog, String> {
    // 1. Lire les serveurs actifs actuels via resolvectl
    let mut current_dns = String::new();
    if let Ok(out) = Command::new("resolvectl").arg("status").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with("Current DNS Server:") || l.starts_with("DNS Servers:") {
                let parts: Vec<&str> = l.split(": ").collect();
                if parts.len() > 1 {
                    current_dns = parts[1].trim().to_string();
                    break;
                }
            }
        }
    }
    if current_dns.is_empty() {
        current_dns = "DHCP / Automatique".to_string();
    }

    // 2. Vérifier dns-user.nix pour identifier la sélection permanente
    let mut configured_ips = Vec::new();
    let mut custom_primary = String::new();
    let mut custom_secondary = String::new();

    if Path::new(DNS_USER_FILE).exists() {
        if let Ok(content) = fs::read_to_string(DNS_USER_FILE) {
            configured_ips = parse_nameservers_from_nix(&content);
        }
    }

    let mut providers = get_default_providers();
    let mut active_provider_id = "default".to_string();
    let mut is_custom = false;

    if configured_ips.is_empty() {
        active_provider_id = "default".to_string();
    } else {
        // Trouver un fournisseur dont la primary_ip correspond
        let first_ip = &configured_ips[0];
        let mut matched = false;
        for p in &providers {
            if p.id != "default" && p.id != "custom" && &p.primary_ip == first_ip {
                active_provider_id = p.id.clone();
                matched = true;
                break;
            }
        }
        if !matched {
            active_provider_id = "custom".to_string();
            is_custom = true;
            custom_primary = first_ip.clone();
            if configured_ips.len() > 1 {
                custom_secondary = configured_ips[1].clone();
            }
        }
    }

    // Mettre à jour is_active
    for p in &mut providers {
        if p.id == active_provider_id {
            p.is_active = true;
        }
    }

    Ok(DnsCatalog {
        current_dns,
        active_provider_id,
        is_custom,
        custom_primary,
        custom_secondary,
        providers,
    })
}

fn parse_nameservers_from_nix(content: &str) -> Vec<String> {
    let mut ips = Vec::new();
    let re = regex::Regex::new(r#""([0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3})""#).unwrap();
    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            ips.push(m.as_str().to_string());
        }
    }
    ips
}

#[tauri::command]
pub async fn ping_dns_servers(ips: Vec<String>) -> Result<HashMap<String, Option<f64>>, String> {
    let mut tasks = Vec::new();

    for ip in ips {
        tasks.push(tokio::spawn(async move {
            let latency = measure_single_dns_ping(&ip).await;
            (ip, latency)
        }));
    }

    let mut results = HashMap::new();
    for t in tasks {
        if let Ok((ip, latency)) = t.await {
            results.insert(ip, latency);
        }
    }

    Ok(results)
}

async fn measure_single_dns_ping(ip: &str) -> Option<f64> {
    if ip.is_empty() || ip == "Automatique" {
        return None;
    }

    let target = format!("{}:53", ip);
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(_) => return None,
    };

    // Paquet de requête DNS A minimal pour google.com
    let query: [u8; 28] = [
        0x12, 0x34, // ID
        0x01, 0x00, // Flags standard query recursion desired
        0x00, 0x01, // 1 question
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // RRs
        0x06, b'g', b'o', b'o', b'g', b'l', b'e',
        0x03, b'c', b'o', b'm',
        0x00,       // End
        0x00, 0x01, // Type A
        0x00, 0x01, // Class IN
    ];

    let t0 = Instant::now();
    let send_res = tokio::time::timeout(Duration::from_millis(1500), socket.send_to(&query, &target)).await;
    if send_res.is_err() || send_res.unwrap().is_err() {
        return None;
    }

    let mut buf = [0u8; 512];
    let recv_res = tokio::time::timeout(Duration::from_millis(1500), socket.recv_from(&mut buf)).await;
    match recv_res {
        Ok(Ok((bytes_read, _))) if bytes_read > 0 => {
            let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
            Some((elapsed * 10.0).round() / 10.0)
        }
        _ => {
            // Fallback: tentative TCP connect sur port 53
            let t0_tcp = Instant::now();
            let tcp_res = tokio::time::timeout(Duration::from_millis(1200), tokio::net::TcpStream::connect(&target)).await;
            if let Ok(Ok(_)) = tcp_res {
                let elapsed = t0_tcp.elapsed().as_secs_f64() * 1000.0;
                Some((elapsed * 10.0).round() / 10.0)
            } else {
                None
            }
        }
    }
}

#[tauri::command]
pub async fn apply_dns_server(
    provider_id: String,
    custom_ips: Vec<String>,
    apply_runtime: bool,
    save_permanent: bool,
) -> Result<String, String> {
    let mut target_ips: Vec<String> = Vec::new();

    if provider_id == "default" {
        target_ips = Vec::new();
    } else if provider_id == "custom" {
        target_ips = custom_ips.into_iter().filter(|ip| !ip.trim().is_empty()).collect();
    } else {
        let providers = get_default_providers();
        if let Some(p) = providers.into_iter().find(|p| p.id == provider_id) {
            if !p.primary_ip.is_empty() && p.primary_ip != "Automatique" {
                target_ips.push(p.primary_ip);
                if let Some(sec) = p.secondary_ip {
                    target_ips.push(sec);
                }
            }
        }
    }

    // 1. Application immédiate à chaud via resolvectl & NetworkManager
    if apply_runtime {
        // Trouver l'interface par défaut (ex: enp7s0, wlp8s0)
        let mut default_iface = String::new();
        if let Ok(out) = Command::new("ip").args(["route", "show", "default"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = text.split_whitespace().collect();
            if let Some(pos) = parts.iter().position(|&r| r == "dev") {
                if pos + 1 < parts.len() {
                    default_iface = parts[pos + 1].to_string();
                }
            }
        }

        if default_iface.is_empty() {
            default_iface = "enp7s0".to_string();
        }

        if target_ips.is_empty() {
            let _ = Command::new("resolvectl").args(["revert", &default_iface]).output();
            let _ = Command::new("resolvectl").arg("flush-caches").output();
            let _ = Command::new("nmcli").args(["dev", "modify", &default_iface, "ipv4.ignore-auto-dns", "no"]).output();
        } else {
            let mut args = vec!["dns".to_string(), default_iface.clone()];
            args.extend(target_ips.clone());
            let _ = Command::new("resolvectl").args(&args).output();
            let _ = Command::new("resolvectl").args(["domain", &default_iface, "~."]).output();
            let _ = Command::new("resolvectl").arg("flush-caches").output();
            let ips_joined = target_ips.join(" ");
            let _ = Command::new("nmcli").args(["dev", "modify", &default_iface, "ipv4.ignore-auto-dns", "yes", "ipv4.dns", &ips_joined]).output();
        }
    }

    // 2. Sauvegarde permanente dans /etc/nixos/dns-user.nix
    if save_permanent {
        let mut nix_servers = String::new();
        for ip in &target_ips {
            nix_servers.push_str(&format!("    \"{}\"\n", ip));
        }

        let content = format!(
            r#"# =========================================================================
# 🌐 CONFIGURATION DNS SYSTÈME (CHOMIAMOS)
# Modifié automatiquement par le Dashboard ChomiamOS
# Fournisseur : {provider_id}
# =========================================================================
{{ config, pkgs, lib, ... }}:

{{
  networking.nameservers = [
{nix_servers}  ];
}}
"#
        );

        fs::write(DNS_USER_FILE, &content)
            .map_err(|e| format!("Impossible d'écrire {}: {}", DNS_USER_FILE, e))?;

        let _ = fs::write(DNS_USER_BACKUP, &content);
    }

    let msg = if target_ips.is_empty() {
        "Serveurs DNS remis par défaut (DHCP / Box internet) avec succès !".to_string()
    } else {
        format!("Serveurs DNS ({}) appliqués avec succès !", target_ips.join(", "))
    };

    Ok(msg)
}

#[tauri::command]
pub async fn get_podman_overview() -> Result<PodmanOverview, String> {
    // 1. Informations du moteur Podman
    let mut engine_version = "Podman (Système)".to_string();
    if let Ok(out) = Command::new("podman").arg("--version").output() {
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !v.is_empty() {
            engine_version = v;
        }
    }

    let docker_compat = Path::new("/run/podman/podman.sock").exists() || Path::new("/var/run/docker.sock").exists();
    let dns_enabled = true;

    // 2. Scanner les unités systemd podman-*.service
    let mut containers = Vec::new();
    let mut total_mem_bytes: u64 = 0;

    let units_out = Command::new("systemctl")
        .args(["list-units", "podman-*.service", "--all", "--plain", "--no-legend"])
        .output();

    if let Ok(out) = units_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            let unit_name = parts[0].to_string();
            if !unit_name.starts_with("podman-") || !unit_name.ends_with(".service") {
                continue;
            }

            if let Some(info) = inspect_podman_unit(&unit_name) {
                total_mem_bytes += info.memory_bytes;
                containers.push(info);
            }
        }
    }

    // Trier les conteneurs : les conteneurs actifs en premier
    containers.sort_by(|a, b| b.is_active.cmp(&a.is_active).then_with(|| a.name.cmp(&b.name)));

    let running_count = containers.iter().filter(|c| c.is_active).count();
    let total_count = containers.len();
    let total_memory_human = format_bytes_human(total_mem_bytes);

    Ok(PodmanOverview {
        engine_version,
        docker_compat,
        dns_enabled,
        total_containers: total_count,
        running_containers: running_count,
        total_memory_human,
        containers,
    })
}

fn inspect_podman_unit(unit_name: &str) -> Option<PodmanContainerInfo> {
    let show_out = Command::new("systemctl")
        .args(["show", unit_name, "--property=Id,ActiveState,SubState,ActiveEnterTimestamp,MemoryCurrent,CPUUsageNSec,ExecStart"])
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&show_out.stdout);
    let mut props = HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once("=") {
            props.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    let active_state = props.get("ActiveState").cloned().unwrap_or_else(|| "inactive".to_string());
    let _sub_state = props.get("SubState").cloned().unwrap_or_default();
    let is_active = active_state == "active";
    let status_text = if is_active {
        "En cours d'exécution".to_string()
    } else {
        "Arrêté".to_string()
    };

    let uptime = props.get("ActiveEnterTimestamp").cloned().unwrap_or_else(|| "-".to_string());
    let memory_bytes = props.get("MemoryCurrent").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let memory_human = format_bytes_human(memory_bytes);
    let cpu_ns = props.get("CPUUsageNSec").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let cpu_usage_sec = (cpu_ns as f64 / 1_000_000_000.0 * 100.0).round() / 100.0;

    // Nom brut (ex: "open-webui", "hermes-agent")
    let raw_name = unit_name
        .trim_start_matches("podman-")
        .trim_end_matches(".service")
        .to_string();

    let display_name = match raw_name.as_str() {
        "open-webui" => "Open WebUI (Interface IA locale)".to_string(),
        "hermes-agent" => "Agent IA Hermes (Gateway & Dashboard)".to_string(),
        _ => raw_name.clone(),
    };

    // Analyser le script de démarrage pour extraire ports, image, env et volumes
    let mut image = "Conteneur OCI".to_string();
    let mut ports = Vec::new();
    let mut env_summary = HashMap::new();
    let mut volumes = Vec::new();
    let mut network_mode = "Bridge".to_string();

    if let Some(exec_start) = props.get("ExecStart") {
        if let Some(path) = extract_exec_start_path(exec_start) {
            if let Ok(script_content) = fs::read_to_string(&path) {
                parse_script_details(
                    &raw_name,
                    &script_content,
                    &mut image,
                    &mut ports,
                    &mut env_summary,
                    &mut volumes,
                    &mut network_mode,
                );
            }
        }
    }

    // Compléter les ports connus par défaut si non détectés
    if ports.is_empty() {
        if raw_name == "open-webui" {
            ports.push(PodmanPortInfo {
                host_ip: "127.0.0.1".to_string(),
                host_port: 8080,
                container_port: 8080,
                protocol: "TCP".to_string(),
                is_http: true,
                url: Some("http://localhost:8080".to_string()),
            });
        } else if raw_name == "hermes-agent" {
            ports.push(PodmanPortInfo {
                host_ip: "0.0.0.0".to_string(),
                host_port: 9119,
                container_port: 9119,
                protocol: "TCP".to_string(),
                is_http: true,
                url: Some("http://localhost:9119".to_string()),
            });
            ports.push(PodmanPortInfo {
                host_ip: "127.0.0.1".to_string(),
                host_port: 8642,
                container_port: 8642,
                protocol: "TCP".to_string(),
                is_http: true,
                url: Some("http://localhost:8642/health".to_string()),
            });
        }
    }

    Some(PodmanContainerInfo {
        id: raw_name.clone(),
        name: raw_name,
        display_name,
        unit_name: unit_name.to_string(),
        is_active,
        status_text,
        uptime,
        memory_bytes,
        memory_human,
        cpu_usage_sec,
        image,
        ports,
        env_summary,
        volumes,
        network_mode,
        has_logs: true,
    })
}

fn extract_exec_start_path(exec_start: &str) -> Option<String> {
    if let Some(pos) = exec_start.find("path=") {
        let after = &exec_start[pos + 5..];
        if let Some(end) = after.find(' ') {
            return Some(after[..end].trim().to_string());
        }
    }
    None
}

fn parse_script_details(
    name: &str,
    script: &str,
    image: &mut String,
    ports: &mut Vec<PodmanPortInfo>,
    env: &mut HashMap<String, String>,
    volumes: &mut Vec<String>,
    network_mode: &mut String,
) {
    for line in script.lines() {
        let l = line.trim();

        // Réseau
        if l.contains("--network=host") || l.contains("'--network=host'") {
            *network_mode = "Host (Réseau hôte partagé)".to_string();
        } else if l.contains("--network=") {
            *network_mode = "Bridge".to_string();
        }

        // Image
        if (l.contains("ghcr.io/") || l.contains("docker.io/") || l.contains("nousresearch/") || l.contains(":latest") || l.contains(":main"))
            && !l.starts_with("#") && !l.starts_with("exec") && !l.contains("podman")
        {
            let cleaned = l.replace('\\', "").trim().to_string();
            if !cleaned.is_empty() && !cleaned.starts_with('-') {
                *image = cleaned;
            }
        }

        // Volumes
        if l.starts_with("-v ") || l.contains(" -v ") {
            let parts: Vec<&str> = l.split("-v ").collect();
            if parts.len() > 1 {
                let v = parts[1].split_whitespace().next().unwrap_or("").replace('\\', "");
                if !v.is_empty() {
                    volumes.push(v);
                }
            }
        }

        // Variables d'environnement clés
        if l.starts_with("-e ") || l.contains(" -e ") {
            let parts: Vec<&str> = l.split("-e ").collect();
            if parts.len() > 1 {
                let pair = parts[1].split_whitespace().next().unwrap_or("").replace('\\', "");
                if let Some((k, v)) = pair.split_once('=') {
                    let k = k.trim();
                    let v = v.trim();
                    // Masquer les clés et mots de passe
                    if k.contains("PASSWORD") || k.contains("SECRET") || k.contains("KEY") {
                        env.insert(k.to_string(), "••••••••••••".to_string());
                    } else if !k.is_empty() {
                        env.insert(k.to_string(), v.to_string());
                    }

                    // Détection des ports via les variables d'environnement
                    if k == "PORT" || k == "WEBUI_PORT" || k == "HERMES_DASHBOARD_PORT" || k == "API_SERVER_PORT" {
                        if let Ok(p) = v.parse::<u16>() {
                            let url = if p == 8080 {
                                Some("http://localhost:8080".to_string())
                            } else if p == 9119 {
                                Some("http://localhost:9119".to_string())
                            } else if p == 8642 {
                                Some("http://localhost:8642/health".to_string())
                            } else {
                                Some(format!("http://localhost:{}", p))
                            };

                            ports.push(PodmanPortInfo {
                                host_ip: "0.0.0.0".to_string(),
                                host_port: p,
                                container_port: p,
                                protocol: "TCP".to_string(),
                                is_http: true,
                                url,
                            });
                        }
                    }
                }
            }
        }
    }

    // Config options spécifiques par conteneur pour le résumé
    if name == "open-webui" {
        env.insert("Moteur IA backend".to_string(), "Ollama (127.0.0.1:11434)".to_string());
        env.insert("Agent IA connecté".to_string(), "Hermes Agent (127.0.0.1:8642)".to_string());
        env.insert("Télémétrie".to_string(), "Désactivée (DO_NOT_TRACK)".to_string());
    } else if name == "hermes-agent" {
        env.insert("Modèle IA par défaut".to_string(), "Hermes 3 (Nous Research)".to_string());
        env.insert("Serveur LLM".to_string(), "Ollama (127.0.0.1:11434)".to_string());
        env.insert("Mode Gateway".to_string(), "Actif (Port 8642 & Dashboard 9119)".to_string());
    }
}

fn format_bytes_human(bytes: u64) -> String {
    if bytes == 0 {
        return "0 Mo".to_string();
    }
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    let b = bytes as f64;
    if b >= GIB {
        format!("{:.2} Go", b / GIB)
    } else if b >= MIB {
        format!("{:.1} Mo", b / MIB)
    } else {
        format!("{:.0} Ko", b / KIB)
    }
}

#[tauri::command]
pub async fn get_podman_logs(unit_name: String, lines: u32) -> Result<String, String> {
    let target_unit = if !unit_name.ends_with(".service") {
        format!("podman-{}.service", unit_name)
    } else {
        unit_name
    };

    let line_str = lines.to_string();
    let out = Command::new("journalctl")
        .args(["-u", &target_unit, "-n", &line_str, "--no-pager"])
        .output()
        .map_err(|e| format!("Erreur exécution journalctl: {}", e))?;

    if out.status.success() {
        let logs = String::from_utf8_lossy(&out.stdout).to_string();
        if logs.trim().is_empty() {
            Ok(format!("Aucun journal récent disponible pour {}", target_unit))
        } else {
            Ok(logs)
        }
    } else {
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        Err(format!("Impossible de charger les logs: {}", err))
    }
}

#[tauri::command]
pub async fn restart_podman_container(unit_name: String) -> Result<String, String> {
    let target_unit = if !unit_name.ends_with(".service") {
        format!("podman-{}.service", unit_name)
    } else {
        unit_name
    };

    let res = Command::new("systemctl")
        .args(["restart", &target_unit])
        .output()
        .map_err(|e| format!("Erreur système: {}", e))?;

    if res.status.success() {
        Ok(format!("Conteneur {} redémarré avec succès !", target_unit))
    } else {
        let err = String::from_utf8_lossy(&res.stderr).to_string();
        Err(format!("Erreur lors du redémarrage: {}", err))
    }
}
