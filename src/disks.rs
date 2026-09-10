use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionInfo {
    pub name: String,
    pub path: String,
    pub size: String,
    pub size_bytes: u64,
    pub fstype: Option<String>,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub mountpoints: Vec<String>,
    pub is_root: bool,
    pub is_boot: bool,
    pub is_swap: bool,
    pub is_persistent_nix: bool,
    pub persistent_mount_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskDevice {
    pub name: String,
    pub path: String,
    pub model: Option<String>,
    pub size: String,
    pub size_bytes: u64,
    pub partitions: Vec<PartitionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentMountConfig {
    pub mount_point: String,
    pub uuid: String,
    pub fs_type: String,
    pub options: Vec<String>,
}

const MOUNT_NIX_PATH: &str = "/etc/nixos/hosts/desktop/mount.nix";

fn format_bytes(bytes: u64) -> String {
    const TO: f64 = 1_099_511_627_776.0;
    const GO: f64 = 1_073_741_824.0;
    const MO: f64 = 1_048_576.0;

    if bytes as f64 >= TO {
        format!("{:.1} To", bytes as f64 / TO)
    } else if bytes as f64 >= GO {
        format!("{:.1} Go", bytes as f64 / GO)
    } else if bytes as f64 >= MO {
        format!("{:.1} Mo", bytes as f64 / MO)
    } else {
        format!("{} octets", bytes)
    }
}

pub fn read_persistent_mounts() -> Vec<PersistentMountConfig> {
    let mut results = Vec::new();
    let content = match fs::read_to_string(MOUNT_NIX_PATH) {
        Ok(c) => c,
        Err(_) => return results,
    };

    let re_fs = regex::Regex::new(r#"fileSystems\."([^"]+)"\s*=\s*\{([^}]+)\};"#).unwrap();
    let re_uuid = regex::Regex::new(r#"/dev/disk/by-uuid/([a-zA-Z0-9_-]+)"#).unwrap();
    let re_type = regex::Regex::new(r#"fsType\s*=\s*"([^"]+)""#).unwrap();
    let re_opt = regex::Regex::new(r#""([^"]+)""#).unwrap();

    for cap in re_fs.captures_iter(&content) {
        let mount_point = cap[1].to_string();
        let body = &cap[2];

        let uuid = match re_uuid.captures(body) {
            Some(c) => c[1].to_string(),
            None => continue,
        };

        let fs_type = match re_type.captures(body) {
            Some(c) => c[1].to_string(),
            None => "ext4".to_string(),
        };

        let mut options = Vec::new();
        if let Some(opts_idx) = body.find("options = [") {
            let sub = &body[opts_idx..];
            if let Some(end_idx) = sub.find(']') {
                for opt_cap in re_opt.captures_iter(&sub[..end_idx]) {
                    options.push(opt_cap[1].to_string());
                }
            }
        }

        results.push(PersistentMountConfig {
            mount_point,
            uuid,
            fs_type,
            options,
        });
    }

    results
}

pub fn write_persistent_mounts(mounts: &[PersistentMountConfig]) -> Result<(), String> {
    let mut tmpfiles_rules = String::new();
    let mut filesystems = String::new();

    for m in mounts {
        tmpfiles_rules.push_str(&format!(
            "    \"d {} 0775 ${{username}} users -\"\n    \"z {} 0775 ${{username}} users -\"\n",
            m.mount_point, m.mount_point
        ));

        let mut opts_str = String::new();
        for opt in &m.options {
            opts_str.push_str(&format!("      \"{}\"\n", opt));
        }

        filesystems.push_str(&format!(
r#"  fileSystems."{}" = {{
    device = "/dev/disk/by-uuid/{}";
    fsType = "{}";
    options = [
{}    ];
  }};

"#,
            m.mount_point, m.uuid, m.fs_type, opts_str
        ));
    }

    let generated = format!(
r#"{{ config, pkgs, lib, ... }}:

let
  username = config.chomiamos.user.username;
in
{{
  # =========================================================================
  # 💾 MONTAGES DE DISQUES PERSISTANTS (GÉRÉS PAR CHOMIAMOS DASHBOARD)
  # Ce fichier est préservé automatiquement lors des synchronisations GitHub.
  # =========================================================================

  systemd.tmpfiles.rules = [
{}  ];

{}  systemd.services.systemd-tmpfiles-setup.after = [ "local-fs.target" ];
}}
"#,
        tmpfiles_rules, filesystems
    );

    let path = Path::new(MOUNT_NIX_PATH);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    fs::write(path, generated)
        .map_err(|e| format!("Impossible d'écrire dans {} : {}", MOUNT_NIX_PATH, e))
}

#[derive(Deserialize)]
struct LsblkItem {
    name: String,
    size: Option<serde_json::Value>,
    #[serde(rename = "type")]
    device_type: Option<String>,
    fstype: Option<String>,
    label: Option<String>,
    mountpoints: Option<Vec<Option<String>>>,
    uuid: Option<String>,
    model: Option<String>,
    children: Option<Vec<LsblkItem>>,
}

#[derive(Deserialize)]
struct LsblkOutput {
    blockdevices: Vec<LsblkItem>,
}

fn parse_size_bytes(val: &Option<serde_json::Value>) -> u64 {
    match val {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.parse::<u64>().unwrap_or(0),
        _ => 0,
    }
}

pub fn list_storage_devices() -> Result<Vec<DiskDevice>, String> {
    let output = Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,SIZE,TYPE,FSTYPE,LABEL,MOUNTPOINTS,UUID,MODEL",
        ])
        .output()
        .map_err(|e| format!("Erreur lors de l'exécution de lsblk : {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "lsblk a échoué avec le code : {:?}",
            output.status.code()
        ));
    }

    let parsed: LsblkOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Erreur de désérialisation du JSON lsblk : {}", e))?;

    let persistent_mounts = read_persistent_mounts();
    let mut devices = Vec::new();

    for b in parsed.blockdevices {
        // Ignorer les périphériques virtuels en lecture seule zram / loop
        let dev_type = b.device_type.clone().unwrap_or_default();
        if b.name.starts_with("loop") || b.name.starts_with("ram") {
            continue;
        }

        let disk_size_bytes = parse_size_bytes(&b.size);
        let mut partitions = Vec::new();

        if let Some(children) = b.children {
            for c in children {
                let part_size_bytes = parse_size_bytes(&c.size);
                let mountpoints: Vec<String> = c
                    .mountpoints
                    .unwrap_or_default()
                    .into_iter()
                    .flatten()
                    .filter(|m| !m.is_empty())
                    .collect();

                let is_root = mountpoints.iter().any(|m| m == "/" || m == "/nix/store");
                let is_boot = mountpoints.iter().any(|m| m == "/boot" || m == "/efi");
                let is_swap = c.fstype.as_deref() == Some("swap")
                    || c.device_type.as_deref() == Some("swap")
                    || mountpoints.iter().any(|m| m == "[SWAP]");

                let mut is_persistent_nix = false;
                let mut persistent_mount_path = None;

                if let Some(ref u) = c.uuid {
                    if let Some(pm) = persistent_mounts.iter().find(|p| p.uuid == *u) {
                        is_persistent_nix = true;
                        persistent_mount_path = Some(pm.mount_point.clone());
                    }
                }

                partitions.push(PartitionInfo {
                    name: c.name.clone(),
                    path: format!("/dev/{}", c.name),
                    size: format_bytes(part_size_bytes),
                    size_bytes: part_size_bytes,
                    fstype: c.fstype,
                    label: c.label,
                    uuid: c.uuid,
                    mountpoints,
                    is_root,
                    is_boot,
                    is_swap,
                    is_persistent_nix,
                    persistent_mount_path,
                });
            }
        } else if dev_type == "disk" || dev_type == "part" {
            // Disque entier sans table de partitionnement séparée
            let mountpoints: Vec<String> = b
                .mountpoints
                .unwrap_or_default()
                .into_iter()
                .flatten()
                .filter(|m| !m.is_empty())
                .collect();

            let is_root = mountpoints.iter().any(|m| m == "/" || m == "/nix/store");
            let is_boot = mountpoints.iter().any(|m| m == "/boot" || m == "/efi");
            let is_swap = b.fstype.as_deref() == Some("swap")
                || dev_type == "swap"
                || mountpoints.iter().any(|m| m == "[SWAP]");

            let mut is_persistent_nix = false;
            let mut persistent_mount_path = None;

            if let Some(ref u) = b.uuid {
                if let Some(pm) = persistent_mounts.iter().find(|p| p.uuid == *u) {
                    is_persistent_nix = true;
                    persistent_mount_path = Some(pm.mount_point.clone());
                }
            }

            if b.fstype.is_some() || disk_size_bytes > 0 {
                partitions.push(PartitionInfo {
                    name: b.name.clone(),
                    path: format!("/dev/{}", b.name),
                    size: format_bytes(disk_size_bytes),
                    size_bytes: disk_size_bytes,
                    fstype: b.fstype.clone(),
                    label: b.label.clone(),
                    uuid: b.uuid.clone(),
                    mountpoints,
                    is_root,
                    is_boot,
                    is_swap,
                    is_persistent_nix,
                    persistent_mount_path,
                });
            }
        }

        devices.push(DiskDevice {
            name: b.name.clone(),
            path: format!("/dev/{}", b.name),
            model: b.model,
            size: format_bytes(disk_size_bytes),
            size_bytes: disk_size_bytes,
            partitions,
        });
    }

    Ok(devices)
}

pub fn mount_storage_device(
    uuid: String,
    mount_point: String,
    fs_type: String,
) -> Result<String, String> {
    let username = std::env::var("USER").unwrap_or_else(|_| "chomiam".to_string());
    let clean_mount = mount_point.trim().trim_end_matches('/').replace("chomiam", &username);

    if !clean_mount.starts_with('/') {
        return Err("Le point de montage doit commencer par '/' (ex: /mnt/Jeux)".into());
    }

    let forbidden = ["/", "/boot", "/efi", "/nix", "/etc", "/dev", "/proc", "/sys", "/bin", "/usr", "/root"];
    if forbidden.contains(&clean_mount.as_str()) {
        return Err(format!("Le chemin '{}' est réservé au système.", clean_mount));
    }

    // 1. Mise à jour déclarative de mount.nix
    let mut mounts = read_persistent_mounts();
    mounts.retain(|m| m.uuid != uuid && m.mount_point != clean_mount);

    let options = match fs_type.as_str() {
        "btrfs" => vec!["defaults".into(), "nofail".into(), "compress=zstd".into()],
        "ext4" => vec!["defaults".into(), "nofail".into()],
        "ntfs" | "vfat" | "exfat" => vec![
            "defaults".into(),
            "nofail".into(),
            "uid=1000".into(),
            "gid=100".into(),
            "dmask=022".into(),
            "fmask=133".into(),
        ],
        _ => vec!["defaults".into(), "nofail".into()],
    };

    mounts.push(PersistentMountConfig {
        mount_point: clean_mount.clone(),
        uuid: uuid.clone(),
        fs_type,
        options,
    });

    write_persistent_mounts(&mounts)?;

    // 2. Montage immédiat en direct avec permissions
    let username = std::env::var("USER").unwrap_or_else(|_| "chomiam".to_string());
    let script = format!(
        "mkdir -p '{mnt}' && mount /dev/disk/by-uuid/{uuid} '{mnt}' 2>/dev/null || mount -o remount '{mnt}' 2>/dev/null ; chown -R {user}:users '{mnt}' 2>/dev/null || true",
        mnt = clean_mount,
        uuid = uuid,
        user = username
    );

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Configuration enregistrée dans mount.nix, mais le montage immédiat a échoué : {}",
            err_msg.trim()
        ));
    }

    Ok(format!("Disque monté avec succès sur {}", clean_mount))
}

pub fn unmount_storage_device(
    mount_point: String,
    uuid: Option<String>,
) -> Result<String, String> {
    let username = std::env::var("USER").unwrap_or_else(|_| "chomiam".to_string());
    let clean_mount = mount_point.trim().trim_end_matches('/').replace("chomiam", &username);

    let forbidden = ["/", "/boot", "/efi", "/nix", "/nix/store"];
    if forbidden.contains(&clean_mount.as_str()) {
        return Err(format!("Impossible de démonter le point système '{}'.", clean_mount));
    }

    // 1. Retirer de mount.nix
    let mut mounts = read_persistent_mounts();
    if let Some(ref u) = uuid {
        mounts.retain(|m| m.uuid != *u && m.mount_point != clean_mount);
    } else {
        mounts.retain(|m| m.mount_point != clean_mount);
    }
    write_persistent_mounts(&mounts)?;

    // 2. Démonter en direct
    let script = format!("umount '{mnt}' 2>/dev/null || umount -l '{mnt}'", mnt = clean_mount);
    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Erreur lors du démontage : {}", err_msg.trim()));
    }

    Ok(format!("Disque démonté de {}", clean_mount))
}

pub fn format_storage_device(
    device_path: String,
    fs_type: String,
    label: String,
) -> Result<String, String> {
    if !device_path.starts_with("/dev/") {
        return Err("Chemin de périphérique invalide".into());
    }

    // Sécurité stricte : refuser formellement si monté sur / ou /boot
    let devices = list_storage_devices()?;
    for d in &devices {
        for p in &d.partitions {
            if p.path == device_path {
                if p.is_root || p.is_boot {
                    return Err(format!(
                        "INTERDIT : La partition {} est utilisée par le système d'exploitation.",
                        device_path
                    ));
                }
            }
        }
    }

    let clean_label = label
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>();
    let clean_label = if clean_label.is_empty() { "Storage".to_string() } else { clean_label };

    // Démonter d'abord si monté
    let unmount_cmd = format!("umount '{}' 2>/dev/null || true", device_path);
    let _ = Command::new("pkexec").args(["sh", "-c", &unmount_cmd]).output();

    let format_cmd = match fs_type.to_lowercase().as_str() {
        "btrfs" => format!("mkfs.btrfs -f -L '{}' '{}'", clean_label, device_path),
        _ => format!("mkfs.ext4 -F -L '{}' '{}'", clean_label, device_path),
    };

    let output = Command::new("pkexec")
        .args(["sh", "-c", &format_cmd])
        .output()
        .map_err(|e| format!("Erreur lors de l'exécution du formatage : {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Échec du formatage : {}", err_msg.trim()));
    }

    Ok(format!("Partition {} formatée en {} (Label: {})", device_path, fs_type, clean_label))
}
