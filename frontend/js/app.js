// ==========================================================================
// ChomiamOS Dashboard - Tauri v2 + xterm.js Controller
// ==========================================================================

const invoke = window.__TAURI__ ? window.__TAURI__.core.invoke : async () => ({});
const listen = window.__TAURI__ ? window.__TAURI__.event.listen : async () => () => {};

let currentConfig = null;
let initialConfigStr = "";
let term = null;
let fitAddon = null;
let termInitialized = false;

document.addEventListener("DOMContentLoaded", () => {
  initTabs();
  startMetricsPolling();
  loadGenerations();
  loadConfig();
  initTerminal();
});

// 1. Tab Navigation
function initTabs() {
  const tabs = document.querySelectorAll(".tab-btn");
  tabs.forEach(btn => {
    btn.addEventListener("click", () => {
      tabs.forEach(t => t.classList.remove("active"));
      document.querySelectorAll(".tab-content").forEach(c => c.classList.remove("active"));

      btn.classList.add("active");
      const targetId = btn.getAttribute("data-tab");
      const targetContent = document.getElementById(targetId);
      if (targetContent) {
        targetContent.classList.add("active");
      }
    });
  });
}

// 2. Metrics Polling
function startMetricsPolling() {
  fetchMetrics();
  setInterval(fetchMetrics, 1500);
}

async function fetchMetrics() {
  try {
    const m = await invoke("get_system_metrics");
    if (!m) return;
    renderMetrics(m);
  } catch (err) {
    console.error("Erreur métriques:", err);
  }
}

function renderMetrics(m) {
  // System Info
  const hostnameEl = document.getElementById("sys-hostname");
  if (hostnameEl) hostnameEl.textContent = m.hostname || "ChomiamOS";
  const osEl = document.getElementById("sys-os");
  if (osEl) osEl.textContent = m.os_name || "ChomiamOS Linux";
  const kernelEl = document.getElementById("sys-kernel");
  if (kernelEl) kernelEl.textContent = m.kernel_version || "Linux";
  const uptimeEl = document.getElementById("sys-uptime");
  if (uptimeEl) uptimeEl.textContent = formatUptime(m.uptime_secs || 0);

  // CPU
  const cpuPercent = Math.round(m.cpu_usage_percent || 0);
  const cpuUsageEl = document.getElementById("cpu-usage");
  if (cpuUsageEl) cpuUsageEl.textContent = `${cpuPercent}%`;
  const cpuModelEl = document.getElementById("cpu-model");
  if (cpuModelEl) cpuModelEl.textContent = m.cpu_model || "CPU";
  const cpuBarEl = document.getElementById("cpu-bar");
  if (cpuBarEl) cpuBarEl.style.width = `${cpuPercent}%`;

  // RAM
  const ramUsageEl = document.getElementById("ram-usage");
  if (ramUsageEl) ramUsageEl.textContent = `${m.ram_used_gb?.toFixed(1) || 0} / ${m.ram_total_gb?.toFixed(1) || 0} Go`;
  const ramPercent = m.ram_total_gb ? Math.round((m.ram_used_gb / m.ram_total_gb) * 100) : 0;
  const ramBarEl = document.getElementById("ram-bar");
  if (ramBarEl) ramBarEl.style.width = `${ramPercent}%`;

  // GPU
  const gpuModelEl = document.getElementById("gpu-model");
  if (gpuModelEl) gpuModelEl.textContent = m.gpu_model || "GPU Détecté";
  const gpuUsageEl = document.getElementById("gpu-usage");
  if (gpuUsageEl) gpuUsageEl.textContent = m.gpu_usage_percent !== null ? `${Math.round(m.gpu_usage_percent)}%` : "Actif";
  const gpuBarEl = document.getElementById("gpu-bar");
  if (gpuBarEl) gpuBarEl.style.width = m.gpu_usage_percent !== null ? `${Math.round(m.gpu_usage_percent)}%` : "10%";

  // Disks
  renderDisks(m.disks || []);
}

function renderDisks(disks) {
  const container = document.getElementById("disks-container");
  if (!container) return;

  if (disks.length === 0) {
    container.innerHTML = `<div class="empty-state">Aucun disque détecté</div>`;
    return;
  }

  container.innerHTML = disks.map(d => {
    const pct = d.total_gb ? Math.round((d.used_gb / d.total_gb) * 100) : 0;
    return `
      <div class="disk-card">
        <div class="disk-header">
          <div class="disk-icon">💽</div>
          <div class="disk-info">
            <div class="disk-name">${d.name || d.mount_point}</div>
            <div class="disk-mount">${d.mount_point} (${d.fs_type})</div>
          </div>
          <div class="disk-pct">${pct}%</div>
        </div>
        <div class="progress-bar-bg">
          <div class="progress-bar-fill ${pct > 85 ? 'danger' : ''}" style="width: ${pct}%"></div>
        </div>
        <div class="disk-footer">
          <span>${d.used_gb?.toFixed(1)} Go utilisés</span>
          <span>${d.total_gb?.toFixed(1)} Go au total</span>
        </div>
      </div>
    `;
  }).join("");
}

function formatUptime(seconds) {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

// 3. Generations Management
async function loadGenerations() {
  const listEl = document.getElementById("generations-list");
  const countBadge = document.getElementById("generations-count-badge");
  if (!listEl) return;

  try {
    const summary = await invoke("get_generations");
    if (!summary || !summary.generations) {
      listEl.innerHTML = `<div class="empty-state">Impossible de charger les générations</div>`;
      return;
    }

    if (countBadge) {
      countBadge.textContent = `${summary.count} génération${summary.count > 1 ? 's' : ''}`;
    }

    if (summary.generations.length === 0) {
      listEl.innerHTML = `<div class="empty-state">Aucune génération disponible</div>`;
      return;
    }

    listEl.innerHTML = summary.generations.map(g => `
      <div class="gen-item ${g.is_current ? 'current' : ''}">
        <div class="gen-col-num">#${g.id}</div>
        <div class="gen-col-date">${g.date}</div>
        <div class="gen-col-badge">
          ${g.is_current ? '<span class="badge badge-active">Active</span>' : '<span class="badge badge-idle">Archivée</span>'}
        </div>
      </div>
    `).join("");
  } catch (err) {
    console.error("Erreur générations:", err);
    listEl.innerHTML = `<div class="empty-state">Erreur : ${err}</div>`;
  }
}

// 4. Config Management
async function loadConfig() {
  try {
    const c = await invoke("get_chomiamos_config");
    if (!c) return;
    currentConfig = c;
    initialConfigStr = JSON.stringify(c);
    populateConfigUI(c);
    attachConfigChangeListeners();
  } catch (err) {
    console.error("Erreur chargement config:", err);
  }
}

function populateConfigUI(c) {
  setRadioVal("browser", c.browser);
  setRadioVal("discord_client", c.discord_client);

  // Gaming
  setCheck("cfg-steam", c.gaming.steam);
  setCheck("cfg-lutris", c.gaming.lutris);
  setCheck("cfg-heroic", c.gaming.heroic);
  setCheck("cfg-faugus", c.gaming.faugus);
  setCheck("cfg-decky", c.gaming.decky_loader);
  setCheck("cfg-geforce", c.gaming.geforce_now);
  setCheck("cfg-wheels", c.gaming.steering_wheels);

  // Emulation
  setCheck("cfg-esde", c.emulation.frontend === "es-de");
  setCheck("cfg-retroarch", c.emulation.retroarch);
  setCheck("cfg-eden", c.emulation.eden);
  setCheck("cfg-dolphin", c.emulation.dolphin);
  setCheck("cfg-pcsx2", c.emulation.pcsx2);
  setCheck("cfg-ppsspp", c.emulation.ppsspp);
  setCheck("cfg-azahar", c.emulation.azahar);
  setCheck("cfg-melonds", c.emulation.melonds);
  setCheck("cfg-mgba", c.emulation.mgba);
  setCheck("cfg-rpcs3", c.emulation.rpcs3);

  // Media
  setCheck("cfg-stremio", c.media.stremio);
  setCheck("cfg-vlc", c.media.vlc);
  setCheck("cfg-mpv", c.media.mpv);
  setCheck("cfg-localsend", c.media.localsend);
  setCheck("cfg-tailscale", c.media.tailscale);
  setCheck("cfg-motrix", c.media.motrix);

  // Creation
  setCheck("cfg-obs", c.creation.obs_studio);
  setCheck("cfg-kdenlive", c.creation.kdenlive);
  setCheck("cfg-blender", c.creation.blender);
  setCheck("cfg-godot", c.creation.godot);
  setCheck("cfg-kvm", c.creation.virtualisation);
  setCheck("cfg-antigravity", c.creation.antigravity);
  setCheck("cfg-pear", c.creation.pear_desktop);
  setCheck("cfg-aisuite", c.creation.ai_suite);

  checkDirtyState();
}

function setRadioVal(name, val) {
  const radio = document.querySelector(`input[name="${name}"][value="${val}"]`);
  if (radio) radio.checked = true;
}

function setCheck(id, val) {
  const el = document.getElementById(id);
  if (el) el.checked = !!val;
}

function attachConfigChangeListeners() {
  const inputs = document.querySelectorAll("#tab-config input");
  inputs.forEach(input => {
    input.addEventListener("change", () => {
      readConfigFromUI();
      checkDirtyState();
    });
  });
}

function readConfigFromUI() {
  if (!currentConfig) return;

  const browserRadio = document.querySelector('input[name="browser"]:checked');
  if (browserRadio) currentConfig.browser = browserRadio.value;

  const discordRadio = document.querySelector('input[name="discord_client"]:checked');
  if (discordRadio) currentConfig.discord_client = discordRadio.value;

  currentConfig.gaming.steam = isChecked("cfg-steam");
  currentConfig.gaming.lutris = isChecked("cfg-lutris");
  currentConfig.gaming.heroic = isChecked("cfg-heroic");
  currentConfig.gaming.faugus = isChecked("cfg-faugus");
  currentConfig.gaming.decky_loader = isChecked("cfg-decky");
  currentConfig.gaming.geforce_now = isChecked("cfg-geforce");
  currentConfig.gaming.steering_wheels = isChecked("cfg-wheels");

  currentConfig.emulation.frontend = isChecked("cfg-esde") ? "es-de" : "none";
  currentConfig.emulation.retroarch = isChecked("cfg-retroarch");
  currentConfig.emulation.eden = isChecked("cfg-eden");
  currentConfig.emulation.dolphin = isChecked("cfg-dolphin");
  currentConfig.emulation.pcsx2 = isChecked("cfg-pcsx2");
  currentConfig.emulation.ppsspp = isChecked("cfg-ppsspp");
  currentConfig.emulation.azahar = isChecked("cfg-azahar");
  currentConfig.emulation.melonds = isChecked("cfg-melonds");
  currentConfig.emulation.mgba = isChecked("cfg-mgba");
  currentConfig.emulation.rpcs3 = isChecked("cfg-rpcs3");

  currentConfig.media.stremio = isChecked("cfg-stremio");
  currentConfig.media.vlc = isChecked("cfg-vlc");
  currentConfig.media.mpv = isChecked("cfg-mpv");
  currentConfig.media.localsend = isChecked("cfg-localsend");
  currentConfig.media.tailscale = isChecked("cfg-tailscale");
  currentConfig.media.motrix = isChecked("cfg-motrix");

  currentConfig.creation.obs_studio = isChecked("cfg-obs");
  currentConfig.creation.kdenlive = isChecked("cfg-kdenlive");
  currentConfig.creation.blender = isChecked("cfg-blender");
  currentConfig.creation.godot = isChecked("cfg-godot");
  currentConfig.creation.virtualisation = isChecked("cfg-kvm");
  currentConfig.creation.antigravity = isChecked("cfg-antigravity");
  currentConfig.creation.pear_desktop = isChecked("cfg-pear");
  currentConfig.creation.ai_suite = isChecked("cfg-aisuite");
}

function isChecked(id) {
  const el = document.getElementById(id);
  return el ? el.checked : false;
}

function checkDirtyState() {
  const dirtyBadge = document.getElementById("config-dirty-badge");
  if (!dirtyBadge) return;
  const isDirty = JSON.stringify(currentConfig) !== initialConfigStr;
  if (isDirty) {
    dirtyBadge.classList.remove("hidden");
  } else {
    dirtyBadge.classList.add("hidden");
  }
}

function resetConfig() {
  if (initialConfigStr) {
    currentConfig = JSON.parse(initialConfigStr);
    populateConfigUI(currentConfig);
  }
}

async function saveConfig(andApply = false) {
  readConfigFromUI();
  try {
    await invoke("save_chomiamos_config", { config: currentConfig });
    initialConfigStr = JSON.stringify(currentConfig);
    checkDirtyState();

    if (andApply) {
      runAction("switch");
    } else {
      alert("Configuration sauvegardée avec succès dans /etc/nixos/vars.nix !");
    }
  } catch (e) {
    alert("Erreur lors de la sauvegarde : " + e);
  }
}

// 5. Embedded xterm.js Native Terminal
function initTerminal() {
  if (termInitialized) return;
  termInitialized = true;

  const container = document.getElementById("terminal-output");
  if (!container) return;
  container.innerHTML = "";

  term = new Terminal({
    theme: {
      background: '#11111b',
      foreground: '#cdd6f4',
      cursor: '#f5e0dc',
      cursorAccent: '#11111b',
      selectionBackground: '#585b7066',
      black: '#45475a',
      red: '#f38ba8',
      green: '#a6e3a1',
      yellow: '#f9e2af',
      blue: '#89b4fa',
      magenta: '#f5c2e7',
      cyan: '#94e2d5',
      white: '#bac2de',
      brightBlack: '#585b70',
      brightRed: '#f38ba8',
      brightGreen: '#a6e3a1',
      brightYellow: '#f9e2af',
      brightBlue: '#89b4fa',
      brightMagenta: '#f5c2e7',
      brightCyan: '#94e2d5',
      brightWhite: '#a6adc8',
    },
    fontFamily: '"JetBrains Mono", monospace',
    fontSize: 13,
    lineHeight: 1.2,
    cursorBlink: true,
    convertEol: true,
  });

  if (window.FitAddon && window.FitAddon.FitAddon) {
    fitAddon = new window.FitAddon.FitAddon();
    term.loadAddon(fitAddon);
  }

  term.open(container);

  // Send keystrokes directly to PTY stdin
  term.onData(data => {
    invoke("write_pty", { data }).catch(console.error);
  });

  // Handle window resize
  window.addEventListener("resize", () => {
    if (fitAddon && term) {
      fitAddon.fit();
      invoke("resize_pty", { cols: term.cols, rows: term.rows }).catch(console.error);
    }
  });

  // Receive PTY data in real-time from Rust
  listen("pty-data", event => {
    if (term) {
      term.write(event.payload);
    }
  });

  // Process exit handler
  listen("pty-exit", event => {
    const exitCode = event.payload;
    const statusDot = document.getElementById("term-status-icon");
    const statusText = document.getElementById("term-status-text");

    if (term) {
      if (exitCode === 0) {
        term.write("\r\n\x1b[1;32m✔ Opération terminée avec succès !\x1b[0m\r\n");
      } else {
        term.write(`\r\n\x1b[1;31m✘ L'opération a échoué avec le code ${exitCode}\x1b[0m\r\n`);
      }
    }

    if (statusDot) {
      statusDot.className = exitCode === 0 ? "status-dot success" : "status-dot error";
    }
    if (statusText) {
      statusText.textContent = exitCode === 0 ? "Terminé (code 0)" : `Terminé avec erreur (code ${exitCode})`;
    }

    loadGenerations();
  });
}

function openTerminal(title = "Exécution en direct") {
  const modal = document.getElementById("terminal-modal");
  const titleEl = document.getElementById("term-title");
  const statusDot = document.getElementById("term-status-icon");
  const statusText = document.getElementById("term-status-text");

  if (titleEl) titleEl.textContent = title;
  if (statusDot) statusDot.className = "status-dot running";
  if (statusText) statusText.textContent = "Exécution en cours...";

  modal.classList.remove("hidden");

  if (!termInitialized) {
    initTerminal();
  }

  if (term) {
    term.reset();
  }

  setTimeout(() => {
    if (fitAddon && term) {
      fitAddon.fit();
    }
  }, 50);
}

function closeTerminal() {
  const modal = document.getElementById("terminal-modal");
  if (modal) modal.classList.add("hidden");
}

function clearTerminal() {
  if (term) term.reset();
}

function runAction(action) {
  let task = "";
  let title = "";
  let extra = null;

  switch (action) {
    case "switch":
      task = "apply-config";
      title = "Application de la configuration ChomiamOS";
      break;
    case "switch-update":
      task = "update-now";
      title = "Mise à jour complète du système (Immédiate)";
      break;
    case "boot-update":
      task = "update-boot";
      title = "Mise à jour au prochain redémarrage";
      break;
    case "clean-generations":
      task = "clean-generations";
      title = "Nettoyage des générations NixOS";
      extra = document.getElementById("keep-generations")?.value || "3";
      break;
    case "clean-all":
      task = "clean-all";
      title = "Nettoyage complet du Garbage Collector";
      break;
    case "optimise":
      task = "optimise-store";
      title = "Optimisation du Nix Store";
      break;
    default:
      console.error("Action inconnue:", action);
      return;
  }

  openTerminal(title);

  setTimeout(() => {
    if (fitAddon && term) {
      fitAddon.fit();
    }
    const cols = term ? term.cols : 100;
    const rows = term ? term.rows : 24;

    invoke("start_terminal_task", { task, extra, cols, rows }).catch(err => {
      if (term) term.write(`\r\n\x1b[31mErreur : ${err}\x1b[0m\r\n`);
      const statusDot = document.getElementById("term-status-icon");
      if (statusDot) statusDot.className = "status-dot error";
    });
  }, 100);
}
