mod config;
mod firewall;
mod disks;
mod generations;
mod packages;
mod pty;
mod system;
mod updates;

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

use config::{get_vars_path, read_vars_nix, save_vars_nix, ChomiamConfig};
use generations::{list_generations, delete_generations as do_delete_generations, switch_to_generation as do_switch_to_generation, GenerationsSummary};
use packages::{
    add_custom_package as do_add_custom_package,
    get_packages_state as do_get_packages_state,
    remove_custom_package as do_remove_custom_package,
    search_nixpkgs as do_search_nixpkgs,
    PackageEntry, PackagesState,
};
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
fn delete_nix_generations(ids: Vec<u32>) -> Result<String, String> {
    do_delete_generations(ids)
}

#[tauri::command]
fn switch_nix_generation(id: u32) -> Result<String, String> {
    do_switch_to_generation(id)
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
async fn search_nix_packages(query: String) -> Result<Vec<PackageEntry>, String> {
    do_search_nixpkgs(query).await
}

#[tauri::command]
async fn get_custom_packages() -> Result<PackagesState, String> {
    do_get_packages_state().await
}

#[tauri::command]
fn add_custom_package(name: String, channel: String) -> Result<(), String> {
    do_add_custom_package(name, channel)
}

#[tauri::command]
fn remove_custom_package(name: String, channel: Option<String>) -> Result<(), String> {
    do_remove_custom_package(name, channel)
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
r#"echo -e "\033[1;35m🐙 Synchronisation de la configuration NixOS depuis GitHub ({desc})...\033[0m\n"
cd /etc/nixos || exit 1

# 0. Automatisation totale : interdire tout éditeur interactif (nano, vim, etc.)
export GIT_MERGE_AUTOEDIT=no
export GIT_EDITOR=true
export EDITOR=true
export VISUAL=true

# 1. Sauvegarde inviolable et permanente de vars.nix et hardware-configuration.nix
git config merge.ours.driver true || true
if [ -f /etc/nixos/vars.nix ]; then
  echo -e "\033[1;34m🛡️ Sauvegarde et protection de votre configuration locale et compte utilisateur...\033[0m"
  cp -f /etc/nixos/vars.nix /etc/nixos/.vars.nix.backup
fi
if [ -f /etc/nixos/hosts/desktop/hardware-configuration.nix ]; then
  cp -f /etc/nixos/hosts/desktop/hardware-configuration.nix /etc/nixos/.hardware-configuration.nix.backup
fi
if [ -f /etc/nixos/hosts/desktop/mount.nix ]; then
  cp -f /etc/nixos/hosts/desktop/mount.nix /etc/nixos/.mount.nix.backup
fi
if [ -f /etc/nixos/modules/core/firewall.nix ]; then
  cp -f /etc/nixos/modules/core/firewall.nix /etc/nixos/modules/core/.firewall.nix.backup
fi
if [ -f /etc/nixos/secrets/github-token.conf ]; then
  cp -f /etc/nixos/secrets/github-token.conf /etc/nixos/secrets/.github-token.conf.backup
fi

# 2. Sauvegarde dans le stash git (indépendant de la langue avec --porcelain)
DID_STASH=0
if [ -n "$(git status --porcelain)" ]; then
  echo -e "\033[1;34m📦 Sauvegarde des modifications locales (git stash)...\033[0m"
  git stash
  DID_STASH=1
else
  echo -e "\033[1;34mℹ️ Aucune modification locale en attente.\033[0m"
fi

# 3. Pull depuis GitHub
echo -e "\n\033[1;34m⬇️ Récupération des dernières modifications depuis GitHub (git pull --no-rebase)...\033[0m"
if ! git pull --no-rebase --no-edit origin main; then
  echo -e "\n\033[1;33m⚠️ Conflit détecté lors du pull...\033[0m"

  # Si flake.lock a un conflit, écraser depuis origin/main
  if git status --porcelain | grep -q "flake\.lock"; then
    echo -e "\033[1;33m🔧 Résolution automatique du conflit flake.lock depuis origin/main...\033[0m"
    git checkout origin/main -- flake.lock
    git add flake.lock
  fi

  # Si vars.nix a un conflit lors du pull, NE JAMAIS PRENDRE origin/main ! Garder ou restaurer la version locale !
  if git status --porcelain | grep -q "vars\.nix"; then
    echo -e "\033[1;33m🛡️ Préservation de votre fichier vars.nix personnel...\033[0m"
    if [ -f /etc/nixos/.vars.nix.backup ]; then
      cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
    fi
    git add vars.nix
  fi

  # Si mount.nix a un conflit lors du pull, préserver les disques locaux
  if git status --porcelain | grep -q "mount\.nix"; then
    echo -e "\033[1;33m🛡️ Préservation de vos montages de disques personnels (mount.nix)...\033[0m"
    if [ -f /etc/nixos/.mount.nix.backup ]; then
      cp -f /etc/nixos/.mount.nix.backup /etc/nixos/hosts/desktop/mount.nix
    fi
    git add hosts/desktop/mount.nix
  fi

  # Si firewall.nix a un conflit lors du pull, préserver les règles du pare-feu
  if git status --porcelain | grep -q "firewall\.nix"; then
    echo -e "\033[1;33m🛡️ Préservation de vos règles de pare-feu personnelles (firewall.nix)...\033[0m"
    if [ -f /etc/nixos/modules/core/.firewall.nix.backup ]; then
      cp -f /etc/nixos/modules/core/.firewall.nix.backup /etc/nixos/modules/core/firewall.nix
    fi
    git add modules/core/firewall.nix
  fi

  git -c user.name="ChomiamOS" -c user.email="root@chomiamos" commit -m "fix: resolve sync conflict" --no-edit || true
fi

# 4. Restauration du stash uniquement si créé
if [ "$DID_STASH" = "1" ]; then
  echo -e "\n\033[1;34m📤 Restauration des modifications locales (git stash pop)...\033[0m"
  if ! git stash pop; then
    echo -e "\033[1;33m⚠️ Conflit lors de la réapplication du stash...\033[0m"
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
    if git status --porcelain | grep -E "mount\.nix"; then
      if [ -f /etc/nixos/.mount.nix.backup ]; then
        cp -f /etc/nixos/.mount.nix.backup /etc/nixos/hosts/desktop/mount.nix
      fi
      git add hosts/desktop/mount.nix
    fi
    if git status --porcelain | grep -E "firewall\.nix"; then
      if [ -f /etc/nixos/modules/core/.firewall.nix.backup ]; then
        cp -f /etc/nixos/modules/core/.firewall.nix.backup /etc/nixos/modules/core/firewall.nix
      fi
      git add modules/core/firewall.nix
    fi
    git -c user.name="ChomiamOS" -c user.email="root@chomiamos" commit -m "fix: resolve sync conflict" --no-edit || true
    git stash drop || true
  fi
fi

# 5. GARANTIE ABSOLUE : Préservation intégrale de vos paramètres personnels (vars.nix)
if [ -f /etc/nixos/.vars.nix.backup ]; then
  echo -e "\033[1;34m🛡️ Préservation de vos paramètres locaux et choix de bureau (vars.nix)...\033[0m"
  cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
fi
if [ -f /etc/nixos/modules/core/.firewall.nix.backup ]; then
  echo -e "\033[1;34m🛡️ Préservation de vos règles de pare-feu personnelles (firewall.nix)...\033[0m"
  cp -f /etc/nixos/modules/core/.firewall.nix.backup /etc/nixos/modules/core/firewall.nix
fi

# Détection de sécurité avancée : vérifier avec l'UID 1000 du système local
REAL_USER=$(awk -F: '$3 == 1000 {{print $1}}' /etc/passwd 2>/dev/null || true)
if [ -n "$REAL_USER" ] && [ "$REAL_USER" != "chomiam" ]; then
  CURRENT_USER_NAME=$(grep -oP 'username\s*=\s*"\K[^"]+' /etc/nixos/vars.nix 2>/dev/null || true)
  if [ "$CURRENT_USER_NAME" = "chomiam" ]; then
    echo -e "\033[1;31m🛡️ ALERTE DE SÉCURITÉ : Le compte utilisateur a été écrasé par chomiam ! Correction automatique pour $REAL_USER...\033[0m"
    if [ -f /etc/nixos/.vars.nix.backup ]; then
      cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
    else
      sed -i "s/username = \"chomiam\"/username = \"$REAL_USER\"/g" /etc/nixos/vars.nix
      sed -i "s/homeDirectory = \"/home/chomiam\"/homeDirectory = \"/home/$REAL_USER\"/g" /etc/nixos/vars.nix
    fi
  fi
fi

# 6. FUSION INTELLIGENTE : injecter les nouvelles variables de vars-defaults.nix sans toucher à vos réglages
echo -e "\n\033[1;35m✨ Synchronisation & fusion des variables système (vars-defaults.nix ➔ vars.nix)...\033[0m"
if [ -f /etc/nixos/scripts/merge-vars.py ]; then
  python3 /etc/nixos/scripts/merge-vars.py /etc/nixos/vars-defaults.nix /etc/nixos/vars.nix
else
  echo -e "\033[1;33mℹ️ Script merge-vars.py introuvable, fusion différée.\033[0m"
fi
git add vars.nix 2>/dev/null || true

# Restauration automatique de hardware-configuration.nix si altéré
if [ -f /etc/nixos/.hardware-configuration.nix.backup ]; then
  if grep -qE '^([<]{{7}}|=<{{7}}|[>]{{7}})' /etc/nixos/hosts/desktop/hardware-configuration.nix 2>/dev/null; then
    echo -e "\033[1;33m🛡️ Restauration de hardware-configuration.nix depuis la sauvegarde...\033[0m"
    cp -f /etc/nixos/.hardware-configuration.nix.backup /etc/nixos/hosts/desktop/hardware-configuration.nix
  fi
fi

# Restauration automatique du token GitHub si altéré
if [ -f /etc/nixos/secrets/.github-token.conf.backup ] && [ ! -s /etc/nixos/secrets/github-token.conf ]; then
  cp -f /etc/nixos/secrets/.github-token.conf.backup /etc/nixos/secrets/github-token.conf
fi

# Restauration automatique de mount.nix si altéré
if [ -f /etc/nixos/.mount.nix.backup ]; then
  if grep -qE '^([<]{{7}}|=<{{7}}|[>]{{7}})' /etc/nixos/hosts/desktop/mount.nix 2>/dev/null; then
    echo -e "\033[1;33m🛡️ Restauration de mount.nix depuis la sauvegarde...\033[0m"
    cp -f /etc/nixos/.mount.nix.backup /etc/nixos/hosts/desktop/mount.nix
  fi
fi

# Vérification syntaxe Git dans vars.nix
if grep -qE '^([<]{{7}}|=<{{7}}|[>]{{7}})' /etc/nixos/vars.nix 2>/dev/null; then
  echo -e "\033[1;31m❌ Marqueurs de conflit Git détectés dans vars.nix ! Restauration d'urgence...\033[0m"
  if [ -f /etc/nixos/.vars.nix.backup ]; then
    cp -f /etc/nixos/.vars.nix.backup /etc/nixos/vars.nix
  fi
fi

echo -e "\n\033[1;32m🚀 Déploiement du système avec {deploy_cmd}...\033[0m\n"
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
        "sync-github" => (
            "bash".into(),
            vec!["-c".into(), get_sync_script("switch")],
        ),
        "update-now" | "switch-update" => (
            "nh".into(),
            vec!["os".into(), "switch".into(), "-u".into(), "/etc/nixos".into()],
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
        "delete-generations" => {
            let ids_raw = extra.unwrap_or_default();
            let safe_ids: Vec<String> = ids_raw
                .split_whitespace()
                .filter_map(|s| s.parse::<u32>().ok().map(|n| n.to_string()))
                .collect();
            if safe_ids.is_empty() {
                return Err("Aucune génération valide spécifiée pour la suppression.".into());
            }
            let ids_str = safe_ids.join(" ");
            (
                "bash".into(),
                vec![
                    "-c".into(),
                    format!(
                        r#"echo -e "\033[1;35m🗑️ Suppression des générations NixOS sélectionnées ({ids_str})...\033[0m\n" ; sudo nix-env --profile /nix/var/nix/profiles/system --delete-generations {ids_str} && echo -e "\n\033[1;34m🔄 Actualisation du menu de démarrage (bootloader)...\033[0m\n" && sudo /nix/var/nix/profiles/system/bin/switch-to-configuration boot && echo -e "\n\033[1;32m✅ Génération(s) supprimée(s) avec succès !\033[0m\n\033[1;36m💡 Pour récupérer l'espace disque des paquets non utilisés, vous pouvez lancer « Garbage Collect complet » dans le Dashboard.\033[0m""#
                    ),
                ],
            )
        }
        "switch-generation" => {
            let id_raw = extra.unwrap_or_default();
            let id: u32 = id_raw
                .trim()
                .parse::<u32>()
                .map_err(|_| "ID de génération invalide.".to_string())?;
            (
                "bash".into(),
                vec![
                    "-c".into(),
                    format!(
                        r#"echo -e "\033[1;35m🔄 Bascule sur la génération NixOS #{id} pour le prochain reboot...\033[0m\n" ; sudo nix-env --profile /nix/var/nix/profiles/system --switch-generation {id} && echo -e "\n\033[1;34m⚙️ Configuration du bootloader sur le profil #{id}...\033[0m\n" && sudo /nix/var/nix/profiles/system/bin/switch-to-configuration boot && echo -e "\n\033[1;32m✅ Génération #{id} activée avec succès ! Elle sera chargée au prochain redémarrage.\033[0m""#
                    ),
                ],
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
        "apply-packages" => (
            "bash".into(),
            vec![
                "-c".into(),
                r#"echo -e "\033[1;35m📦 Application des paquets personnalisés NixOS (nh os switch)...\033[0m\n" ; nh os switch /etc/nixos && echo -e "\n\033[1;32m✅ Configuration et paquets personnalisés appliqués avec succès !\033[0m""#.into(),
            ],
        ),
        "apply-firewall" => (
            "bash".into(),
            vec![
                "-c".into(),
                r#"echo -e "\033[1;35m🛡️ Application des règles du pare-feu NixOS (nh os switch)...\033[0m\n" ; nh os switch /etc/nixos && echo -e "\n\033[1;32m✅ Règles du pare-feu appliquées avec succès !\033[0m""#.into(),
            ],
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

#[tauri::command]
fn get_storage_devices() -> Result<Vec<disks::DiskDevice>, String> {
    disks::list_storage_devices()
}

#[tauri::command]
fn mount_storage_device(uuid: String, mount_point: String, fs_type: String) -> Result<String, String> {
    disks::mount_storage_device(uuid, mount_point, fs_type)
}

#[tauri::command]
fn unmount_storage_device(mount_point: String, uuid: Option<String>) -> Result<String, String> {
    disks::unmount_storage_device(mount_point, uuid)
}

#[tauri::command]
fn format_storage_device(device_path: String, fs_type: String, label: String) -> Result<String, String> {
    disks::format_storage_device(device_path, fs_type, label)
}

#[tauri::command]
fn get_current_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "chomiam".to_string())
}

#[tauri::command]
fn check_system_updates() -> Result<updates::UpdateCheckResult, String> {
    Ok(updates::check_system_updates())
}

#[tauri::command]
fn open_in_file_manager(path: String) -> Result<(), String> {
    let _ = std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Impossible d'ouvrir le dossier : {}", e))?;
    Ok(())
}

#[tauri::command]
fn restart_dashboard(app: AppHandle) {
    // Sur NixOS, tauri::process::restart() ré-exécute current_exe() qui pointe
    // vers l'ancien chemin immuable /nix/store/... de l'application qui tournait.
    // Pour exécuter la NOUVELLE version après un nh os switch, on cible en priorité
    // le lien système actif /run/current-system/sw/bin/chomiamos-dashboard.
    let user = std::env::var("USER").unwrap_or_else(|_| "chomiam".into());
    let per_user_path = format!("/etc/profiles/per-user/{}/bin/chomiamos-dashboard", user);

    let candidates = [
        "/run/current-system/sw/bin/chomiamos-dashboard",
        &per_user_path,
    ];

    let mut target_bin = None;
    for c in &candidates {
        let p = std::path::Path::new(c);
        if p.exists() {
            target_bin = Some(p.to_path_buf());
            break;
        }
    }

    let binary = target_bin.unwrap_or_else(|| {
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("chomiamos-dashboard"))
    });

    let _ = std::process::Command::new(&binary)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    app.exit(0);
}

#[tauri::command]
fn get_github_token() -> Result<Option<String>, String> {
    let path = std::path::Path::new("/etc/nixos/secrets/github-token.conf");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Impossible de lire le fichier de token : {}", e))?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(pos) = trimmed.find("github.com=") {
            let token = trimmed[pos + "github.com=".len()..].trim();
            return Ok(Some(token.to_string()));
        } else if trimmed.starts_with("ghp_") || trimmed.starts_with("github_pat_") {
            return Ok(Some(trimmed.to_string()));
        }
    }
    Ok(None)
}

#[tauri::command]
fn save_github_token(token: String) -> Result<(), String> {
    let dir = std::path::Path::new("/etc/nixos/secrets");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(dir);
    }
    let path = dir.join("github-token.conf");
    let trimmed = token.trim();

    if trimmed.is_empty() {
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        return Ok(());
    }

    let clean_token = if let Some(pos) = trimmed.find("github.com=") {
        trimmed[pos + "github.com=".len()..].trim()
    } else {
        trimmed
    };

    let file_content = format!("access-tokens = github.com={}\n", clean_token);

    match std::fs::write(&path, &file_content) {
        Ok(_) => Ok(()),
        Err(_) => {
            let mut child = std::process::Command::new("pkexec")
                .args(["tee", path.to_str().unwrap()])
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("Impossible d'enregistrer le token : {}", e))?;

            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(file_content.as_bytes());
            }
            let status = child.wait().map_err(|e| e.to_string())?;
            if status.success() {
                Ok(())
            } else {
                Err("Échec de l'enregistrement avec les privilèges administrateur.".into())
            }
        }
    }
}

#[tauri::command]
fn delete_github_token() -> Result<(), String> {
    let path = std::path::Path::new("/etc/nixos/secrets/github-token.conf");
    if path.exists() {
        match std::fs::remove_file(path) {
            Ok(_) => Ok(()),
            Err(_) => {
                let status = std::process::Command::new("pkexec")
                    .args(["rm", "-f", path.to_str().unwrap()])
                    .status()
                    .map_err(|e| e.to_string())?;
                if status.success() {
                    Ok(())
                } else {
                    Err("Impossible de supprimer le fichier de token.".into())
                }
            }
        }
    } else {
        Ok(())
    }
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Impossible d'ouvrir le navigateur : {}", e))?;
    Ok(())
}


#[tauri::command]
fn get_firewall_state() -> Result<firewall::FirewallState, String> {
    firewall::load_firewall_state()
}

#[tauri::command]
fn save_firewall_state(enabled: bool, rules: Vec<firewall::FirewallPortRule>) -> Result<(), String> {
    firewall::save_firewall_state(enabled, rules)
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
            delete_nix_generations,
            switch_nix_generation,
            get_chomiamos_config,
            save_chomiamos_config,
            start_terminal_task,
            write_pty,
            resize_pty,
            get_keyboard_lock_state,
            get_storage_devices,
            mount_storage_device,
            unmount_storage_device,
            format_storage_device,
            open_in_file_manager,
            get_current_user,
            check_system_updates,
            restart_dashboard,
            get_github_token,
            save_github_token,
            delete_github_token,
            open_external_url,
            search_nix_packages,
            get_custom_packages,
            add_custom_package,
            remove_custom_package,
            get_firewall_state,
            save_firewall_state
        ])
        .run(tauri::generate_context!())
        .expect("Erreur lors de l'exécution de l'application ChomiamOS Dashboard");
}
