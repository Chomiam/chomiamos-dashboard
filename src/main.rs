mod config;
mod generations;
mod pty;
mod system;
mod updates;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

use config::{get_vars_path, read_vars_nix, save_vars_nix, ChomiamConfig};
use generations::{list_generations, GenerationsSummary};
use pty::PtyManager;
use system::{SystemCollector, SystemMetrics};

#[tauri::command]
fn get_system_metrics(collector: State<'_, Arc<Mutex<SystemCollector>>>) -> SystemMetrics {
    let mut c = collector.lock().unwrap();
    c.collect()
}

#[tauri::command]
fn get_generations() -> Result<GenerationsSummary, String> {
    list_generations()
}

#[tauri::command]
fn get_chomiamos_config() -> Result<ChomiamConfig, String> {
    let path = get_vars_path();
    read_vars_nix(&path)
}

#[tauri::command]
fn save_chomiamos_config(config: ChomiamConfig) -> Result<(), String> {
    let path = get_vars_path();
    save_vars_nix(&path, &config)
}

#[tauri::command]
fn start_terminal_task(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    task: String,
    extra: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<(), String> {
    let cols = cols.unwrap_or(100);
    let rows = rows.unwrap_or(24);

    let (program, args): (String, Vec<String>) = match task.as_str() {
        "update-now" => (
            "bash".into(),
            vec![
                "-c".into(),
                "echo -e '\\033[1;35m🚀 Mise à jour du système ChomiamOS (Immédiate)...\\033[0m\\n' ; cd /etc/nixos && git stash && git pull --no-rebase && git stash pop ; nh os switch -u /etc/nixos".into(),
            ],
        ),
        "update-boot" => (
            "bash".into(),
            vec![
                "-c".into(),
                "echo -e '\\033[1;35m🚀 Mise à jour du système ChomiamOS (Au prochain boot)...\\033[0m\\n' ; cd /etc/nixos && git stash && git pull --no-rebase && git stash pop ; nh os boot -u /etc/nixos".into(),
            ],
        ),
        "clean-generations" => {
            let keep = extra.unwrap_or_else(|| "3".into());
            (
                "nh".into(),
                vec!["clean".into(), "all".into(), "--keep".into(), keep],
            )
        }
        "clean-all" => (
            "nh".into(),
            vec!["clean".into(), "all".into()],
        ),
        "optimise-store" => (
            "bash".into(),
            vec!["-c".into(), "echo -e '\\033[1;34m⚡ Optimisation des hardlinks du Nix Store...\\033[0m\\n' ; sudo nix-store --optimise".into()],
        ),
        "apply-config" => (
            "nh".into(),
            vec!["os".into(), "switch".into(), "/etc/nixos".into()],
        ),
        "boot-config" => (
            "nh".into(),
            vec!["os".into(), "boot".into(), "/etc/nixos".into()],
        ),
        _ => return Err(format!("Action inconnue : {}", task)),
    };

    pty.start(app, program, args, cols, rows)
}

#[tauri::command]
fn write_pty(pty: State<'_, PtyManager>, data: String) -> Result<(), String> {
    pty.write(&data)
}

#[tauri::command]
fn resize_pty(pty: State<'_, PtyManager>, cols: u16, rows: u16) -> Result<(), String> {
    pty.resize(cols, rows)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KeyboardLockState {
    pub caps_lock: bool,
    pub num_lock: bool,
}

#[tauri::command]
fn get_keyboard_lock_state() -> KeyboardLockState {
    let mut caps_lock = false;
    let mut num_lock = false;

    if let Ok(entries) = std::fs::read_dir("/sys/class/leds") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with("::capslock") {
                if let Ok(content) = std::fs::read_to_string(entry.path().join("brightness")) {
                    if content.trim() != "0" {
                        caps_lock = true;
                    }
                }
            } else if name.ends_with("::numlock") {
                if let Ok(content) = std::fs::read_to_string(entry.path().join("brightness")) {
                    if content.trim() != "0" {
                        num_lock = true;
                    }
                }
            }
        }
    }

    KeyboardLockState {
        caps_lock,
        num_lock,
    }
}

fn main() {
    let collector = Arc::new(Mutex::new(SystemCollector::new()));
    let pty_manager = PtyManager::new();

    tauri::Builder::default()
        .manage(collector)
        .manage(pty_manager)
        .invoke_handler(tauri::generate_handler![
            get_system_metrics,
            get_generations,
            get_chomiamos_config,
            save_chomiamos_config,
            start_terminal_task,
            write_pty,
            resize_pty,
            get_keyboard_lock_state
        ])
        .run(tauri::generate_context!())
        .expect("Erreur lors de l'exécution de l'application ChomiamOS Dashboard");
}
