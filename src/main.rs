mod config;
mod generations;
mod system;
mod updates;

slint::include_modules!();

use regex::Regex;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::config::{get_vars_path, read_vars_nix, save_vars_nix};
use crate::generations::list_generations;
use crate::system::SystemCollector;

static ACTIVE_STDIN: Mutex<Option<ChildStdin>> = Mutex::new(None);

fn get_sudo_wrapper_path() -> String {
    let local = std::path::Path::new("/home/chomiam/Projects/dashboard-chomiamos/scripts/sudo-stdin");
    if local.exists() {
        return local.to_string_lossy().to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(prefix) = exe.parent().and_then(|p| p.parent()) {
            let shared = prefix.join("share/chomiamos-dashboard/scripts/sudo-stdin");
            if shared.exists() {
                return shared.to_string_lossy().to_string();
            }
        }
    }
    let fallback = std::path::Path::new("/tmp/chomiamos-sudo-stdin");
    if !fallback.exists() {
        let _ = std::fs::write(fallback, "#!/usr/bin/env bash\nexec /run/wrappers/bin/sudo -S -p \"[sudo] Mot de passe : \" \"$@\"\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(fallback, std::fs::Permissions::from_mode(0o755));
        }
    }
    fallback.to_string_lossy().to_string()
}

fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    format!("{:.1} Go", bytes as f64 / GIB)
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let mins = (seconds % 3600) / 60;
    if days > 0 {
        format!("{}j {}h {}m", days, hours, mins)
    } else {
        format!("{}h {}m", hours, mins)
    }
}

fn run_command_in_terminal(
    ui_handle: slint::Weak<AppWindow>,
    title: impl Into<String>,
    program: impl Into<String>,
    args: Vec<String>,
) {
    let title = title.into();
    let program = program.into();
    if let Some(ui) = ui_handle.upgrade() {
        let bridge = ui.global::<DashboardBridge>();
        bridge.set_terminal_open(true);
        bridge.set_operation_running(true);
        bridge.set_password_required(false);
        bridge.set_terminal_title(title.into());
        bridge.set_terminal_output(format!("🚀 Commande : {} {}\n\n", program, args.join(" ")).into());
    }

    thread::spawn(move || {
        let ansi_regex = Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").unwrap();

        let mut child = match Command::new(program)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("\n❌ Erreur de lancement : {}\n", e);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        let bridge = ui.global::<DashboardBridge>();
                        let cur = bridge.get_terminal_output().to_string();
                        bridge.set_terminal_output((cur + &err_msg).into());
                        bridge.set_operation_running(false);
                    }
                });
                return;
            }
        };

        if let Some(stdin) = child.stdin.take() {
            let mut lock = ACTIVE_STDIN.lock().unwrap();
            *lock = Some(stdin);
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let ui_handle_out = ui_handle.clone();
        let ansi_out = ansi_regex.clone();
        let out_thread = thread::spawn(move || {
            if let Some(stdout) = stdout {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    let clean_line = ansi_out.replace_all(&line, "").to_string();
                    let is_pw_prompt = clean_line.contains("[sudo]")
                        || clean_line.to_lowercase().contains("mot de passe")
                        || clean_line.to_lowercase().contains("password");

                    let ui = ui_handle_out.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui.upgrade() {
                            let bridge = ui.global::<DashboardBridge>();
                            let cur = bridge.get_terminal_output().to_string();
                            bridge.set_terminal_output((cur + &clean_line + "\n").into());
                            if is_pw_prompt {
                                bridge.set_password_required(true);
                            }
                        }
                    });
                }
            }
        });

        let ui_handle_err = ui_handle.clone();
        let ansi_err = ansi_regex;
        let err_thread = thread::spawn(move || {
            if let Some(stderr) = stderr {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    let clean_line = ansi_err.replace_all(&line, "").to_string();
                    let is_pw_prompt = clean_line.contains("[sudo]")
                        || clean_line.to_lowercase().contains("mot de passe")
                        || clean_line.to_lowercase().contains("password");

                    let ui = ui_handle_err.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui.upgrade() {
                            let bridge = ui.global::<DashboardBridge>();
                            let cur = bridge.get_terminal_output().to_string();
                            bridge.set_terminal_output((cur + &clean_line + "\n").into());
                            if is_pw_prompt {
                                bridge.set_password_required(true);
                            }
                        }
                    });
                }
            }
        });

        let _ = out_thread.join();
        let _ = err_thread.join();
        let status = child.wait();

        {
            let mut lock = ACTIVE_STDIN.lock().unwrap();
            *lock = None;
        }

        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_handle.upgrade() {
                let bridge = ui.global::<DashboardBridge>();
                let cur = bridge.get_terminal_output().to_string();
                let conclusion = match status {
                    Ok(s) if s.success() => "\n✔ Opération terminée avec succès !\n".to_string(),
                    Ok(s) => format!("\n❌ L'opération a échoué avec le code {}\n", s.code().unwrap_or(1)),
                    Err(e) => format!("\n❌ Erreur : {}\n", e),
                };
                bridge.set_terminal_output((cur + &conclusion).into());
                bridge.set_operation_running(false);
                bridge.set_password_required(false);
            }
        });
    });
}

fn load_generations(ui_handle: slint::Weak<AppWindow>) {
    thread::spawn(move || {
        if let Ok(summary) = list_generations() {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_handle.upgrade() {
                    let bridge = ui.global::<DashboardBridge>();
                    bridge.set_generations_count(summary.count as i32);
                    bridge.set_active_generation(summary.active_generation.unwrap_or(0) as i32);
                    bridge.set_store_size(summary.store_size.into());

                    let model = Rc::new(VecModel::default());
                    for g in summary.generations {
                        model.push(GenerationItem {
                            id: g.id as i32,
                            date: g.date.into(),
                            kernel: g.kernel.into(),
                            version: g.nixos_version.into(),
                            is_current: g.current,
                        });
                    }
                    bridge.set_generations(ModelRc::from(model));
                }
            });
        }
    });
}

fn load_config(ui: &AppWindow) {
    if let Ok(cfg) = read_vars_nix(&get_vars_path()) {
        let bridge = ui.global::<DashboardBridge>();
        bridge.set_browser(cfg.browser.into());
        bridge.set_discord_client(cfg.discord_client.into());

        // Gaming
        bridge.set_game_steam(cfg.gaming.steam);
        bridge.set_game_lutris(cfg.gaming.lutris);
        bridge.set_game_heroic(cfg.gaming.heroic);
        bridge.set_game_faugus(cfg.gaming.faugus);
        bridge.set_game_decky(cfg.gaming.decky_loader);
        bridge.set_game_geforce(cfg.gaming.geforce_now);
        bridge.set_game_wheels(cfg.gaming.steering_wheels);

        // Emulation
        bridge.set_emu_enable(cfg.emulation.enable);
        bridge.set_emu_frontend(cfg.emulation.frontend.into());
        bridge.set_emu_retroarch(cfg.emulation.retroarch);
        bridge.set_emu_eden(cfg.emulation.eden);
        bridge.set_emu_dolphin(cfg.emulation.dolphin);
        bridge.set_emu_pcsx2(cfg.emulation.pcsx2);
        bridge.set_emu_ppsspp(cfg.emulation.ppsspp);
        bridge.set_emu_melonds(cfg.emulation.melonds);
        bridge.set_emu_azahar(cfg.emulation.azahar);
        bridge.set_emu_mgba(cfg.emulation.mgba);
        bridge.set_emu_rpcs3(cfg.emulation.rpcs3);

        // Media
        bridge.set_media_stremio(cfg.media.stremio);
        bridge.set_media_vlc(cfg.media.vlc);
        bridge.set_media_mpv(cfg.media.mpv);
        bridge.set_media_tailscale(cfg.media.tailscale);
        bridge.set_media_localsend(cfg.media.localsend);
        bridge.set_media_motrix(cfg.media.motrix);

        // Creation
        bridge.set_create_davinci(cfg.creation.davinci_resolve.into());
        bridge.set_create_blender(cfg.creation.blender);
        bridge.set_create_godot(cfg.creation.godot);
        bridge.set_create_kdenlive(cfg.creation.kdenlive);
        bridge.set_create_obs(cfg.creation.obs_studio);
        bridge.set_create_antigravity(cfg.creation.antigravity);
        bridge.set_create_pear(cfg.creation.pear_desktop);
        bridge.set_create_kvm(cfg.creation.virtualisation);
        bridge.set_create_ai(cfg.creation.ai_suite);
    }
}

fn save_current_config(ui: &AppWindow) -> Result<(), String> {
    let bridge = ui.global::<DashboardBridge>();
    let mut cfg = read_vars_nix(&get_vars_path()).unwrap_or_default();

    cfg.browser = bridge.get_browser().to_string();
    cfg.discord_client = bridge.get_discord_client().to_string();

    cfg.gaming.steam = bridge.get_game_steam();
    cfg.gaming.lutris = bridge.get_game_lutris();
    cfg.gaming.heroic = bridge.get_game_heroic();
    cfg.gaming.faugus = bridge.get_game_faugus();
    cfg.gaming.decky_loader = bridge.get_game_decky();
    cfg.gaming.geforce_now = bridge.get_game_geforce();
    cfg.gaming.steering_wheels = bridge.get_game_wheels();

    cfg.emulation.enable = bridge.get_emu_enable();
    cfg.emulation.frontend = bridge.get_emu_frontend().to_string();
    cfg.emulation.retroarch = bridge.get_emu_retroarch();
    cfg.emulation.eden = bridge.get_emu_eden();
    cfg.emulation.dolphin = bridge.get_emu_dolphin();
    cfg.emulation.pcsx2 = bridge.get_emu_pcsx2();
    cfg.emulation.ppsspp = bridge.get_emu_ppsspp();
    cfg.emulation.melonds = bridge.get_emu_melonds();
    cfg.emulation.azahar = bridge.get_emu_azahar();
    cfg.emulation.mgba = bridge.get_emu_mgba();
    cfg.emulation.rpcs3 = bridge.get_emu_rpcs3();

    cfg.media.stremio = bridge.get_media_stremio();
    cfg.media.vlc = bridge.get_media_vlc();
    cfg.media.mpv = bridge.get_media_mpv();
    cfg.media.tailscale = bridge.get_media_tailscale();
    cfg.media.localsend = bridge.get_media_localsend();
    cfg.media.motrix = bridge.get_media_motrix();

    cfg.creation.davinci_resolve = bridge.get_create_davinci().to_string();
    cfg.creation.blender = bridge.get_create_blender();
    cfg.creation.godot = bridge.get_create_godot();
    cfg.creation.kdenlive = bridge.get_create_kdenlive();
    cfg.creation.obs_studio = bridge.get_create_obs();
    cfg.creation.antigravity = bridge.get_create_antigravity();
    cfg.creation.pear_desktop = bridge.get_create_pear();
    cfg.creation.virtualisation = bridge.get_create_kvm();
    cfg.creation.ai_suite = bridge.get_create_ai();

    save_vars_nix(&get_vars_path(), &cfg)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════╗");
    println!("║       🎮 ChomiamOS Dashboard (Slint GUI)          ║");
    println!("║       Catppuccin Mocha • Native Linux / Wayland   ║");
    println!("╚═══════════════════════════════════════════════════╝");

    let ui = AppWindow::new()?;

    // Initial config and generations
    load_config(&ui);
    load_generations(ui.as_weak());

    // Connect callbacks
    let ui_weak = ui.as_weak();
    let bridge = ui.global::<DashboardBridge>();

    // Callback: send_terminal_input (for interactive input and sudo password)
    let weak = ui_weak.clone();
    bridge.on_send_terminal_input(move |text| {
        let mut lock = ACTIVE_STDIN.lock().unwrap();
        if let Some(stdin) = lock.as_mut() {
            let _ = stdin.write_all(text.as_bytes());
            let _ = stdin.write_all(b"\n");
            let _ = stdin.flush();
        }
        if let Some(ui) = weak.upgrade() {
            let bridge = ui.global::<DashboardBridge>();
            let cur = bridge.get_terminal_output().to_string();
            let display_msg = if bridge.get_password_required() {
                "********\n".to_string()
            } else {
                format!("{}\n", text)
            };
            bridge.set_terminal_output((cur + &display_msg).into());
            bridge.set_password_required(false);
        }
    });

    // Callback: update_now
    let weak = ui_weak.clone();
    bridge.on_update_now(move || {
        run_command_in_terminal(
            weak.clone(),
            "Mise à jour du système (Immédiate)",
            "nh",
            vec!["os".into(), "switch".into(), "-u".into(), "-e".into(), get_sudo_wrapper_path()],
        );
    });

    // Callback: update_on_boot
    let weak = ui_weak.clone();
    bridge.on_update_on_boot(move || {
        run_command_in_terminal(
            weak.clone(),
            "Mise à jour du système (Au prochain boot)",
            "nh",
            vec!["os".into(), "boot".into(), "-u".into(), "-e".into(), get_sudo_wrapper_path()],
        );
    });

    // Callback: clean_generations
    let weak = ui_weak.clone();
    bridge.on_clean_generations(move |keep| {
        let keep_str = keep.to_string();
        let weak_for_refresh = weak.clone();
        run_command_in_terminal(
            weak.clone(),
            "Nettoyage des générations NixOS",
            "nh",
            vec!["clean".into(), "all".into(), "--keep".into(), keep_str, "-e".into(), get_sudo_wrapper_path()],
        );
        load_generations(weak_for_refresh);
    });

    // Callback: clean_all
    let weak = ui_weak.clone();
    bridge.on_clean_all(move || {
        let weak_for_refresh = weak.clone();
        run_command_in_terminal(
            weak.clone(),
            "Nettoyage complet du Garbage Collector",
            "nh",
            vec!["clean".into(), "all".into(), "-e".into(), get_sudo_wrapper_path()],
        );
        load_generations(weak_for_refresh);
    });

    // Callback: optimise_store
    let weak = ui_weak.clone();
    bridge.on_optimise_store(move || {
        let weak_for_refresh = weak.clone();
        run_command_in_terminal(
            weak.clone(),
            "Optimisation des hardlinks du Nix Store",
            get_sudo_wrapper_path(),
            vec!["nix-store".into(), "--optimise".into()],
        );
        load_generations(weak_for_refresh);
    });

    // Callback: save_and_apply
    let weak = ui_weak.clone();
    bridge.on_save_and_apply(move || {
        if let Some(ui) = weak.upgrade() {
            if let Err(e) = save_current_config(&ui) {
                eprintln!("Erreur lors de l'enregistrement de vars.nix : {}", e);
                return;
            }
        }
        run_command_in_terminal(
            weak.clone(),
            "Application de la configuration ChomiamOS",
            "nh",
            vec!["os".into(), "switch".into(), "-e".into(), get_sudo_wrapper_path()],
        );
    });

    // Callback: close_terminal
    let weak = ui_weak.clone();
    bridge.on_close_terminal(move || {
        if let Some(ui) = weak.upgrade() {
            ui.global::<DashboardBridge>().set_terminal_open(false);
        }
    });

    // Telemetry update timer
    let collector = Arc::new(Mutex::new(SystemCollector::new()));
    let timer = slint::Timer::default();
    let weak = ui_weak.clone();

    // Initial telemetry collection
    {
        let mut col = collector.lock().unwrap();
        let metrics = col.collect();
        let b = ui.global::<DashboardBridge>();
        b.set_hostname(metrics.hostname.into());
        b.set_os_name(metrics.os_name.into());
        b.set_kernel_version(metrics.kernel_version.into());
        b.set_cpu_model(metrics.cpu_model.into());
    }

    timer.start(slint::TimerMode::Repeated, Duration::from_millis(1500), move || {
        if let Some(ui) = weak.upgrade() {
            let mut col = collector.lock().unwrap();
            let metrics = col.collect();
            let b = ui.global::<DashboardBridge>();

            b.set_uptime(format_uptime(metrics.uptime_seconds).into());
            b.set_cpu_usage(metrics.cpu_usage_percent as f32);
            b.set_cpu_freq(format!("{} MHz", metrics.cpu_freq_mhz).into());

            // 16 cores model
            let cores_model = Rc::new(VecModel::default());
            for &c in &metrics.cpu_cores_usage {
                cores_model.push(c as f32);
            }
            b.set_cpu_cores(ModelRc::from(cores_model));

            // GPU
            if let Some(gpu) = metrics.gpu {
                b.set_gpu_name(gpu.name.into());
                b.set_gpu_driver(gpu.driver.into());
                b.set_gpu_usage(gpu.usage_percent.unwrap_or(0.0));
                b.set_gpu_temp(gpu.temp_celsius.unwrap_or(0.0));
                b.set_vram_used(format_bytes(gpu.vram_used_bytes.unwrap_or(0)).into());
                b.set_vram_total(format_bytes(gpu.vram_total_bytes.unwrap_or(0)).into());
                b.set_vram_percent(gpu.vram_used_percent.unwrap_or(0.0));
            }

            // RAM & Swap
            b.set_ram_used(format_bytes(metrics.ram_used_bytes).into());
            b.set_ram_total(format_bytes(metrics.ram_total_bytes).into());
            b.set_ram_percent(metrics.ram_used_percent as f32);
            b.set_swap_used(format_bytes(metrics.swap_used_bytes).into());
            b.set_swap_total(format_bytes(metrics.swap_total_bytes).into());
            b.set_swap_percent(metrics.swap_used_percent as f32);

            // Disks
            for disk in &metrics.disks {
                if disk.mount_point == "/" {
                    b.set_disk_root_used(format_bytes(disk.used_bytes).into());
                    b.set_disk_root_total(format_bytes(disk.total_bytes).into());
                    b.set_disk_root_percent(disk.used_percent as f32);
                } else if disk.mount_point == "/mnt/Games" {
                    b.set_disk_games_used(format_bytes(disk.used_bytes).into());
                    b.set_disk_games_total(format_bytes(disk.total_bytes).into());
                    b.set_disk_games_percent(disk.used_percent as f32);
                }
            }
        }
    });

    ui.run()?;
    Ok(())
}
