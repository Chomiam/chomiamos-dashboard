use serde::Serialize;
use std::fs;
use std::path::Path;
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f32,
    pub file_system: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GpuInfo {
    pub name: String,
    pub driver: String,
    pub usage_percent: Option<f32>,
    pub temp_celsius: Option<f32>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
    pub vram_used_percent: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub hostname: String,
    pub os_name: String,
    pub kernel_version: String,
    pub uptime_seconds: u64,
    pub cpu_model: String,
    pub cpu_usage_percent: f32,
    pub cpu_cores_usage: Vec<f32>,
    pub cpu_freq_mhz: u64,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub ram_used_percent: f32,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_percent: f32,
    pub gpu: Option<GpuInfo>,
    pub disks: Vec<DiskInfo>,
}

pub struct SystemCollector {
    sys: System,
    disks: Disks,
}

impl SystemCollector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        std::thread::sleep(std::time::Duration::from_millis(100));
        sys.refresh_cpu_all();
        let disks = Disks::new_with_refreshed_list();
        Self { sys, disks }
    }

    pub fn collect(&mut self) -> SystemMetrics {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.disks.refresh(true);

        // CPU
        let cpus = self.sys.cpus();
        let cpu_usage_percent = self.sys.global_cpu_usage();
        let cpu_cores_usage: Vec<f32> = cpus.iter().map(|c| (c.cpu_usage() * 10.0).round() / 10.0).collect();
        let cpu_model = cpus
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "AMD Ryzen 7 7800X3D".to_string());
        let cpu_freq_mhz = cpus.first().map(|c| c.frequency()).unwrap_or(0);

        // RAM & Swap
        let ram_total = self.sys.total_memory();
        let ram_used = self.sys.used_memory();
        let ram_used_percent = if ram_total > 0 {
            (ram_used as f32 / ram_total as f32) * 100.0
        } else {
            0.0
        };

        let swap_total = self.sys.total_swap();
        let swap_used = self.sys.used_swap();
        let swap_used_percent = if swap_total > 0 {
            (swap_used as f32 / swap_total as f32) * 100.0
        } else {
            0.0
        };

        // Disks
        let disks: Vec<DiskInfo> = self
            .disks
            .iter()
            .filter(|d| {
                let mp = d.mount_point().to_string_lossy();
                mp == "/" || mp.starts_with("/home") || mp.starts_with("/mnt") || mp.starts_with("/media") || mp.starts_with("/run/media") || mp.starts_with("/Jeux")
            })
            .map(|d| {
                let total = d.total_space();
                let avail = d.available_space();
                let used = total.saturating_sub(avail);
                let pct = if total > 0 {
                    (used as f32 / total as f32) * 100.0
                } else {
                    0.0
                };
                DiskInfo {
                    name: d.name().to_string_lossy().to_string(),
                    mount_point: d.mount_point().to_string_lossy().to_string(),
                    total_bytes: total,
                    available_bytes: avail,
                    used_bytes: used,
                    used_percent: (pct * 10.0).round() / 10.0,
                    file_system: d.file_system().to_string_lossy().to_string(),
                }
            })
            .collect();

        // GPU Detection
        let gpu = detect_gpu();

        SystemMetrics {
            hostname: System::host_name().unwrap_or_else(|| "chomiamos".to_string()),
            os_name: "ChomiamOS 26.05 (Gaming Edition)".to_string(),
            kernel_version: System::kernel_version().unwrap_or_else(|| "Linux".to_string()),
            uptime_seconds: System::uptime(),
            cpu_model,
            cpu_usage_percent: (cpu_usage_percent * 10.0).round() / 10.0,
            cpu_cores_usage,
            cpu_freq_mhz,
            ram_used_bytes: ram_used,
            ram_total_bytes: ram_total,
            ram_used_percent: (ram_used_percent * 10.0).round() / 10.0,
            swap_used_bytes: swap_used,
            swap_total_bytes: swap_total,
            swap_used_percent: (swap_used_percent * 10.0).round() / 10.0,
            gpu,
            disks,
        }
    }
}

fn detect_gpu() -> Option<GpuInfo> {
    // 1. Try AMD / Intel sysfs (/sys/class/drm/card*)
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("card") && !name.contains('-') {
                let dev_path = entry.path().join("device");
                if dev_path.exists() {
                    let uevent = fs::read_to_string(dev_path.join("uevent")).unwrap_or_default();
                    let is_amdgpu = uevent.contains("DRIVER=amdgpu");
                    let is_intel = uevent.contains("DRIVER=i915") || uevent.contains("DRIVER=xe");

                    if is_amdgpu {
                        let usage = fs::read_to_string(dev_path.join("gpu_busy_percent"))
                            .ok()
                            .and_then(|s| s.trim().parse::<f32>().ok());

                        let vram_used = fs::read_to_string(dev_path.join("mem_info_vram_used"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u64>().ok());

                        let vram_total = fs::read_to_string(dev_path.join("mem_info_vram_total"))
                            .ok()
                            .and_then(|s| s.trim().parse::<u64>().ok());

                        let vram_percent = match (vram_used, vram_total) {
                            (Some(u), Some(t)) if t > 0 => Some(((u as f32 / t as f32) * 100.0 * 10.0).round() / 10.0),
                            _ => None,
                        };

                        let temp = read_hwmon_temp(&dev_path.join("hwmon"));
                        let gpu_name = resolve_pci_gpu_name(&uevent, "AMD Radeon RX Gaming GPU");

                        return Some(GpuInfo {
                            name: gpu_name,
                            driver: "amdgpu (RADV/Mesa)".to_string(),
                            usage_percent: usage,
                            temp_celsius: temp,
                            vram_used_bytes: vram_used,
                            vram_total_bytes: vram_total,
                            vram_used_percent: vram_percent,
                        });
                    } else if is_intel {
                        let temp = read_hwmon_temp(&dev_path.join("hwmon"));
                        return Some(GpuInfo {
                            name: "Intel Arc / Iris Xe Graphics".to_string(),
                            driver: "i915/xe".to_string(),
                            usage_percent: None,
                            temp_celsius: temp,
                            vram_used_bytes: None,
                            vram_total_bytes: None,
                            vram_used_percent: None,
                        });
                    }
                }
            }
        }
    }

    // 2. Try NVIDIA via nvidia-smi
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() >= 5 {
                    let name = parts[0].to_string();
                    let usage = parts[1].parse::<f32>().ok();
                    let temp = parts[2].parse::<f32>().ok();
                    let mem_used = parts[3].parse::<u64>().ok().map(|mb| mb * 1024 * 1024);
                    let mem_total = parts[4].parse::<u64>().ok().map(|mb| mb * 1024 * 1024);
                    let mem_pct = match (mem_used, mem_total) {
                        (Some(u), Some(t)) if t > 0 => Some(((u as f32 / t as f32) * 100.0 * 10.0).round() / 10.0),
                        _ => None,
                    };
                    return Some(GpuInfo {
                        name,
                        driver: "NVIDIA Proprietary".to_string(),
                        usage_percent: usage,
                        temp_celsius: temp,
                        vram_used_bytes: mem_used,
                        vram_total_bytes: mem_total,
                        vram_used_percent: mem_pct,
                    });
                }
            }
        }
    }

    None
}

fn read_hwmon_temp(hwmon_dir: &Path) -> Option<f32> {
    if let Ok(entries) = fs::read_dir(hwmon_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            for temp_file in ["temp1_input", "temp2_input", "temp3_input"] {
                let t_path = path.join(temp_file);
                if let Ok(content) = fs::read_to_string(t_path) {
                    if let Ok(milli_c) = content.trim().parse::<f32>() {
                        if milli_c > 0.0 && milli_c < 120000.0 {
                            return Some((milli_c / 1000.0 * 10.0).round() / 10.0);
                        }
                    }
                }
            }
        }
    }
    None
}

fn resolve_pci_gpu_name(uevent: &str, fallback: &str) -> String {
    for line in uevent.lines() {
        if line.starts_with("PCI_ID=") {
            let id = line.trim_start_matches("PCI_ID=").to_uppercase();
            if id.contains("1002:7550") || id.contains("7550") {
                return "AMD Radeon RX 9070 / 9070 XT".to_string();
            }
            return format!("AMD Radeon (PCI ID: {})", id);
        }
    }
    fallback.to_string()
}
