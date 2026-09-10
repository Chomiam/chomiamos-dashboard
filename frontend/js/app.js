// ==========================================================================
// ChomiamOS Dashboard - Tauri v2 + xterm.js Controller
// ==========================================================================

// Helper to reliably access Tauri IPC in Tauri v2
async function ensureTauri() {
  if (window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke) {
    return true;
  }
  for (let i = 0; i < 50; i++) {
    await new Promise(r => setTimeout(r, 20));
    if (window.__TAURI__?.core?.invoke || window.__TAURI_INTERNALS__?.invoke) {
      return true;
    }
  }
  return false;
}

async function invoke(cmd, args = {}) {
  await ensureTauri();
  if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function") {
    return window.__TAURI__.core.invoke(cmd, args);
  }
  if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === "function") {
    return window.__TAURI_INTERNALS__.invoke(cmd, args);
  }
  console.error("Tauri invoke non disponible pour:", cmd);
  throw new Error("Tauri IPC non disponible");
}

async function listen(event, cb) {
  await ensureTauri();
  if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === "function") {
    return window.__TAURI__.event.listen(event, cb);
  }
  if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.listen === "function") {
    return window.__TAURI_INTERNALS__.listen(event, cb);
  }
  console.error("Tauri listen non disponible pour:", event);
  return () => {};
}

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

// 2. Metrics Polling & Rendering
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

function setGauge(elId, percent) {
  const el = document.getElementById(elId);
  if (!el) return;
  const pct = Math.min(100, Math.max(0, percent || 0));
  // Circle radius 50 -> circumference 2 * PI * 50 = 314.15
  const offset = 314.15 * (1 - pct / 100);
  el.style.strokeDashoffset = offset;
}

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return "0 Go";
  const gb = bytes / (1024 ** 3);
  return `${gb.toFixed(1)} Go`;
}

function formatUptime(seconds) {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function renderMetrics(m) {
  // Top Navigation Bar Info
  const navKernel = document.getElementById("nav-kernel");
  if (navKernel) navKernel.textContent = m.kernel_version || "Linux";
  const navUptime = document.getElementById("nav-uptime");
  if (navUptime) navUptime.textContent = formatUptime(m.uptime_seconds || 0);

  // CPU Section
  const cpuPercent = Math.round(m.cpu_usage_percent || 0);
  const cpuPercentEl = document.getElementById("cpu-percent");
  if (cpuPercentEl) cpuPercentEl.textContent = `${cpuPercent}%`;
  setGauge("cpu-gauge", cpuPercent);

  const cpuModelEl = document.getElementById("cpu-model");
  if (cpuModelEl) cpuModelEl.textContent = m.cpu_model || "Processeur";

  const cpuFreqEl = document.getElementById("cpu-freq");
  if (cpuFreqEl) {
    cpuFreqEl.textContent = m.cpu_freq_mhz > 0 ? `${(m.cpu_freq_mhz / 1000).toFixed(2)} GHz` : "-- GHz";
  }

  // CPU Cores Meter
  const coresBarEl = document.getElementById("cpu-cores-bar");
  if (coresBarEl && Array.isArray(m.cpu_cores_usage) && m.cpu_cores_usage.length > 0) {
    coresBarEl.innerHTML = m.cpu_cores_usage.map(u => {
      const corePct = Math.min(100, Math.max(0, u || 0));
      return `
        <div class="core-bar" title="Cœur : ${Math.round(corePct)}%">
          <div class="core-bar-fill" style="transform: scaleY(${corePct / 100});"></div>
        </div>
      `;
    }).join("");
  }

  // GPU Section
  if (m.gpu) {
    const gpuNameEl = document.getElementById("gpu-name");
    if (gpuNameEl) gpuNameEl.textContent = m.gpu.name || "GPU Détecté";

    const gpuDriverEl = document.getElementById("gpu-driver");
    if (gpuDriverEl) gpuDriverEl.textContent = m.gpu.driver || "amdgpu";

    const gpuTempEl = document.getElementById("gpu-temp");
    if (gpuTempEl) {
      gpuTempEl.textContent = m.gpu.temp_celsius != null ? `${Math.round(m.gpu.temp_celsius)} °C` : "-- °C";
    }

    const gpuUsageVal = m.gpu.usage_percent != null ? Math.round(m.gpu.usage_percent) : null;
    const gpuPercentEl = document.getElementById("gpu-percent");
    if (gpuPercentEl) {
      gpuPercentEl.textContent = gpuUsageVal != null ? `${gpuUsageVal}%` : "Actif";
    }
    setGauge("gpu-gauge", gpuUsageVal != null ? gpuUsageVal : 10);

    const vramValEl = document.getElementById("vram-val");
    if (vramValEl) {
      if (m.gpu.vram_used_bytes != null && m.gpu.vram_total_bytes != null) {
        vramValEl.textContent = `${formatBytes(m.gpu.vram_used_bytes)} / ${formatBytes(m.gpu.vram_total_bytes)}`;
      } else {
        vramValEl.textContent = "-- / -- Go";
      }
    }
  }

  // RAM Section
  const ramUsed = m.ram_used_bytes || 0;
  const ramTotal = m.ram_total_bytes || 0;
  const ramPct = Math.round(m.ram_used_percent || (ramTotal > 0 ? (ramUsed / ramTotal) * 100 : 0));

  const ramNumbersEl = document.getElementById("ram-numbers");
  if (ramNumbersEl) ramNumbersEl.textContent = `${formatBytes(ramUsed)} / ${formatBytes(ramTotal)}`;

  const ramPercentEl = document.getElementById("ram-percent");
  if (ramPercentEl) ramPercentEl.textContent = `${ramPct}%`;
  setGauge("ram-gauge", ramPct);

  const ramBarEl = document.getElementById("ram-bar");
  if (ramBarEl) ramBarEl.style.width = `${ramPct}%`;

  const swapValEl = document.getElementById("swap-val");
  if (swapValEl) {
    swapValEl.textContent = `Swap: ${formatBytes(m.swap_used_bytes)} / ${formatBytes(m.swap_total_bytes)}`;
  }

  // Root Storage (/) on Overview
  const disks = m.disks || [];
  const rootDisk = disks.find(d => d.mount_point === "/") || disks[0];
  if (rootDisk) {
    const rootUsed = rootDisk.used_bytes || 0;
    const rootTotal = rootDisk.total_bytes || 0;
    const rootPct = Math.round(rootDisk.used_percent || (rootTotal > 0 ? (rootUsed / rootTotal) * 100 : 0));

    const rootNumbersEl = document.getElementById("root-disk-numbers");
    if (rootNumbersEl) rootNumbersEl.textContent = `${formatBytes(rootUsed)} / ${formatBytes(rootTotal)}`;

    const rootPercentEl = document.getElementById("root-disk-percent");
    if (rootPercentEl) rootPercentEl.textContent = `${rootPct}%`;
    setGauge("disk-gauge", rootPct);

    const rootBarEl = document.getElementById("root-disk-bar");
    if (rootBarEl) rootBarEl.style.width = `${rootPct}%`;
  }

  // Host metadata in action banner
  const metaHostEl = document.getElementById("meta-host");
  if (metaHostEl) metaHostEl.textContent = m.hostname || "chomiamos";

  // Tab 2: Disks List
  renderDisksList(disks);
}

function renderDisksList(disks) {
  const container = document.getElementById("disks-list");
  if (!container) return;

  if (!disks || disks.length === 0) {
    container.innerHTML = `<div class="card" style="grid-column: 1 / -1; text-align: center; color: var(--subtext0);">Aucun volume monté détecté</div>`;
    return;
  }

  container.innerHTML = disks.map(d => {
    const pct = Math.round(d.used_percent);
    const isCritical = pct > 85;
    return `
      <div class="card" style="display: flex; flex-direction: column; gap: 10px;">
        <div style="display: flex; justify-content: space-between; align-items: center;">
          <div style="display: flex; align-items: center; gap: 10px;">
            <span style="font-size: 24px;">💽</span>
            <div>
              <strong style="font-size: 14px; color: var(--text);">${d.name || d.mount_point}</strong>
              <div style="font-size: 11px; color: var(--subtext0);">${d.mount_point} (${d.file_system || "ext4"})</div>
            </div>
          </div>
          <span style="font-family: 'JetBrains Mono', monospace; font-weight: 700; color: ${isCritical ? "var(--red)" : "var(--peach)"};">${pct}%</span>
        </div>
        <div class="progress-bar-wrap" style="margin: 4px 0;">
          <div class="progress-bar-inner disk-fill" style="width: ${pct}%; background-color: ${isCritical ? "var(--red)" : "var(--peach)"};"></div>
        </div>
        <div style="display: flex; justify-content: space-between; font-size: 11px; color: var(--subtext0); font-family: 'JetBrains Mono', monospace;">
          <span>${formatBytes(d.used_bytes)} utilisés</span>
          <span>${formatBytes(d.total_bytes)} total</span>
        </div>
      </div>
    `;
  }).join("");
}

// 3. Generations Management
async function loadGenerations() {
  const tbody = document.getElementById("generations-tbody");
  const countEl = document.getElementById("generations-count-val");
  const storeSizeEl = document.getElementById("store-size-val");
  const metaCommitEl = document.getElementById("meta-commit");

  try {
    const summary = await invoke("get_generations");
    if (!summary || !summary.generations) {
      if (tbody) tbody.innerHTML = `<tr><td colspan="5" style="text-align: center; color: var(--red); padding: 20px;">Impossible de charger les générations système</td></tr>`;
      return;
    }

    if (countEl) countEl.textContent = `${summary.count} génération${summary.count > 1 ? "s" : ""}`;
    if (storeSizeEl) storeSizeEl.textContent = summary.store_size || "N/A";

    const activeGen = summary.generations.find(g => g.current);
    if (metaCommitEl && activeGen) {
      metaCommitEl.textContent = `Image active : #${activeGen.id} (${activeGen.nixos_version})`;
    }

    if (!tbody) return;

    if (summary.generations.length === 0) {
      tbody.innerHTML = `<tr><td colspan="5" style="text-align: center; color: var(--subtext0); padding: 20px;">Aucune image de génération trouvée</td></tr>`;
      return;
    }

    tbody.innerHTML = summary.generations.map(g => `
      <tr style="${g.current ? "background-color: rgba(166, 227, 161, 0.08); font-weight: 500;" : ""}">
        <td style="font-family: 'JetBrains Mono', monospace; font-weight: 700;">#${g.id}</td>
        <td>${g.date}</td>
        <td><span class="metric-tag" style="background: var(--surface0);">${g.nixos_version}</span></td>
        <td style="font-family: 'JetBrains Mono', monospace; color: var(--subtext1);">${g.kernel}</td>
        <td>
          <span class="badge-gen ${g.current ? "badge-active" : "badge-inactive"}">
            ${g.current ? "● Active" : "Archivée"}
          </span>
        </td>
      </tr>
    `).join("");
  } catch (err) {
    console.error("Erreur générations:", err);
    if (tbody) tbody.innerHTML = `<tr><td colspan="5" style="text-align: center; color: var(--red); padding: 20px;">Erreur : ${err}</td></tr>`;
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
  setRadioVal("desktop_env", c.desktop_env);
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

  const deRadio = document.querySelector('input[name="desktop_env"]:checked');
  if (deRadio) currentConfig.desktop_env = deRadio.value;

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
  const deWarning = document.getElementById("de-change-warning");
  const initialCfg = initialConfigStr ? JSON.parse(initialConfigStr) : null;
  const isDirty = JSON.stringify(currentConfig) !== initialConfigStr;

  if (dirtyBadge) {
    if (isDirty) {
      dirtyBadge.classList.remove("hidden");
    } else {
      dirtyBadge.classList.add("hidden");
    }
  }

  if (deWarning && initialCfg && currentConfig) {
    if (initialCfg.desktop_env !== currentConfig.desktop_env) {
      deWarning.style.display = "flex";
    } else {
      deWarning.style.display = "none";
    }
  }
}

function resetConfig() {
  if (initialConfigStr) {
    currentConfig = JSON.parse(initialConfigStr);
    populateConfigUI(currentConfig);
  }
}

async function saveConfig(andApply = false, atBoot = false) {
  readConfigFromUI();

  const initialCfg = initialConfigStr ? JSON.parse(initialConfigStr) : null;
  const deChanged = initialCfg && initialCfg.desktop_env !== currentConfig.desktop_env;

  if (andApply && !atBoot && deChanged) {
    const confirmSwitch = confirm(
      "⚠️ Attention : Vous changez d'environnement de bureau (" + (initialCfg?.desktop_env || "") + " → " + currentConfig.desktop_env + ").\n\n" +
      "L'application en direct ('nh os switch') va relancer le gestionnaire d'affichage et risque de fermer brutalement votre session graphique.\n\n" +
      "Voulez-vous plutôt l'appliquer en toute sécurité au prochain redémarrage ('nh os boot') ?"
    );
    if (confirmSwitch) {
      atBoot = true;
    }
  }

  try {
    await invoke("save_chomiamos_config", { config: currentConfig });
    initialConfigStr = JSON.stringify(currentConfig);
    checkDirtyState();

    if (andApply) {
      if (atBoot) {
        runAction("boot-apply");
      } else {
        runAction("switch");
      }
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

// Keyboard Lock Indicators (Caps Lock / Num Lock)
let lockStateInterval = null;

function setLockStates(capsOn, numOn) {
  const capsBadge = document.getElementById("kb-caps-badge");
  const numBadge = document.getElementById("kb-num-badge");

  if (capsBadge) {
    if (capsOn) {
      capsBadge.classList.add("caps-active");
      capsBadge.textContent = "⇪ MAJ : ACTIF";
      capsBadge.setAttribute("title", "Attention : Verrouillage Majuscule activé");
    } else {
      capsBadge.classList.remove("caps-active");
      capsBadge.textContent = "⇪ MAJ";
      capsBadge.setAttribute("title", "Verrouillage Majuscule inactif");
    }
  }

  if (numBadge) {
    if (numOn) {
      numBadge.classList.add("num-active");
      numBadge.textContent = "🔢 NUM : ON";
      numBadge.setAttribute("title", "Pavé numérique activé");
    } else {
      numBadge.classList.remove("num-active");
      numBadge.textContent = "🔢 NUM : OFF";
      numBadge.setAttribute("title", "Attention : Pavé numérique désactivé");
    }
  }
}

async function refreshLockStateFromHardware() {
  try {
    const state = await invoke("get_keyboard_lock_state");
    if (state) {
      setLockStates(state.caps_lock, state.num_lock);
    }
  } catch (err) {
    // Non-fatal
  }
}

function handleKeyModifierEvent(e) {
  if (!e || typeof e.getModifierState !== "function") return;
  const capsOn = e.getModifierState("CapsLock");
  const numOn = e.getModifierState("NumLock");
  setLockStates(capsOn, numOn);
}

window.addEventListener("keydown", handleKeyModifierEvent, true);
window.addEventListener("keyup", handleKeyModifierEvent, true);

function openTerminal(title = "Exécution en direct") {
  const modal = document.getElementById("terminal-modal");
  const titleEl = document.getElementById("term-title");
  const statusDot = document.getElementById("term-status-icon");
  const statusText = document.getElementById("term-status-text");

  if (titleEl) titleEl.textContent = title;
  if (statusDot) statusDot.className = "status-dot running";
  if (statusText) statusText.textContent = "Exécution en cours...";

  modal.classList.remove("hidden");

  refreshLockStateFromHardware();
  if (lockStateInterval) clearInterval(lockStateInterval);
  lockStateInterval = setInterval(refreshLockStateFromHardware, 1000);

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
  if (lockStateInterval) {
    clearInterval(lockStateInterval);
    lockStateInterval = null;
  }
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
      title = "Application de la configuration ChomiamOS (Live)";
      break;
    case "boot-apply":
      task = "boot-config";
      title = "Application au prochain redémarrage (nh os boot)";
      break;
    case "sync-github":
      task = "sync-github";
      title = "Synchronisation NixOS depuis GitHub";
      break;
    case "boot-sync-github":
      task = "boot-sync-github";
      title = "Synchronisation GitHub au prochain reboot (nh os boot)";
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
