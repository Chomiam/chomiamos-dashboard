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

fn get_sync_script(mode: &str) -> String {
    let deploy_cmd = match mode {
        "boot" => "nh os boot -u /etc/nixos",
        _ => "nh os switch -u /etc/nixos",
    };
    let mode_desc = match mode {
        "boot" => "au prochain redémarrage (nh os boot -u)",
        _ => "immédiate (nh os switch -u)",
    };

    format!(
r#"echo -e '\033[1;35m🐙 Synchronisation de la configuration NixOS depuis GitHub ({desc})...\033[0m\n'
cd /etc/nixos || exit 1

# 1. Sauvegarde inviolable et permanente de vars.nix et hardware-configuration.nix
git config merge.ours.driver true || true
if [ -f /etc/nixos/vars.nix ]; then
  echo -e '\033[1;34m🛡️ Sauvegarde et protection de votre configuration locale et compte utilisateur...\033[0m'
  cp -f /etc/nixos/vars.nix /etc/nixos/.vars.nix.backup
fi
if [ -f /etc/nixos/hosts/desktop/hardware-configuration.nix ]; then
  cp -f /etc/nixos/hosts/desktop/hardware-configuration.nix /etc/nixos/.hardware-configuration.nix.backup
fi

# 2. Sauvegarde dans le stash git
STASH_OUT=$(git stash 2>&1)
echo "$STASH_OUT"
DID_STASH=0
if [[ "$STASH_OUT" != *"No local changes to save"* ]]; then
  DID_STASH=1
fi

# 3. Pull depuis GitHub
echo -e '\n\033[1;34m⬇️ Récupération des dernières modifications depuis GitHub (git pull --no-rebase)...\033[0m'
if ! git pull --no-rebase origin main; then
  echo -e '\n\033[1;33m⚠️ Conflit détecté lors du pull...\033[0m'

  # Si flake.lock a un conflit, écraser depuis origin/main
  if git status --porcelain | grep -q "flake\.lock"; then
    echo -e '\033[1;33m🔧 Résolution automatique du conflit flake.lock depuis origin/main...\033[0m'
    git checkout origin/main -- flake.lock
    git add flake.lock
  fi

  # Si vars.nix a un conflit lors du pull, NE JAMAIS PRENDRE origin/main ! Garder ou restaurer la version locale !
  if git status --porcelain | grep -q "vars\.nix"; then
    echo -e '\033[1;33m🛡️ Préservation de votre fichier vars.nix personnel...\033[0m'
    if [ -f /etc/nixos/.vars.nix.backup ]; then
      cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
    fi
    git add vars.nix
  fi

  git -c user.name="ChomiamOS" -c user.email="root@chomiamos" commit -m "fix: resolve sync conflict" || true
fi

# 4. Restauration du stash
if [ $DID_STASH -eq 1 ]; then
  echo -e '\n\033[1;34m📤 Restauration des modifications locales (git stash pop)...\033[0m'
  if ! git stash pop; then
    echo -e '\033[1;33m⚠️ Conflit lors de la réapplication du stash...\033[0m'
    if git status --porcelain | grep -E "flake\.lock"; then
      git checkout origin/main -- flake.lock
      git add flake.lock
    fi
    if git status --porcelain | grep -E "vars\.nix"; then
      if [ -f /etc/nixos/.vars.nix.backup ]; then
        cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
      fi
      git add vars.nix
    fi
    git -c user.name="ChomiamOS" -c user.email="root@chomiamos" commit -m "fix: resolve sync conflict" || true
    git stash drop || true
  fi
fi

# 5. GARANTIE ABSOLUE : Vérifier et protéger le compte utilisateur et le matériel
if [ -f /etc/nixos/.vars.nix.backup ]; then
  BACKUP_USER=$(grep -E 'username\s*=' /etc/nixos/.vars.nix.backup | head -n 1)
  CURRENT_USER=$(grep -E 'username\s*=' /etc/nixos/vars.nix | head -n 1)
  if [ -n "$BACKUP_USER" ] && [ "$BACKUP_USER" != "$CURRENT_USER" ]; then
    echo -e '\033[1;33m⚠️ Détection d'\''un écrasement de votre compte utilisateur ! Restauration immédiate...\033[0m'
    cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
  fi
fi

# Détection de sécurité avancée : vérifier avec l'UID 1000 du système local
REAL_USER=$(awk -F: '$3 == 1000 {{print $1}}' /etc/passwd 2>/dev/null || true)
if [ -n "$REAL_USER" ] && [ "$REAL_USER" != "chomiam" ]; then
  CURRENT_USER_NAME=$(grep -oP 'username\s*=\s*"\K[^"]+' /etc/nixos/vars.nix 2>/dev/null || true)
  if [ "$CURRENT_USER_NAME" = "chomiam" ]; then
    echo -e '\033[1;31m🛡️ ALERTE DE SÉCURITÉ : Le compte utilisateur a été écrasé par chomiam ! Correction automatique pour '"$REAL_USER"'...\033[0m'
    if [ -f /etc/nixos/.vars.nix.backup ]; then
      cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
    else
      sed -i "s/username = \"chomiam\"/username = \"$REAL_USER\"/g" /etc/nixos/vars.nix
      sed -i "s/homeDirectory = \"\/home\/chomiam\"/homeDirectory = \"\/home\/$REAL_USER\"/g" /etc/nixos/vars.nix
    fi
  fi
fi

# Restauration automatique de hardware-configuration.nix si altéré
if [ -f /etc/nixos/.hardware-configuration.nix.backup ]; then
  if grep -qE '^(<{{7}}|=<{{7}}|>{{7}})' /etc/nixos/hosts/desktop/hardware-configuration.nix 2>/dev/null; then
    echo -e '\033[1;33m🛡️ Restauration de hardware-configuration.nix depuis la sauvegarde...\033[0m'
    cp -f /etc/nixos/.hardware-configuration.nix.backup /etc/nixos/hosts/desktop/hardware-configuration.nix
  fi
fi

# Vérification syntaxe Git dans vars.nix
if grep -qE '^(<{{7}}|=<{{7}}|>{{7}})' /etc/nixos/vars.nix 2>/dev/null; then
  echo -e '\033[1;31m❌ Marqueurs de conflit Git détectés dans vars.nix ! Restauration d'urgence...\033[0m'
  if [ -f /etc/nixos/.vars.nix.backup ]; then
    cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
  fi
fi

echo -e '\n\033[1;32m🚀 Déploiement du système avec {deploy_cmd}...\033[0m\n'
{deploy_cmd}
"#,
        desc = mode_desc,
        deploy_cmd = deploy_cmd
    )
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
        "sync-github" | "update-now" => (
            "bash".into(),
            vec!["-c".into(), get_sync_script("switch")],
        ),
        "boot-sync-github" | "update-boot" => (
            "bash".into(),
            vec!["-c".into(), get_sync_script("boot")],
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
