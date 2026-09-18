use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeyboardConfig {
    #[serde(default = "default_layout")]
    pub layout: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default = "default_layout")]
    pub key_map: String,
}

fn default_layout() -> String { "fr".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingConfig {
    pub steam: bool,
    pub gamescope_session: bool,
    pub goverlay: bool,
    pub lutris: bool,
    pub heroic: bool,
    pub faugus: bool,
    pub decky_loader: bool,
    pub geforce_now: bool,
    pub steering_wheels: bool,
    pub sunshine: bool,
    pub sober: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulationConfig {
    pub enable: bool,
    pub duckstation: bool,
    pub frontend: String, // "es-de" or "none"
    pub retroarch: bool,
    pub eden: bool,
    pub dolphin: bool,
    pub pcsx2: bool,
    pub ppsspp: bool,
    pub melonds: bool,
    pub azahar: bool,
    pub mgba: bool,
    pub rpcs3: bool,
    pub xemu: bool,
    #[serde(default)]
    pub cemu: bool,
    #[serde(default, alias = "xenia-canary", alias = "xenia")]
    pub xenia_canary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    pub stremio: bool,
    pub flatseal: bool,
    pub vlc: bool,
    pub mpv: bool,
    pub tailscale: bool,
    pub localsend: bool,
    pub motrix: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreationConfig {
    pub davinci_resolve: String, // "none" | "free" | "studio"
    pub blender: bool,
    pub audacity: bool,
    pub ardour: bool,
    pub godot: bool,
    pub kdenlive: bool,
    pub obs_studio: bool,
    pub antigravity: bool,
    pub zed: bool,
    pub vscode: bool,
    pub pear_desktop: bool,
    pub virtualisation: bool,
    pub omniroute: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlicersConfig {
    pub orcaslicer: bool,
    pub prusaslicer: bool,
    pub cura: bool,
    pub bambustudio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChomiamConfig {
    pub host_name: String,
    #[serde(default)]
    pub user_block: String,
    #[serde(default = "default_gpu")]
    pub gpu_driver: String,
    #[serde(default = "default_timezone")]
    pub time_zone: String,
    #[serde(default = "default_locale")]
    pub default_locale: String,
    #[serde(default = "default_state_ver")]
    pub state_version: String,
    pub browser: String,
    pub discord_client: String,
    #[serde(default)]
    pub firewall: bool,
    pub desktop_env: String,
    #[serde(default)]
    pub keyboard: KeyboardConfig,
    pub gaming: GamingConfig,
    pub emulation: EmulationConfig,
    pub media: MediaConfig,
    pub creation: CreationConfig,
    #[serde(default)]
    pub slicers: SlicersConfig,
}

fn default_gpu() -> String { "amd".to_string() }
fn default_timezone() -> String { "Europe/Paris".to_string() }
fn default_locale() -> String { "fr_FR.UTF-8".to_string() }
fn default_state_ver() -> String { "26.05".to_string() }

fn default_user_block() -> String {
    let username = std::env::var("USER").unwrap_or_else(|_| "chomiam".to_string());
    format!(
r#"user = {{
    username = "{username}";
    fullName = "ChomiamOS User";
    homeDirectory = "/home/{username}";
    shell = "fish";
    initialHashedPassword = null;
    extraGroups = [
      "networkmanager"
      "wheel"
      "docker"
      "video"
    ];
  }};"#
    )
}

impl Default for ChomiamConfig {
    fn default() -> Self {
        Self {
            host_name: "chomiamos".to_string(),
            user_block: default_user_block(),
            gpu_driver: "amd".to_string(),
            time_zone: "Europe/Paris".to_string(),
            default_locale: "fr_FR.UTF-8".to_string(),
            state_version: "26.05".to_string(),
            browser: "chrome".to_string(),
            discord_client: "discord".to_string(),
            firewall: false,
            desktop_env: "gnome".to_string(),
            keyboard: KeyboardConfig {
                layout: "fr".to_string(),
                variant: "".to_string(),
                key_map: "fr".to_string(),
            },
            gaming: GamingConfig {
                steam: true,
                gamescope_session: true,
                goverlay: true,
                lutris: true,
                heroic: true,
                faugus: true,
                decky_loader: true,
                geforce_now: true,
                steering_wheels: true,
                sunshine: false,
                sober: false,
            },
            emulation: EmulationConfig {
                enable: true,
                frontend: "es-de".to_string(),
                retroarch: true,
                duckstation: true,
                eden: true,
                dolphin: true,
                pcsx2: true,
                ppsspp: true,
                melonds: true,
                azahar: true,
                mgba: true,
                rpcs3: false,
                xemu: false,
                cemu: false,
                xenia_canary: false,
            },
            media: MediaConfig {
                stremio: true,
                flatseal: true,
                vlc: true,
                mpv: true,
                tailscale: true,
                localsend: true,
                motrix: true,
            },
            creation: CreationConfig {
                davinci_resolve: "none".to_string(),
                blender: true,
                audacity: false,
                ardour: false,
                godot: true,
                kdenlive: true,
                obs_studio: true,
                antigravity: true,
                zed: true,
                vscode: false,
                pear_desktop: true,
                virtualisation: true,
                omniroute: false,
            },
            slicers: SlicersConfig::default(),
        }
    }
}

pub fn get_vars_path() -> PathBuf {
    PathBuf::from("/etc/nixos/vars.nix")
}

pub fn read_vars_nix(path: &Path) -> Result<ChomiamConfig, String> {
    if !path.exists() {
        return Ok(ChomiamConfig::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("Impossible de lire {}: {}", path.display(), e))?;

    // Charger vars-defaults.nix en tant que schéma de référence fallback
    let defaults_content = path.parent()
        .map(|p| p.join("vars-defaults.nix"))
        .and_then(|p| fs::read_to_string(p).ok());

    let get_str = |key: &str| -> Option<String> {
        extract_string_var(&content, key)
            .or_else(|| defaults_content.as_ref().and_then(|d| extract_string_var(d, key)))
    };
    let get_bool = |key: &str, default_val: bool| -> bool {
        extract_bool_var(&content, key)
            .or_else(|| defaults_content.as_ref().and_then(|d| extract_bool_var(d, key)))
            .unwrap_or(default_val)
    };
    let get_bool_in_block = |block: &str, key: &str, default_val: bool| -> bool {
        extract_bool_var_in_block(&content, block, key)
            .or_else(|| defaults_content.as_ref().and_then(|d| extract_bool_var_in_block(d, block, key)))
            .unwrap_or(default_val)
    };

    let mut cfg = ChomiamConfig::default();

    // Preserve exact user block
    if let Some(user) = extract_user_block(&content) {
        cfg.user_block = user;
    } else if let Some(ref d) = defaults_content {
        if let Some(user) = extract_user_block(d) {
            cfg.user_block = user;
        }
    }

    // Hardware & system variables
    if let Some(val) = get_str("gpuDriver") {
        cfg.gpu_driver = val;
    }
    if let Some(val) = get_str("hostName") {
        cfg.host_name = val;
    }
    if let Some(val) = get_str("timeZone") {
        cfg.time_zone = val;
    }
    if let Some(val) = get_str("defaultLocale") {
        cfg.default_locale = val;
    }
    if let Some(val) = get_str("stateVersion") {
        cfg.state_version = val;
    }

    if let Some(val) = get_str("browser") {
        cfg.browser = val;
    }
    if let Some(val) = get_str("discordClient") {
        cfg.discord_client = val;
    }
    cfg.firewall = get_bool("firewall", false);
    if let Some(val) = get_str("desktopEnv") {
        cfg.desktop_env = val;
    }
    if let Some(val) = get_str("davinciResolve") {
        cfg.creation.davinci_resolve = val;
    }

    // Gaming
    let is_nvidia = cfg.gpu_driver == "nvidia" || cfg.gpu_driver == "nvidia-legacy";
    cfg.gaming.steam = get_bool("steam", true);
    cfg.gaming.gamescope_session = if is_nvidia {
        false
    } else {
        extract_bool_var_in_block(&content, "gaming", "gamescopeSession")
            .or_else(|| defaults_content.as_ref().and_then(|d| extract_bool_var_in_block(d, "gaming", "gamescopeSession")))
            .unwrap_or(true)
    };
    cfg.gaming.goverlay = get_bool("goverlay", true);
    cfg.gaming.lutris = get_bool("lutris", true);
    cfg.gaming.heroic = get_bool("heroic", true);
    cfg.gaming.faugus = get_bool("faugus", true);
    cfg.gaming.decky_loader = get_bool("deckyLoader", false);
    cfg.gaming.geforce_now = get_bool("geforceNow", true);
    cfg.gaming.steering_wheels = get_bool("steeringWheelSupport", true);
    cfg.gaming.sunshine = extract_bool_var_in_block(&content, "gaming", "sunshine")
        .or_else(|| defaults_content.as_ref().and_then(|d| extract_bool_var_in_block(d, "gaming", "sunshine")))
        .unwrap_or(false);
    cfg.gaming.sober = extract_bool_var_in_block(&content, "gaming", "sober")
        .or_else(|| defaults_content.as_ref().and_then(|d| extract_bool_var_in_block(d, "gaming", "sober")))
        .unwrap_or(false);

    // Keyboard
    cfg.keyboard.layout = extract_string_var_in_block(&content, "keyboard", "layout")
        .or_else(|| defaults_content.as_ref().and_then(|d| extract_string_var_in_block(d, "keyboard", "layout")))
        .unwrap_or_else(|| "fr".to_string());
    cfg.keyboard.variant = extract_string_var_in_block(&content, "keyboard", "variant")
        .or_else(|| defaults_content.as_ref().and_then(|d| extract_string_var_in_block(d, "keyboard", "variant")))
        .unwrap_or_default();
    cfg.keyboard.key_map = extract_string_var_in_block(&content, "keyboard", "keyMap")
        .or_else(|| defaults_content.as_ref().and_then(|d| extract_string_var_in_block(d, "keyboard", "keyMap")))
        .unwrap_or_else(|| "fr".to_string());

    // Emulation
    if let Some(val) = get_str("frontend") {
        cfg.emulation.frontend = val;
    }
    cfg.emulation.enable = get_bool_in_block("emulation", "enable", false);
    cfg.emulation.retroarch = get_bool_in_block("retroarch", "enable", true);
    cfg.emulation.duckstation = get_bool("duckstation", true);
    cfg.emulation.eden = get_bool("eden", true);
    cfg.emulation.dolphin = get_bool("dolphin", true);
    cfg.emulation.pcsx2 = get_bool("pcsx2", true);
    cfg.emulation.ppsspp = get_bool("ppsspp", true);
    cfg.emulation.melonds = get_bool("melonds", true);
    cfg.emulation.azahar = get_bool("azahar", true);
    cfg.emulation.mgba = get_bool("mgba", true);
    cfg.emulation.rpcs3 = get_bool("rpcs3", false);
    cfg.emulation.xemu = get_bool("xemu", false);
    cfg.emulation.cemu = get_bool("cemu", false);
    cfg.emulation.xenia_canary = get_bool("xenia-canary", false) || get_bool("xenia", false);

    // Media & Network
    cfg.media.stremio = get_bool("stremio", true);
    cfg.media.flatseal = get_bool("flatseal", true);
    cfg.media.vlc = get_bool("vlc", true);
    cfg.media.mpv = get_bool("mpv", true);
    cfg.media.tailscale = get_bool("tailscale", true);
    cfg.media.localsend = get_bool("localsend", true);
    cfg.media.motrix = get_bool("motrix", true);

    // Creation & Tools
    cfg.creation.blender = get_bool("blender", false);
    cfg.creation.audacity = get_bool("audacity", false);
    cfg.creation.ardour = get_bool("ardour", false);
    cfg.creation.godot = get_bool("godot", false);

    // 3D Printing & Slicers
    cfg.slicers.orcaslicer = get_bool_in_block("slicers", "orcaslicer", false);
    cfg.slicers.prusaslicer = get_bool_in_block("slicers", "prusaslicer", false);
    cfg.slicers.cura = get_bool_in_block("slicers", "cura", false);
    cfg.slicers.bambustudio = get_bool_in_block("slicers", "bambustudio", false);
    cfg.creation.kdenlive = get_bool("kdenlive", false);
    cfg.creation.obs_studio = get_bool("obsStudio", true);
    cfg.creation.antigravity = get_bool_in_block("ide", "antigravity", false) || get_bool("antigravity", true);
    cfg.creation.zed = get_bool_in_block("ide", "zed", false) || get_bool("zed", false);
    cfg.creation.vscode = get_bool_in_block("ide", "vscode", false) || get_bool("vscode", false);
    cfg.creation.pear_desktop = get_bool("pearDesktop", true);
    cfg.creation.virtualisation = get_bool_in_block("virtualisation", "enable", false);
    cfg.creation.omniroute = get_bool_in_block("iaSuite", "enable", false) || get_bool_in_block("aiSuite", "enable", false) || get_bool_in_block("omniroute", "enable", false);

    Ok(cfg)
}

pub fn save_vars_nix(path: &Path, cfg: &ChomiamConfig) -> Result<(), String> {
    let mut config_to_save = cfg.clone();

    // CRITICAL SECURITY & STABILITY RULE:
    // Never overwrite an existing user block with default or empty user block!
    // If the file on disk has an existing user block, ALWAYS preserve it!
    if path.exists() {
        if let Ok(disk_content) = fs::read_to_string(path) {
            if let Some(existing_user) = extract_user_block(&disk_content) {
                config_to_save.user_block = existing_user;
            }
            if config_to_save.gpu_driver.is_empty() {
                if let Some(existing_gpu) = extract_string_var(&disk_content, "gpuDriver") {
                    config_to_save.gpu_driver = existing_gpu;
                }
            }
        }
    }

    if config_to_save.user_block.trim().is_empty() {
        config_to_save.user_block = default_user_block();
    }

    let nix_code = generate_vars_nix_content(&config_to_save);

    // Atomic write
    let tmp_path = path.with_extension("nix.tmp");
    fs::write(&tmp_path, nix_code)
        .map_err(|e| format!("Impossible d'écrire dans {}: {}", tmp_path.display(), e))?;

    fs::rename(&tmp_path, path)
        .map_err(|e| format!("Impossible de remplacer {}: {}", path.display(), e))?;

    Ok(())
}

fn generate_vars_nix_content(c: &ChomiamConfig) -> String {
    format!(
r#"{{
  # =========================================================================
  # ⚙️ VARIABLES DU SYSTÈME CHOMIAMOS GAMING EDITION
  # Modifié via le Dashboard ChomiamOS
  # =========================================================================

  # Nom d'hôte de la machine (Hostname)
  hostName = "{hostname}";

  # Localisation & Fuseau horaire
  timeZone = "{time_zone}";
  defaultLocale = "{default_locale}";

  # Disposition du clavier
  keyboard = {{
    layout = "{kbd_layout}";
    variant = "{kbd_variant}";
    keyMap = "{kbd_key_map}";
  }};

  # Version de l'état système NixOS / Home Manager
  stateVersion = "{state_version}";

  # Profil utilisateur principal (Préservé automatiquement)
  {user_block}

  # Virtualisation
  virtualisation = {{
    enable = {virtualisation};
  }};

  # Navigateur web principal
  browser = "{browser}";

  # Client Discord
  discordClient = "{discord_client}";

  # Pare-feu réseau
  firewall = {firewall};

  # Environnement de bureau
  desktopEnv = "{desktop_env}";

  # Matériel GPU (Préservé automatiquement)
  gpuDriver = "{gpu_driver}";

  # Options du mode Gaming
  gaming = {{
    enable = true;
    gamescopeSession = {gamescope_session};
    launchers = {{
      steam = {steam};
      lutris = {lutris};
      heroic = {heroic};
      faugus = {faugus};
    }};
    deckyLoader = {decky_loader};
    geforceNow = {geforce_now};
    mountGamesDisk = true;
    sunshine = {sunshine};
    sober = {sober};
  }};

  # Suite d'Émulation & Rétrogaming
  emulation = {{
    enable = {emulation_enable};
    frontend = "{emulation_frontend}";
    autoCheckUpdates = true;

    retroarch = {{
      enable = {retroarch};
    }};

    standalone = {{
      duckstation = {duckstation};
      eden = {eden};
      dolphin = {dolphin};
      pcsx2 = {pcsx2};
      ppsspp = {ppsspp};
      melonds = {melonds};
      mgba = {mgba};
      azahar = {azahar};
      rpcs3 = {rpcs3};
      xemu = {xemu};
      cemu = {cemu};
      xenia-canary = {xenia_canary};
    }};
  }};

  # Volants & Simracing
  steeringWheelSupport = {steering_wheels};

  # Montage vidéo DaVinci Resolve
  davinciResolve = "{davinci}";

  # Logiciels de Création 3D & Moteur de jeu
  blender = {blender};
  godot = {godot};

  # Applications Réseau & Partage
  tailscale = {tailscale};
  flatseal = {flatseal};
  goverlay = {goverlay};
  audacity = {audacity};
  ardour = {ardour};
  localsend = {localsend};
  motrix = {motrix};

  # Impression 3D & Slicers
  slicers = {{
    orcaslicer = {orcaslicer};
    prusaslicer = {prusaslicer};
    cura = {cura};
    bambustudio = {bambustudio};
  }};

  # Multimédia & Streaming
  stremio = {stremio};
  vlc = {vlc};
  mpv = {mpv};

  # Environnements de Développement & IDEs (Choix multiple)
  ide = {{
    zed = {zed};
    antigravity = {antigravity};
    vscode = {vscode};
  }};
  antigravity = {antigravity};
  zed = {zed};
  vscode = {vscode};
  pearDesktop = {pear_desktop};
  kdenlive = {kdenlive};
  obsStudio = {obs_studio};

  # Suite IA locale complète (Open WebUI, Ollama accéléré par GPU, Agent IA Hermes)
  # Interfaces : Open WebUI sur http://localhost:8080, Hermes Dashboard sur http://localhost:9119
  iaSuite = {{
    enable = {omniroute};
  }};
}}
"#,
        hostname = c.host_name,
        time_zone = c.time_zone,
        default_locale = c.default_locale,
        state_version = c.state_version,
        user_block = c.user_block.trim(),
        virtualisation = c.creation.virtualisation,
        browser = c.browser,
        discord_client = c.discord_client,
        firewall = c.firewall,
        desktop_env = c.desktop_env,
        gpu_driver = c.gpu_driver,
        steam = c.gaming.steam,
        gamescope_session = c.gaming.gamescope_session,
        goverlay = c.gaming.goverlay,
        flatseal = c.media.flatseal,
        audacity = c.creation.audacity,
        ardour = c.creation.ardour,
        orcaslicer = c.slicers.orcaslicer,
        prusaslicer = c.slicers.prusaslicer,
        cura = c.slicers.cura,
        bambustudio = c.slicers.bambustudio,
        lutris = c.gaming.lutris,
        heroic = c.gaming.heroic,
        faugus = c.gaming.faugus,
        decky_loader = c.gaming.decky_loader,
        geforce_now = c.gaming.geforce_now,
        sunshine = c.gaming.sunshine,
        sober = c.gaming.sober,
        kbd_layout = c.keyboard.layout,
        kbd_variant = c.keyboard.variant,
        kbd_key_map = c.keyboard.key_map,
        emulation_enable = c.emulation.enable,
        emulation_frontend = c.emulation.frontend,
        retroarch = c.emulation.retroarch,
        duckstation = c.emulation.duckstation,
        eden = c.emulation.eden,
        dolphin = c.emulation.dolphin,
        pcsx2 = c.emulation.pcsx2,
        ppsspp = c.emulation.ppsspp,
        melonds = c.emulation.melonds,
        mgba = c.emulation.mgba,
        azahar = c.emulation.azahar,
        rpcs3 = c.emulation.rpcs3,
        xemu = c.emulation.xemu,
        cemu = c.emulation.cemu,
        xenia_canary = c.emulation.xenia_canary,
        steering_wheels = c.gaming.steering_wheels,
        davinci = c.creation.davinci_resolve,
        blender = c.creation.blender,
        godot = c.creation.godot,
        tailscale = c.media.tailscale,
        localsend = c.media.localsend,
        motrix = c.media.motrix,
        stremio = c.media.stremio,
        vlc = c.media.vlc,
        mpv = c.media.mpv,
        antigravity = c.creation.antigravity,
        zed = c.creation.zed,
        vscode = c.creation.vscode,
        pear_desktop = c.creation.pear_desktop,
        kdenlive = c.creation.kdenlive,
        obs_studio = c.creation.obs_studio,
        omniroute = c.creation.omniroute,
    )
}

fn extract_user_block(text: &str) -> Option<String> {
    let start_idx = text.find("user = {")?;
    let brace_start = text[start_idx..].find('{')? + start_idx;
    let mut depth = 0;
    let mut end_idx = None;

    for (i, c) in text[brace_start..].char_indices() {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                let rest = &text[brace_start + i + 1..];
                let semi = rest.find(';').unwrap_or(0);
                end_idx = Some(brace_start + i + 1 + semi + 1);
                break;
            }
        }
    }

    if let Some(end) = end_idx {
        Some(text[start_idx..end].trim().to_string())
    } else {
        None
    }
}

fn extract_string_var(text: &str, var_name: &str) -> Option<String> {
    let re = regex::Regex::new(&format!(r#"{}\s*=\s*"([^"]+)""#, var_name)).ok()?;
    re.captures(text).map(|cap| cap[1].to_string())
}

fn extract_bool_var(text: &str, var_name: &str) -> Option<bool> {
    let re = regex::Regex::new(&format!(r#"{}\s*=\s*(true|false)"#, var_name)).ok()?;
    re.captures(text).map(|cap| &cap[1] == "true")
}

fn extract_bool_var_in_block(text: &str, block: &str, var_name: &str) -> Option<bool> {
    let block_re = regex::Regex::new(&format!(r#"{}\s*=\s*\{{([^}}]+)\}}"#, block)).ok()?;
    if let Some(cap) = block_re.captures(text) {
        let inside = &cap[1];
        return extract_bool_var(inside, var_name);
    }
    None
}

fn extract_string_var_in_block(text: &str, block: &str, var_name: &str) -> Option<String> {
    let block_re = regex::Regex::new(&format!(r#"{}\\s*=\\s*\\{{([^}}]+)\\}}"#, block)).ok()?;
    if let Some(cap) = block_re.captures(text) {
        let inside = &cap[1];
        return extract_string_var(inside, var_name);
    }
    None
}


pub fn get_configured_shell() -> String {
    let path = get_vars_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Some(user_block) = extract_user_block(&content) {
            if let Some(shell) = extract_string_var(&user_block, "shell") {
                return shell;
            }
        }
        if let Some(shell) = extract_string_var(&content, "shell") {
            return shell;
        }
    }
    "fish".to_string()
}

pub fn set_configured_shell(new_shell: &str) -> Result<String, String> {
    let new_shell = new_shell.trim().to_lowercase();
    if !["fish", "zsh", "bash"].contains(&new_shell.as_str()) {
        return Err(format!("Shell non supporté: '{}'. Les options valides sont fish, zsh ou bash.", new_shell));
    }

    let path = get_vars_path();
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Impossible de lire {}: {}", path.display(), e))?;

    let updated_content = if let Some(old_user_block) = extract_user_block(&content) {
        let shell_re = regex::Regex::new(r#"shell\s*=\s*"[^"]+";"#)
            .map_err(|e| e.to_string())?;
        let new_user_block = if shell_re.is_match(&old_user_block) {
            shell_re.replace(&old_user_block, format!(r#"shell = "{}";"#, new_shell)).to_string()
        } else {
            if let Some(last_brace) = old_user_block.rfind("}") {
                let mut s = old_user_block[..last_brace].to_string();
                s.push_str(&format!("    shell = \"{}\";\n  }};", new_shell));
                s
            } else {
                format!("{}\n    shell = \"{}\";", old_user_block, new_shell)
            }
        };
        content.replace(&old_user_block, &new_user_block)
    } else {
        let shell_re = regex::Regex::new(r#"shell\s*=\s*"[^"]+";"#)
            .map_err(|e| e.to_string())?;
        if shell_re.is_match(&content) {
            shell_re.replace(&content, format!(r#"shell = "{}";"#, new_shell)).to_string()
        } else {
            return Err("Impossible de localiser le bloc 'user' dans vars.nix".to_string());
        }
    };

    let tmp_path = path.with_extension("nix.tmp");
    fs::write(&tmp_path, &updated_content)
        .map_err(|e| format!("Impossible d'écrire dans {}: {}", tmp_path.display(), e))?;

    fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Impossible de remplacer {}: {}", path.display(), e))?;

    let _ = fs::write("/etc/nixos/.vars.nix.backup", &updated_content);

    Ok(new_shell)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_block_and_shell() {
        let sample = r#"{
  hostName = "chomiamos";
  user = {
    username = "chomiam";
    shell = "fish";
    extraGroups = [ "wheel" ];
  };
  tailscale = false;
}"#;
        let u = extract_user_block(sample).expect("extract user block");
        assert!(u.contains(r#"shell = "fish";"#));
        let shell = extract_string_var(&u, "shell").expect("shell var");
        assert_eq!(shell, "fish");

        let re = regex::Regex::new(r#"shell\s*=\s*"[^"]+";"#).unwrap();
        let replaced = re.replace(&u, r#"shell = "zsh";"#);
        assert!(replaced.contains(r#"shell = "zsh";"#));
    }

    #[test]
    fn test_emulation_cemu_and_xenia() {
        let mut cfg = ChomiamConfig::default();
        assert!(!cfg.emulation.cemu);
        assert!(!cfg.emulation.xenia_canary);

        cfg.emulation.cemu = true;
        cfg.emulation.xenia_canary = true;

        let json = serde_json::to_string(&cfg).expect("serialize");
        assert!(json.contains(r#""cemu":true"#));
        assert!(json.contains(r#""xenia_canary":true"#));

        let deserialized: ChomiamConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.emulation.cemu);
        assert!(deserialized.emulation.xenia_canary);

        let json_alias = r#"{"enable":true,"duckstation":false,"frontend":"es-de","retroarch":false,"eden":false,"dolphin":false,"pcsx2":false,"ppsspp":false,"melonds":false,"azahar":false,"mgba":false,"rpcs3":false,"xemu":false,"cemu":true,"xenia-canary":true}"#;
        let emu: EmulationConfig = serde_json::from_str(json_alias).expect("deserialize alias");
        assert!(emu.cemu);
        assert!(emu.xenia_canary);
    }
}
