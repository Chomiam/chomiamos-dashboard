use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GamingConfig {
    pub steam: bool,
    pub lutris: bool,
    pub heroic: bool,
    pub faugus: bool,
    pub decky_loader: bool,
    pub geforce_now: bool,
    pub steering_wheels: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulationConfig {
    pub enable: bool,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaConfig {
    pub stremio: bool,
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
    pub godot: bool,
    pub kdenlive: bool,
    pub obs_studio: bool,
    pub antigravity: bool,
    pub pear_desktop: bool,
    pub virtualisation: bool,
    pub ai_suite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChomiamConfig {
    pub host_name: String,
    pub browser: String,
    pub discord_client: String,
    pub desktop_env: String,
    pub gaming: GamingConfig,
    pub emulation: EmulationConfig,
    pub media: MediaConfig,
    pub creation: CreationConfig,
}

impl Default for ChomiamConfig {
    fn default() -> Self {
        Self {
            host_name: "chomiamos".to_string(),
            browser: "chrome".to_string(),
            discord_client: "discord".to_string(),
            desktop_env: "gnome".to_string(),
            gaming: GamingConfig {
                steam: true,
                lutris: true,
                heroic: true,
                faugus: true,
                decky_loader: true,
                geforce_now: true,
                steering_wheels: true,
            },
            emulation: EmulationConfig {
                enable: true,
                frontend: "es-de".to_string(),
                retroarch: true,
                eden: true,
                dolphin: true,
                pcsx2: true,
                ppsspp: true,
                melonds: true,
                azahar: true,
                mgba: true,
                rpcs3: false,
            },
            media: MediaConfig {
                stremio: true,
                vlc: true,
                mpv: true,
                tailscale: true,
                localsend: true,
                motrix: true,
            },
            creation: CreationConfig {
                davinci_resolve: "none".to_string(),
                blender: true,
                godot: true,
                kdenlive: true,
                obs_studio: true,
                antigravity: true,
                pear_desktop: true,
                virtualisation: true,
                ai_suite: false,
            },
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

    let mut cfg = ChomiamConfig::default();

    // Parse simple string assignments
    if let Some(val) = extract_string_var(&content, "hostName") {
        cfg.host_name = val;
    }
    if let Some(val) = extract_string_var(&content, "browser") {
        cfg.browser = val;
    }
    if let Some(val) = extract_string_var(&content, "discordClient") {
        cfg.discord_client = val;
    }
    if let Some(val) = extract_string_var(&content, "desktopEnv") {
        cfg.desktop_env = val;
    }
    if let Some(val) = extract_string_var(&content, "davinciResolve") {
        cfg.creation.davinci_resolve = val;
    }

    // Gaming
    cfg.gaming.steam = extract_bool_var(&content, "steam").unwrap_or(true);
    cfg.gaming.lutris = extract_bool_var(&content, "lutris").unwrap_or(true);
    cfg.gaming.heroic = extract_bool_var(&content, "heroic").unwrap_or(true);
    cfg.gaming.faugus = extract_bool_var(&content, "faugus").unwrap_or(true);
    cfg.gaming.decky_loader = extract_bool_var(&content, "deckyLoader").unwrap_or(true);
    cfg.gaming.geforce_now = extract_bool_var(&content, "geforceNow").unwrap_or(true);
    cfg.gaming.steering_wheels = extract_bool_var(&content, "steeringWheelSupport").unwrap_or(true);

    // Emulation
    if let Some(val) = extract_string_var(&content, "frontend") {
        cfg.emulation.frontend = val;
    }
    cfg.emulation.enable = extract_bool_var_in_block(&content, "emulation", "enable").unwrap_or(true);
    cfg.emulation.retroarch = extract_bool_var_in_block(&content, "retroarch", "enable").unwrap_or(true);
    cfg.emulation.eden = extract_bool_var(&content, "eden").unwrap_or(true);
    cfg.emulation.dolphin = extract_bool_var(&content, "dolphin").unwrap_or(true);
    cfg.emulation.pcsx2 = extract_bool_var(&content, "pcsx2").unwrap_or(true);
    cfg.emulation.ppsspp = extract_bool_var(&content, "ppsspp").unwrap_or(true);
    cfg.emulation.melonds = extract_bool_var(&content, "melonds").unwrap_or(true);
    cfg.emulation.azahar = extract_bool_var(&content, "azahar").unwrap_or(true);
    cfg.emulation.mgba = extract_bool_var(&content, "mgba").unwrap_or(true);
    cfg.emulation.rpcs3 = extract_bool_var(&content, "rpcs3").unwrap_or(false);

    // Media & Network
    cfg.media.stremio = extract_bool_var(&content, "stremio").unwrap_or(true);
    cfg.media.vlc = extract_bool_var(&content, "vlc").unwrap_or(true);
    cfg.media.mpv = extract_bool_var(&content, "mpv").unwrap_or(true);
    cfg.media.tailscale = extract_bool_var(&content, "tailscale").unwrap_or(true);
    cfg.media.localsend = extract_bool_var(&content, "localsend").unwrap_or(true);
    cfg.media.motrix = extract_bool_var(&content, "motrix").unwrap_or(true);

    // Creation & Tools
    cfg.creation.blender = extract_bool_var(&content, "blender").unwrap_or(true);
    cfg.creation.godot = extract_bool_var(&content, "godot").unwrap_or(true);
    cfg.creation.kdenlive = extract_bool_var(&content, "kdenlive").unwrap_or(true);
    cfg.creation.obs_studio = extract_bool_var(&content, "obsStudio").unwrap_or(true);
    cfg.creation.antigravity = extract_bool_var(&content, "antigravity").unwrap_or(true);
    cfg.creation.pear_desktop = extract_bool_var(&content, "pearDesktop").unwrap_or(true);
    cfg.creation.virtualisation = extract_bool_var_in_block(&content, "virtualisation", "enable").unwrap_or(true);
    cfg.creation.ai_suite = extract_bool_var_in_block(&content, "aiSuite", "enable").unwrap_or(false);

    Ok(cfg)
}

pub fn save_vars_nix(path: &Path, cfg: &ChomiamConfig) -> Result<(), String> {
    let nix_code = generate_vars_nix_content(cfg);

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
  timeZone = "Europe/Paris";
  defaultLocale = "fr_FR.UTF-8";

  # Version de l'état système NixOS / Home Manager
  stateVersion = "26.05";

  # Profil utilisateur principal
  user = {{
    username = "chomiam";
    fullName = "ChomiamOS User";
    homeDirectory = "/home/chomiam";
    shell = "fish";
    initialHashedPassword = null;
    extraGroups = [
      "networkmanager"
      "wheel"
      "docker"
      "video"
    ];
  }};

  # Virtualisation
  virtualisation = {{
    enable = {virtualisation};
  }};

  # Navigateur web principal
  browser = "{browser}";

  # Client Discord
  discordClient = "{discord_client}";

  # Pare-feu réseau
  firewall = false;

  # Environnement de bureau
  desktopEnv = "{desktop_env}";

  # Matériel GPU
  gpuDriver = "amd";

  # Options du mode Gaming
  gaming = {{
    enable = true;
    launchers = {{
      steam = {steam};
      lutris = {lutris};
      heroic = {heroic};
      faugus = {faugus};
    }};
    deckyLoader = {decky_loader};
    geforceNow = {geforce_now};
    mountGamesDisk = true;
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
      eden = {eden};
      dolphin = {dolphin};
      pcsx2 = {pcsx2};
      ppsspp = {ppsspp};
      melonds = {melonds};
      mgba = {mgba};
      azahar = {azahar};
      rpcs3 = {rpcs3};
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
  localsend = {localsend};
  motrix = {motrix};

  # Multimédia & Streaming
  stremio = {stremio};
  vlc = {vlc};
  mpv = {mpv};

  # Productivité & Outils
  antigravity = {antigravity};
  pearDesktop = {pear_desktop};
  kdenlive = {kdenlive};
  obsStudio = {obs_studio};

  # Suite IA Locale
  aiSuite = {{
    enable = {ai_suite};
    rocmOverrideGfx = "12.0.1";
    keepAlive = "0s";
    openWebUiPort = 8080;
    searxPort = 8888;
  }};
}}
"#,
        hostname = c.host_name,
        virtualisation = c.creation.virtualisation,
        browser = c.browser,
        discord_client = c.discord_client,
        desktop_env = c.desktop_env,
        steam = c.gaming.steam,
        lutris = c.gaming.lutris,
        heroic = c.gaming.heroic,
        faugus = c.gaming.faugus,
        decky_loader = c.gaming.decky_loader,
        geforce_now = c.gaming.geforce_now,
        emulation_enable = c.emulation.enable,
        emulation_frontend = c.emulation.frontend,
        retroarch = c.emulation.retroarch,
        eden = c.emulation.eden,
        dolphin = c.emulation.dolphin,
        pcsx2 = c.emulation.pcsx2,
        ppsspp = c.emulation.ppsspp,
        melonds = c.emulation.melonds,
        mgba = c.emulation.mgba,
        azahar = c.emulation.azahar,
        rpcs3 = c.emulation.rpcs3,
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
        pear_desktop = c.creation.pear_desktop,
        kdenlive = c.creation.kdenlive,
        obs_studio = c.creation.obs_studio,
        ai_suite = c.creation.ai_suite,
    )
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
