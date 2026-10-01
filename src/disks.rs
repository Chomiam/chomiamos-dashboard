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
pub struct DiskPowerConfig {
    pub drive_id: Option<String>,
    pub is_rotational: bool,
    pub media_type: String, // "NVMe SSD", "SATA SSD", "HDD", "SSD"
    pub rotation_rate: Option<u32>, // 0 pour SSD, 5400/7200 pour HDD
    pub standby_timeout_minutes: Option<u32>, // 0 = désactivé, 10, 20, 30, etc.
    pub standby_timeout_raw: Option<u32>,
    pub apm_level: Option<u32>, // 1..255 (255 = désactivé/max perf)
    pub is_sleep_disabled: bool,
    pub kernel_pm_control: Option<String>, // "on", "auto"
    pub temperature_c: Option<i32>,
    pub power_state: String, // "active", "standby", "unknown"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskDevice {
    pub name: String,
    pub path: String,
    pub model: Option<String>,
    pub size: String,
    pub size_bytes: u64,
    pub partitions: Vec<PartitionInfo>,
    pub power: DiskPowerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentMountConfig {
    pub mount_point: String,
    pub uuid: String,
    pub fs_type: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingMountInfo {
    pub mount_point: String,
    pub uuid: String,
    pub fs_type: String,
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

pub fn generate_mount_nix_content(mounts: &[PersistentMountConfig]) -> String {
    let mut tmpfiles_rules = String::new();
    let mut filesystems = String::new();

    for m in mounts {
        tmpfiles_rules.push_str(&format!(
            "    \"d {} 0775 ${{username}} users -\"\n    \"z {} 0775 ${{username}} users -\"\n",
            m.mount_point, m.mount_point
        ));

        let mut opts = m.options.clone();
        if !opts.iter().any(|o| o == "defaults") {
            opts.insert(0, "defaults".to_string());
        }
        if !opts.iter().any(|o| o == "nofail") {
            opts.push("nofail".to_string());
        }
        if !opts.iter().any(|o| o.starts_with("x-systemd.device-timeout")) {
            opts.push("x-systemd.device-timeout=5s".to_string());
        }
        if !opts.iter().any(|o| o.starts_with("x-systemd.mount-timeout")) {
            opts.push("x-systemd.mount-timeout=5s".to_string());
        }
        if !opts.iter().any(|o| o == "x-gvfs-show") {
            opts.push("x-gvfs-show".to_string());
        }

        let mut opts_str = String::new();
        for opt in &opts {
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

    format!(
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
    )
}

pub fn write_persistent_mounts(mounts: &[PersistentMountConfig]) -> Result<(), String> {
    let generated = generate_mount_nix_content(mounts);

    let path = Path::new(MOUNT_NIX_PATH);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Err(e) = fs::write(path, &generated) {
        // Fallback avec pkexec si la permission utilisateur est insuffisante
        let script = format!("cat << 'EOF' > {}
{}
EOF", MOUNT_NIX_PATH, generated);
        let output = Command::new("pkexec")
            .args(["sh", "-c", &script])
            .output()
            .map_err(|pk_err| format!("Impossible d'écrire dans {} (fs: {}, pkexec: {})", MOUNT_NIX_PATH, e, pk_err))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Impossible d'écrire dans {} : {}", MOUNT_NIX_PATH, err_msg.trim()));
        }
    }

    Ok(())
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

pub fn minutes_to_standby_timeout(minutes: u32) -> u32 {
    if minutes == 0 {
        0
    } else if minutes <= 20 {
        ((minutes * 60) / 5).clamp(1, 240)
    } else if minutes <= 330 {
        240 + ((minutes + 15) / 30).clamp(1, 11)
    } else {
        244
    }
}

pub fn standby_timeout_to_minutes(val: u32) -> Option<u32> {
    match val {
        0 => Some(0),
        1..=240 => Some((val * 5) / 60),
        241..=251 => Some((val - 240) * 30),
        252 => Some(21),
        253 => Some(480),
        255 => Some(21),
        _ => None,
    }
}

pub fn get_disk_power_info(dev_name: &str) -> DiskPowerConfig {
    let is_rotational = fs::read_to_string(format!("/sys/block/{}/queue/rotational", dev_name))
        .map(|s| s.trim() == "1")
        .unwrap_or(false);

    let media_type = if dev_name.starts_with("nvme") {
        "NVMe SSD".to_string()
    } else if is_rotational {
        "HDD".to_string()
    } else if dev_name.starts_with("sd") || dev_name.starts_with("hd") {
        "SATA SSD".to_string()
    } else {
        "SSD".to_string()
    };

    let kernel_pm_control = fs::read_to_string(format!("/sys/block/{}/device/power/control", dev_name))
        .or_else(|_| fs::read_to_string(format!("/sys/block/{}/power/control", dev_name)))
        .map(|s| s.trim().to_string())
        .ok();

    let mut drive_id = None;
    let mut drive_obj_name = None;
    let mut rotation_rate = if is_rotational { Some(7200) } else { Some(0) };
    let mut standby_timeout_raw = None;
    let mut apm_level = None;
    let mut temperature_c = None;

    // 1. Interroger udisksctl pour le block device
    if let Ok(output) = Command::new("udisksctl")
        .args(["info", "-b", &format!("/dev/{}", dev_name)])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Drive:") {
                    let parts: Vec<&str> = trimmed.split('\'').collect();
                    if parts.len() >= 2 {
                        let path = parts[1];
                        if let Some(obj_name) = path.strip_prefix("/org/freedesktop/UDisks2/drives/") {
                            drive_obj_name = Some(obj_name.to_string());
                        }
                    }
                } else if trimmed.starts_with("SmartTemperature:") {
                    if let Some(val_str) = trimmed.strip_prefix("SmartTemperature:") {
                        if let Ok(kelvin) = val_str.trim().parse::<i32>() {
                            if kelvin > 200 && kelvin < 400 {
                                temperature_c = Some(kelvin - 273);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Si un objet Drive a été trouvé, interroger udisksctl info -d <drive_obj_name>
    if let Some(ref obj_name) = drive_obj_name {
        if let Ok(output) = Command::new("udisksctl")
            .args(["info", "-d", obj_name])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Id:") {
                        let id_val = trimmed.strip_prefix("Id:").unwrap_or("").trim();
                        if !id_val.is_empty() {
                            drive_id = Some(id_val.to_string());
                        }
                    } else if trimmed.starts_with("RotationRate:") {
                        if let Some(val_str) = trimmed.strip_prefix("RotationRate:") {
                            if let Ok(r) = val_str.trim().parse::<u32>() {
                                rotation_rate = Some(r);
                            }
                        }
                    } else if trimmed.starts_with("SmartTemperature:") && temperature_c.is_none() {
                        if let Some(val_str) = trimmed.strip_prefix("SmartTemperature:") {
                            if let Ok(kelvin) = val_str.trim().parse::<i32>() {
                                if kelvin > 200 && kelvin < 400 {
                                    temperature_c = Some(kelvin - 273);
                                }
                            }
                        }
                    } else if trimmed.starts_with("Configuration:") {
                        if let Some(pos) = trimmed.find("'ata-pm-standby': <") {
                            let sub = &trimmed[pos + "'ata-pm-standby': <".len()..];
                            if let Some(end) = sub.find('>') {
                                if let Ok(val) = sub[..end].trim().parse::<u32>() {
                                    standby_timeout_raw = Some(val);
                                }
                            }
                        }
                        if let Some(pos) = trimmed.find("'ata-apm-level': <") {
                            let sub = &trimmed[pos + "'ata-apm-level': <".len()..];
                            if let Some(end) = sub.find('>') {
                                if let Ok(val) = sub[..end].trim().parse::<u32>() {
                                    apm_level = Some(val);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fallback drive_id via udevadm si non trouvé
    if drive_id.is_none() {
        if let Ok(output) = Command::new("udevadm")
            .args(["info", "--query=property", &format!("--name=/dev/{}", dev_name)])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some(serial) = line.strip_prefix("ID_SERIAL=") {
                        drive_id = Some(serial.trim().replace('_', "-"));
                        break;
                    }
                }
            }
        }
    }

    // 4. Vérifier si un fichier .conf existe dans /etc/udisks2/
    if let Some(ref id) = drive_id {
        let conf_path = format!("/etc/udisks2/{}.conf", id);
        if let Ok(content) = fs::read_to_string(&conf_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(val_str) = trimmed.strip_prefix("StandbyTimeout=") {
                    if let Ok(val) = val_str.trim().parse::<u32>() {
                        standby_timeout_raw = Some(val);
                    }
                } else if let Some(val_str) = trimmed.strip_prefix("APMLevel=") {
                    if let Ok(val) = val_str.trim().parse::<u32>() {
                        apm_level = Some(val);
                    }
                }
            }
        }
    }

    let standby_timeout_minutes = standby_timeout_raw.and_then(standby_timeout_to_minutes);

    let is_sleep_disabled = match (standby_timeout_raw, apm_level) {
        (Some(0), Some(apm)) if apm >= 254 => true,
        (Some(0), _) => true,
        _ => {
            if kernel_pm_control.as_deref() == Some("on") && standby_timeout_raw == Some(0) {
                true
            } else {
                false
            }
        }
    };

    let power_state = if is_sleep_disabled {
        "active".to_string()
    } else {
        "auto".to_string()
    };

    DiskPowerConfig {
        drive_id,
        is_rotational,
        media_type,
        rotation_rate,
        standby_timeout_minutes,
        standby_timeout_raw,
        apm_level,
        is_sleep_disabled,
        kernel_pm_control,
        temperature_c,
        power_state,
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

    // Auto-migration : fiabiliser mount.nix en injectant les options système anti-blocage au boot
    // (nofail, x-systemd.device-timeout=5s, x-systemd.mount-timeout=5s et x-gvfs-show)
    if !persistent_mounts.is_empty()
        && persistent_mounts.iter().any(|m| {
            !m.options.iter().any(|o| o == "nofail")
                || !m.options.iter().any(|o| o.starts_with("x-systemd.device-timeout"))
                || !m.options.iter().any(|o| o.starts_with("x-systemd.mount-timeout"))
                || !m.options.iter().any(|o| o == "x-gvfs-show")
        })
    {
        let _ = write_persistent_mounts(&persistent_mounts);
    }

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

        let power = get_disk_power_info(&b.name);

        devices.push(DiskDevice {
            name: b.name.clone(),
            path: format!("/dev/{}", b.name),
            model: b.model,
            size: format_bytes(disk_size_bytes),
            size_bytes: disk_size_bytes,
            partitions,
            power,
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
        "btrfs" => vec![
            "defaults".into(),
            "nofail".into(),
            "x-systemd.device-timeout=5s".into(),
            "x-systemd.mount-timeout=5s".into(),
            "compress=zstd".into(),
            "x-gvfs-show".into(),
        ],
        "ext4" => vec![
            "defaults".into(),
            "nofail".into(),
            "x-systemd.device-timeout=5s".into(),
            "x-systemd.mount-timeout=5s".into(),
            "x-gvfs-show".into(),
        ],
        "ntfs" | "vfat" | "exfat" => vec![
            "defaults".into(),
            "nofail".into(),
            "x-systemd.device-timeout=5s".into(),
            "x-systemd.mount-timeout=5s".into(),
            "uid=1000".into(),
            "gid=100".into(),
            "dmask=022".into(),
            "fmask=133".into(),
            "x-gvfs-show".into(),
        ],
        _ => vec![
            "defaults".into(),
            "nofail".into(),
            "x-systemd.device-timeout=5s".into(),
            "x-systemd.mount-timeout=5s".into(),
            "x-gvfs-show".into(),
        ],
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
        "mkdir -p '{mnt}' && (mount -o x-gvfs-show /dev/disk/by-uuid/{uuid} '{mnt}' 2>/dev/null || mount /dev/disk/by-uuid/{uuid} '{mnt}' 2>/dev/null || mount -o remount,x-gvfs-show '{mnt}' 2>/dev/null) ; chown -R {user}:users '{mnt}' 2>/dev/null || true",
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

/// Filtre les points de montage déclarés dans mount.nix dont l'UUID n'est pas présent dans la liste des UUIDs actifs.
pub fn filter_missing_mounts(
    mounts: &[PersistentMountConfig],
    active_uuids: &std::collections::HashSet<String>,
) -> Vec<MissingMountInfo> {
    let mut missing = Vec::new();
    for pm in mounts {
        let clean_uuid = pm.uuid.trim().to_lowercase();
        if !active_uuids.contains(&clean_uuid) {
            missing.push(MissingMountInfo {
                mount_point: pm.mount_point.clone(),
                uuid: pm.uuid.clone(),
                fs_type: pm.fs_type.clone(),
            });
        }
    }
    missing
}

/// Vérifie si les disques déclarés dans mount.nix sont encore physiquement présents dans /dev/disk/by-uuid.
/// Retourne la liste des montages orphelins (disques retirés ou reformatés avec un nouvel UUID).
pub fn check_missing_persistent_mounts() -> Vec<MissingMountInfo> {
    let persistent_mounts = read_persistent_mounts();
    if persistent_mounts.is_empty() {
        return Vec::new();
    }

    let mut active_uuids = std::collections::HashSet::new();
    if let Ok(entries) = fs::read_dir("/dev/disk/by-uuid") {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                // S'assurer que le lien symbolique pointe bien vers un block device existant
                if entry.path().exists() {
                    active_uuids.insert(name.to_lowercase());
                }
            }
        }
    }

    filter_missing_mounts(&persistent_mounts, &active_uuids)
}

/// Supprime la déclaration d'un disque manquant dans mount.nix et nettoie le point de montage.
pub fn remove_persistent_mount(mount_point: &str, uuid: &str) -> Result<String, String> {
    let clean_mount = mount_point.trim().trim_end_matches('/');

    let forbidden = ["/", "/boot", "/efi", "/nix", "/nix/store"];
    if forbidden.contains(&clean_mount) {
        return Err(format!("Impossible de retirer le point de montage système '{}'.", clean_mount));
    }

    // 1. Retirer de mount.nix
    let mut mounts = read_persistent_mounts();
    let initial_count = mounts.len();
    mounts.retain(|m| {
        let m_clean = m.mount_point.trim().trim_end_matches('/');
        !m.uuid.eq_ignore_ascii_case(uuid) && m_clean != clean_mount
    });

    if mounts.len() == initial_count {
        // Fallback si correspondance seulement sur le point de montage ou sur l'UUID
        mounts.retain(|m| {
            let m_clean = m.mount_point.trim().trim_end_matches('/');
            !m.uuid.eq_ignore_ascii_case(uuid) && m_clean != clean_mount
        });
    }

    write_persistent_mounts(&mounts)?;

    // 2. Tenter un démontage paresseux (lazy) non bloquant au cas où un reliquat existerait
    let script = format!("umount -l '{mnt}' 2>/dev/null || true", mnt = clean_mount);
    let _ = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output();

    Ok(format!("La déclaration du point de montage '{}' a été retirée de mount.nix.", clean_mount))
}

/// Configure ou désactive la mise en veille automatique d'un disque de stockage.
/// Écrit la configuration de manière permanente dans /etc/udisks2/<DriveId>.conf,
/// applique le Runtime PM au niveau du noyau (/sys/block/<dev>/power/control)
/// et applique directement la commande firmware ATA si hdparm est disponible.
pub fn set_disk_sleep_config(
    device_name: String,
    drive_id: Option<String>,
    disable_sleep: bool,
    timeout_minutes: Option<u32>,
    apm_level: Option<u32>,
) -> Result<String, String> {
    let clean_dev = device_name.trim().trim_start_matches("/dev/").to_string();

    let target_drive_id = if let Some(id) = drive_id {
        id
    } else {
        let power_info = get_disk_power_info(&clean_dev);
        power_info.drive_id.unwrap_or_else(|| clean_dev.clone())
    };

    let (standby_val, apm_val, kernel_pm) = if disable_sleep {
        (0, 255, "on")
    } else {
        let mins = timeout_minutes.unwrap_or(20);
        let s_val = minutes_to_standby_timeout(mins);
        let a_val = apm_level.unwrap_or(128);
        (s_val, a_val, "auto")
    };

    let conf_content = format!(
r#"[ATA]
StandbyTimeout={}
APMLevel={}
"#,
        standby_val, apm_val
    );

    let script = format!(
r#"mkdir -p /etc/udisks2 && cat << 'EOF' > '/etc/udisks2/{id}.conf'
{conf}EOF
chmod 644 '/etc/udisks2/{id}.conf'

if [ -f "/sys/block/{dev}/device/power/control" ]; then
    echo "{kpm}" > "/sys/block/{dev}/device/power/control" 2>/dev/null || true
fi
if [ -f "/sys/block/{dev}/power/control" ]; then
    echo "{kpm}" > "/sys/block/{dev}/power/control" 2>/dev/null || true
fi

if command -v hdparm >/dev/null 2>&1; then
    hdparm -S {standby} -B {apm} "/dev/{dev}" 2>/dev/null || true
fi
"#,
        id = target_drive_id,
        conf = conf_content,
        dev = clean_dev,
        kpm = kernel_pm,
        standby = standby_val,
        apm = apm_val
    );

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Échec de l'application de la gestion d'énergie : {}", err_msg.trim()));
    }

    // Notifier udisks2 via D-Bus pour recharger instantanément la configuration
    let _ = Command::new("gdbus")
        .args([
            "call",
            "--system",
            "--dest",
            "org.freedesktop.UDisks2",
            "--object-path",
            &format!("/org/freedesktop/UDisks2/drives/{}", target_drive_id.replace('-', "_")),
            "--method",
            "org.freedesktop.UDisks2.Drive.SetConfiguration",
            &format!("{{'ata-pm-standby': <{}>, 'ata-apm-level': <{}>}}", standby_val, apm_val),
            "{}",
        ])
        .output();

    if disable_sleep {
        Ok(format!(
            "Mise en veille automatique DÉSACTIVÉE avec succès pour /dev/{} (Disque toujours actif et prêt sans latence).",
            clean_dev
        ))
    } else {
        let mins_desc = if standby_val == 0 {
            "Désactivée".to_string()
        } else {
            format!("{} minutes", timeout_minutes.unwrap_or(20))
        };
        Ok(format!(
            "Gestion de l'énergie configurée pour /dev/{} (Veille après {}, APM niveau {}).",
            clean_dev, mins_desc, apm_val
        ))
    }
}

/// Réinitialise les paramètres de mise en veille d'un disque aux valeurs système par défaut.
pub fn reset_disk_sleep_config(
    device_name: String,
    drive_id: Option<String>,
) -> Result<String, String> {
    let clean_dev = device_name.trim().trim_start_matches("/dev/").to_string();

    let target_drive_id = if let Some(id) = drive_id {
        id
    } else {
        let power_info = get_disk_power_info(&clean_dev);
        power_info.drive_id.unwrap_or_else(|| clean_dev.clone())
    };

    let script = format!(
r#"rm -f '/etc/udisks2/{id}.conf'
if [ -f "/sys/block/{dev}/device/power/control" ]; then
    echo "auto" > "/sys/block/{dev}/device/power/control" 2>/dev/null || true
fi
if [ -f "/sys/block/{dev}/power/control" ]; then
    echo "auto" > "/sys/block/{dev}/power/control" 2>/dev/null || true
fi
"#,
        id = target_drive_id,
        dev = clean_dev
    );

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Échec de la réinitialisation : {}", err_msg.trim()));
    }

    let _ = Command::new("gdbus")
        .args([
            "call",
            "--system",
            "--dest",
            "org.freedesktop.UDisks2",
            "--object-path",
            &format!("/org/freedesktop/UDisks2/drives/{}", target_drive_id.replace('-', "_")),
            "--method",
            "org.freedesktop.UDisks2.Drive.SetConfiguration",
            "{}",
            "{}",
        ])
        .output();

    Ok(format!("Paramètres d'énergie réinitialisés aux valeurs par défaut pour /dev/{}.", clean_dev))
}

/// Applique ou désactive la veille pour l'ensemble des disques de stockage détectés.
pub fn set_all_disks_sleep_config(disable_sleep: bool) -> Result<String, String> {
    let devices = list_storage_devices()?;
    let mut modified = 0;

    for dev in &devices {
        if dev.name.starts_with("loop") || dev.name.starts_with("ram") || dev.name.starts_with("zram") {
            continue;
        }

        let _ = set_disk_sleep_config(
            dev.name.clone(),
            dev.power.drive_id.clone(),
            disable_sleep,
            None,
            None,
        );
        modified += 1;
    }

    if disable_sleep {
        Ok(format!(
            "Mise en veille automatique désactivée pour tous les disques ({} disques configurés en mode Toujours Actif).",
            modified
        ))
    } else {
        Ok(format!(
            "Gestion d'énergie réinitialisée pour l'ensemble des {} disques de stockage.",
            modified
        ))
    }
}

/// Teste la mise en veille mécanique immédiate d'un disque (spindown).
pub fn test_disk_standby(device_name: String) -> Result<String, String> {
    let clean_dev = device_name.trim().trim_start_matches("/dev/").to_string();
    let script = format!(
        "hdparm -y '/dev/{dev}' 2>/dev/null || smartctl -s standby,now '/dev/{dev}' 2>/dev/null || true",
        dev = clean_dev
    );

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Échec du test de mise en veille : {}", err_msg.trim()));
    }

    Ok(format!("Commande de mise en veille envoyée à /dev/{}.", clean_dev))
}

/// Réveille immédiatement un disque endormi en forçant une lecture de bloc de test.
pub fn wake_disk(device_name: String) -> Result<String, String> {
    let clean_dev = device_name.trim().trim_start_matches("/dev/").to_string();
    let script = format!("dd if='/dev/{}' of=/dev/null count=1 bs=512 2>/dev/null || true", clean_dev);

    let output = Command::new("pkexec")
        .args(["sh", "-c", &script])
        .output()
        .map_err(|e| format!("Erreur d'élévation pkexec : {}", e))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Échec de la commande de réveil : {}", err_msg.trim()));
    }

    Ok(format!("Commande de réveil envoyée à /dev/{} (lecture de test effectuée).", clean_dev))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standby_timeout_conversions() {
        assert_eq!(minutes_to_standby_timeout(0), 0);
        assert_eq!(standby_timeout_to_minutes(0), Some(0));

        // 10 minutes -> 120 (120 * 5s = 600s = 10 min)
        assert_eq!(minutes_to_standby_timeout(10), 120);
        assert_eq!(standby_timeout_to_minutes(120), Some(10));

        // 20 minutes -> 240
        assert_eq!(minutes_to_standby_timeout(20), 240);
        assert_eq!(standby_timeout_to_minutes(240), Some(20));

        // 30 minutes -> 241
        assert_eq!(minutes_to_standby_timeout(30), 241);
        assert_eq!(standby_timeout_to_minutes(241), Some(30));

        // 1 heure (60 min) -> 242
        assert_eq!(minutes_to_standby_timeout(60), 242);
        assert_eq!(standby_timeout_to_minutes(242), Some(60));

        // 2 heures (120 min) -> 244
        assert_eq!(minutes_to_standby_timeout(120), 244);
        assert_eq!(standby_timeout_to_minutes(244), Some(120));
    }

    #[test]
    #[ignore = "nécessite lsblk et le matériel physique hôte (inaccessible dans la sandbox Nix)"]
    fn test_list_storage_devices_with_power() {
        let devices = list_storage_devices().expect("list_storage_devices doit réussir");
        for dev in devices {
            assert!(!dev.name.is_empty());
            assert!(!dev.power.media_type.is_empty());
        }
    }

    #[test]
    fn test_generate_mount_nix_adds_gvfs_show() {
        let dummy_mounts = vec![
            PersistentMountConfig {
                mount_point: "/mnt/hdd4to".to_string(),
                uuid: "1234-5678".to_string(),
                fs_type: "ext4".to_string(),
                options: vec!["defaults".to_string(), "nofail".to_string()],
            },
            PersistentMountConfig {
                mount_point: "/mnt/Emulation".to_string(),
                uuid: "abcd-ef01".to_string(),
                fs_type: "btrfs".to_string(),
                options: vec!["defaults".to_string(), "compress=zstd".to_string(), "x-gvfs-show".to_string()],
            },
        ];

        let generated = generate_mount_nix_content(&dummy_mounts);
        assert!(generated.contains(r#""x-gvfs-show""#));
        assert!(generated.contains(r#""nofail""#));
        assert!(generated.contains(r#""x-systemd.device-timeout=5s""#));
        assert!(generated.contains(r#""x-systemd.mount-timeout=5s""#));
        assert!(generated.contains(r#"fileSystems."/mnt/hdd4to""#));
        assert!(generated.contains(r#"fileSystems."/mnt/Emulation""#));
        assert!(generated.contains(r#"options = ["#));
    }
    #[test]
    fn test_filter_missing_mounts() {
        let dummy_mounts = vec![
            PersistentMountConfig {
                mount_point: "/mnt/Existing".to_string(),
                uuid: "1111-2222".to_string(),
                fs_type: "ext4".to_string(),
                options: vec!["defaults".to_string()],
            },
            PersistentMountConfig {
                mount_point: "/mnt/MissingDisk".to_string(),
                uuid: "3333-4444".to_string(),
                fs_type: "btrfs".to_string(),
                options: vec!["defaults".to_string()],
            },
        ];

        let mut active = std::collections::HashSet::new();
        active.insert("1111-2222".to_string());

        let missing = filter_missing_mounts(&dummy_mounts, &active);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].mount_point, "/mnt/MissingDisk");
        assert_eq!(missing[0].uuid, "3333-4444");
    }
}
