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
let lastExecutedTask = "";

document.addEventListener("DOMContentLoaded", () => {
  initTabs();
  startMetricsPolling();
  loadGenerations();
  loadConfig();
  initTerminal();
  loadStorageDevices();
  loadCustomPackages();
  initPackageSearch();
  checkForUpdates();
  setInterval(checkForUpdates, 30000);
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
        if (targetId === "tab-disks") {
          loadStorageDevices();
        } else if (targetId === "tab-packages") {
          loadCustomPackages();
        }
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
      if (tbody) tbody.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--red); padding: 20px;">Impossible de charger les générations système</td></tr>`;
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
      tbody.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--subtext0); padding: 20px;">Aucune image de génération trouvée</td></tr>`;
      return;
    }

    tbody.innerHTML = summary.generations.map(g => `
      <tr class="${g.current ? 'gen-row-active' : 'gen-row'}" data-gen-id="${g.id}" data-gen-current="${g.current}">
        <td class="gen-checkbox-col">
          ${g.current
            ? '<input type="checkbox" disabled title="Impossible de sélectionner la génération active" class="gen-checkbox-disabled">'
            : `<input type="checkbox" class="gen-checkbox" value="${g.id}" onchange="updateGenActionBar()" title="Sélectionner la génération #${g.id}">`
          }
        </td>
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

    // Reset action bar on reload
    updateGenActionBar();
  } catch (err) {
    console.error("Erreur générations:", err);
    if (tbody) tbody.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--red); padding: 20px;">Erreur : ${err}</td></tr>`;
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
  const bar = document.getElementById("config-floating-bar");
  const initialCfg = initialConfigStr ? JSON.parse(initialConfigStr) : null;
  const isDirty = initialConfigStr && JSON.stringify(currentConfig) !== initialConfigStr;

  if (!bar) return;

  if (!isDirty) {
    bar.classList.add("hidden");
    bar.classList.remove("visible", "de-change");
    return;
  }

  bar.classList.remove("hidden");
  bar.classList.add("visible");

  const deChanged = initialCfg && currentConfig && (initialCfg.desktop_env !== currentConfig.desktop_env);
  const iconEl = document.getElementById("config-bar-icon");
  const titleEl = document.getElementById("config-bar-title");
  const descEl = document.getElementById("config-bar-desc");
  const applyBtn = document.getElementById("config-bar-apply-btn");
  const applyIcon = document.getElementById("config-bar-apply-icon");
  const applyText = document.getElementById("config-bar-apply-text");

  if (deChanged) {
    bar.classList.add("de-change");
    if (iconEl) iconEl.textContent = "🔄";
    if (titleEl) titleEl.textContent = "Changement de Bureau Détecté";
    if (descEl) {
      const oldDE = (initialCfg.desktop_env || "inconnu").toUpperCase();
      const newDE = (currentConfig.desktop_env || "").toUpperCase();
      descEl.innerHTML = "Bascule de <strong>" + oldDE + "</strong> vers <strong>" + newDE + "</strong>. Application sécurisée au prochain redémarrage (<code>nh os boot</code>).";
    }
    if (applyBtn) {
      applyBtn.className = "btn btn-warning";
    }
    if (applyIcon) applyIcon.textContent = "🔄";
    if (applyText) applyText.textContent = "Valider & Reboot (nh os boot)";
  } else {
    bar.classList.remove("de-change");
    if (iconEl) iconEl.textContent = "⚡";
    if (titleEl) titleEl.textContent = "Modifications non appliquées";
    if (descEl) {
      descEl.innerHTML = "Prêt à déployer vos modifications immédiatement (<code>nh os switch</code>).";
    }
    if (applyBtn) {
      applyBtn.className = "btn btn-primary";
    }
    if (applyIcon) applyIcon.textContent = "🚀";
    if (applyText) applyText.textContent = "Valider les modifications (nh os switch)";
  }
}

function resetConfig() {
  if (initialConfigStr) {
    currentConfig = JSON.parse(initialConfigStr);
    populateConfigUI(currentConfig);
    checkDirtyState();
  }
}

async function submitConfigDeploy() {
  readConfigFromUI();
  const initialCfg = initialConfigStr ? JSON.parse(initialConfigStr) : null;
  const deChanged = initialCfg && currentConfig && (initialCfg.desktop_env !== currentConfig.desktop_env);

  if (deChanged) {
    await saveConfig(true, true);
  } else {
    await saveConfig(true, false);
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
    const restartBtn = document.getElementById("term-restart-app-btn");

    if (term) {
      if (exitCode === 0) {
        term.write("\r\n\x1b[1;32m✔ Opération terminée avec succès !\x1b[0m\r\n");
        if (lastExecutedTask && (lastExecutedTask.includes("update") || lastExecutedTask.includes("sync") || lastExecutedTask.includes("config") || lastExecutedTask.includes("switch") || lastExecutedTask.includes("boot"))) {
          term.write("\x1b[1;36m💡 Astuce : Cliquez sur « Relancer le Dashboard » en haut à droite pour charger la nouvelle version.\x1b[0m\r\n");
        }
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

    if (exitCode === 0 && restartBtn) {
      restartBtn.classList.remove("hidden");
    }

    loadGenerations();
    checkForUpdates();
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

  const restartBtn = document.getElementById("term-restart-app-btn");
  if (restartBtn) restartBtn.classList.add("hidden");

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

function runTerminalTask(task, title, extra = null) {
  lastExecutedTask = task;
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

  runTerminalTask(task, title, extra);
}


// ==========================================================================
// 6. Storage & Disk Management Controller
// ==========================================================================

let currentStorageDevices = [];
let currentUsername = "chomiam";
let selectedMountPartition = null;
let selectedFormatPartition = null;
let currentMountPreset = "mnt";
let currentFormatFs = "btrfs";

function escapeHtml(str) {
  if (!str) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

async function loadStorageDevices() {
  const container = document.getElementById("storage-devices-container");
  if (!container) return;

  try {
    try {
      const u = await invoke("get_current_user");
      if (u) currentUsername = u;
    } catch (_) {}

    const devices = await invoke("get_storage_devices");
    currentStorageDevices = devices || [];
    renderStorageDevices(currentStorageDevices);
  } catch (err) {
    console.error("Erreur lors de la récupération des disques :", err);
    container.innerHTML = `
      <div class="card" style="text-align: center; color: var(--red); padding: 36px;">
        <span style="font-size: 32px; display: block; margin-bottom: 8px;">⚠️</span>
        <strong>Erreur de détection des disques</strong>
        <p style="color: var(--subtext0); margin-top: 6px; font-size: 13px;">${err}</p>
        <button class="btn btn-outline" onclick="loadStorageDevices()" style="margin-top: 14px;">Réessayer</button>
      </div>
    `;
  }
}

function renderStorageDevices(devices) {
  const container = document.getElementById("storage-devices-container");
  if (!container) return;

  if (!devices || devices.length === 0) {
    container.innerHTML = `
      <div class="card" style="text-align: center; color: var(--subtext0); padding: 36px;">
        <span style="font-size: 32px; display: block; margin-bottom: 8px;">📭</span>
        <p>Aucun disque de stockage physique secondaire ou amovible détecté.</p>
      </div>
    `;
    return;
  }

  let html = "";

  devices.forEach(dev => {
    const modelText = dev.model ? escapeHtml(dev.model) : "Disque de stockage";
    const devName = escapeHtml(dev.name);
    const devPath = escapeHtml(dev.path);
    const devSize = escapeHtml(dev.size);

    html += `
      <div class="disk-card">
        <div class="disk-card-header">
          <div class="disk-header-left">
            <span class="disk-icon">💽</span>
            <div>
              <div class="disk-title-row">
                <h3 class="disk-name">${devName}</h3>
                <span class="disk-path">${devPath}</span>
              </div>
              <p class="disk-model">${modelText} — <strong style="color: var(--text);">${devSize}</strong></p>
            </div>
          </div>
        </div>

        <div class="partitions-table-wrap">
          <table class="partitions-table">
            <thead>
              <tr>
                <th>Partition</th>
                <th>Système de Fichiers</th>
                <th>Label / Nom</th>
                <th>Taille</th>
                <th>État de Montage</th>
                <th style="text-align: right;">Actions</th>
              </tr>
            </thead>
            <tbody>
    `;

    if (!dev.partitions || dev.partitions.length === 0) {
      html += `
        <tr>
          <td colspan="6" style="text-align: center; color: var(--subtext0); padding: 18px;">
            Aucune partition détectée sur ce périphérique (disque non initialisé).
          </td>
        </tr>
      `;
    } else {
      dev.partitions.forEach(p => {
        const pName = escapeHtml(p.name);
        const pPath = escapeHtml(p.path);
        const pFs = p.fstype ? escapeHtml(p.fstype) : "--";
        const pLabel = p.label ? escapeHtml(p.label) : "--";
        const pSize = escapeHtml(p.size);

        let statusBadge = "";
        let isSystemProtected = p.is_root || p.is_boot || p.is_swap;

        if (p.is_root) {
          statusBadge = `<span class="badge badge-accent">🔒 Système NixOS (/)</span>`;
        } else if (p.is_boot) {
          statusBadge = `<span class="badge badge-warning">⚡ Boot EFI (/boot)</span>`;
        } else if (p.is_swap) {
          statusBadge = `<span class="badge badge-muted">🔄 Swap</span>`;
        } else if (p.is_persistent_nix && p.persistent_mount_path) {
          statusBadge = `<span class="badge badge-success" title="Montage permanent déclaré dans NixOS">🛡️ Fixe : ${escapeHtml(p.persistent_mount_path)}</span>`;
        } else if (p.mountpoints && p.mountpoints.length > 0) {
          const firstMnt = escapeHtml(p.mountpoints[0]);
          statusBadge = `<span class="badge badge-info" title="${escapeHtml(p.mountpoints.join(', '))}">📍 Monté : ${firstMnt}</span>`;
        } else {
          statusBadge = `<span class="badge badge-muted">Non monté</span>`;
        }

        let actionsHtml = "";
        const jsonPart = JSON.stringify(p).replace(/"/g, '&quot;');

        if (isSystemProtected) {
          actionsHtml = `<span class="action-locked">Système protégé 🔒</span>`;
        } else {
          const mountPath = p.persistent_mount_path || (p.mountpoints && p.mountpoints.length > 0 ? p.mountpoints[0] : null);

          let openBtn = "";
          let unmountBtn = "";
          let mountBtn = "";
          let formatBtn = "";

          if (mountPath) {
            openBtn = `
              <button class="btn btn-sm btn-outline" title="Ouvrir dans l'explorateur" onclick="openFileManager('${escapeHtml(mountPath)}')">
                📂 Ouvrir
              </button>
            `;
            const pUuidArg = p.uuid ? `'${p.uuid}'` : "null";
            unmountBtn = `
              <button class="btn btn-sm btn-outline" title="Démonter la partition" onclick="unmountDisk('${escapeHtml(mountPath)}', ${pUuidArg})">
                ⏏️ Démonter
              </button>
            `;
          }

          if (!p.is_persistent_nix) {
            mountBtn = `
              <button class="btn btn-sm btn-primary" title="Monter ce disque en dur de manière permanente" onclick="openMountModal(${jsonPart})">
                🔗 Monter en dur
              </button>
            `;
          }

          formatBtn = `
            <button class="btn btn-sm btn-danger-outline" title="Formater cette partition" onclick="openFormatModal(${jsonPart})">
              🧹 Formater
            </button>
          `;

          actionsHtml = `
            <div class="action-btns-wrap">
              ${openBtn}
              ${mountBtn}
              ${unmountBtn}
              ${formatBtn}
            </div>
          `;
        }

        html += `
          <tr class="partition-row">
            <td>
              <div class="part-name-cell">
                <span class="part-icon">📁</span>
                <div>
                  <strong>${pName}</strong>
                  <div class="part-subpath">${pPath}</div>
                </div>
              </div>
            </td>
            <td><span class="fs-badge">${pFs}</span></td>
            <td>${pLabel}</td>
            <td><strong>${pSize}</strong></td>
            <td>${statusBadge}</td>
            <td style="text-align: right;">${actionsHtml}</td>
          </tr>
        `;
      });
    }

    html += `
            </tbody>
          </table>
        </div>
      </div>
    `;
  });

  container.innerHTML = html;
}

function openMountModal(part) {
  selectedMountPartition = part;
  const modal = document.getElementById("mount-modal");
  if (!modal) return;

  const devEl = document.getElementById("mount-modal-dev");
  const sizeEl = document.getElementById("mount-modal-size");
  const fsEl = document.getElementById("mount-modal-fs");

  if (devEl) devEl.textContent = part.path;
  if (sizeEl) sizeEl.textContent = part.size;
  if (fsEl) fsEl.textContent = part.fstype || "auto";

  let defaultName = "Games";
  if (part.label && part.label.trim().length > 0) {
    defaultName = part.label.trim().replace(/[^a-zA-Z0-9_\-]/g, "");
  }
  const inputEl = document.getElementById("mount-name-input");
  if (inputEl) inputEl.value = defaultName || "Games";

  selectMountPreset("mnt");
  modal.classList.remove("hidden");
}

function closeMountModal() {
  const modal = document.getElementById("mount-modal");
  if (modal) modal.classList.add("hidden");
  selectedMountPartition = null;
}

function selectMountPreset(preset) {
  currentMountPreset = preset;

  ["mnt", "home", "custom"].forEach(p => {
    const card = document.getElementById(`preset-card-${p}`);
    const radio = card ? card.querySelector("input") : null;
    if (card) {
      if (p === preset) {
        card.classList.add("active");
        if (radio) radio.checked = true;
      } else {
        card.classList.remove("active");
        if (radio) radio.checked = false;
      }
    }
  });

  const labelEl = document.getElementById("mount-name-label");
  const inputEl = document.getElementById("mount-name-input");

  if (preset === "custom") {
    if (labelEl) labelEl.textContent = "Chemin absolu personnalisé :";
    if (inputEl) {
      inputEl.placeholder = "/chemin/personnalise";
      if (!inputEl.value.startsWith("/")) {
        inputEl.value = "/mnt/" + (inputEl.value || "Games");
      }
    }
  } else if (preset === "home") {
    if (labelEl) labelEl.textContent = `Nom du sous-dossier dans /home/${currentUsername}/ :`;
    if (inputEl) {
      inputEl.placeholder = "ex: Games, Stockage";
      inputEl.value = inputEl.value.replace(/^\/.*?\//, "").replace(/^\/+/, "") || "Games";
    }
  } else {
    if (labelEl) labelEl.textContent = "Nom du dossier dans /mnt/ :";
    if (inputEl) {
      inputEl.placeholder = "ex: Games, Stockage, SSD2";
      inputEl.value = inputEl.value.replace(/^\/.*?\//, "").replace(/^\/+/, "") || "Games";
    }
  }

  updateMountPreview();
}

function updateMountPreview() {
  const previewEl = document.getElementById("mount-preview-path");
  const inputEl = document.getElementById("mount-name-input");
  if (!previewEl || !inputEl) return;

  const val = inputEl.value.trim();

  let finalPath = "";
  if (currentMountPreset === "custom") {
    finalPath = val.startsWith("/") ? val : "/" + val;
  } else if (currentMountPreset === "home") {
    const cleanSub = val.replace(/^\/+/, "");
    finalPath = `/home/${currentUsername}/${cleanSub}`;
  } else {
    const cleanSub = val.replace(/^\/+/, "");
    finalPath = `/mnt/${cleanSub}`;
  }

  previewEl.textContent = finalPath;
}

async function submitMount() {
  if (!selectedMountPartition) return;

  if (!selectedMountPartition.uuid) {
    alert("Impossible de monter ce disque en dur : Aucun identifiant UUID trouvé pour cette partition. Formatez-la d'abord si elle est neuve.");
    return;
  }

  const previewEl = document.getElementById("mount-preview-path");
  const mountPoint = previewEl ? previewEl.textContent.trim() : "";

  if (!mountPoint || !mountPoint.startsWith("/")) {
    alert("Veuillez saisir un chemin de montage valide débutant par '/'.");
    return;
  }

  const btn = document.getElementById("btn-confirm-mount");
  const originalText = btn ? btn.innerHTML : "Valider";
  if (btn) {
    btn.disabled = true;
    btn.innerHTML = "<span>⏳</span> Montage en cours...";
  }

  try {
    const fsType = selectedMountPartition.fstype || "auto";
    const res = await invoke("mount_storage_device", {
      uuid: selectedMountPartition.uuid,
      mountPoint: mountPoint,
      fsType: fsType
    });

    closeMountModal();
    await loadStorageDevices();
    alert(res || "Disque monté avec succès de manière permanente !");
  } catch (err) {
    alert("Erreur lors du montage : " + err);
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.innerHTML = originalText;
    }
  }
}

async function unmountDisk(mountPoint, uuid) {
  if (!confirm(`Voulez-vous vraiment démonter le disque monté sur "${mountPoint}" ?\n\nS'il s'agit d'un montage permanent NixOS, il sera retiré de mount.nix.`)) {
    return;
  }

  try {
    const res = await invoke("unmount_storage_device", {
      mountPoint: mountPoint,
      uuid: uuid || null
    });
    await loadStorageDevices();
    alert(res || "Disque démonté avec succès.");
  } catch (err) {
    alert("Erreur lors du démontage : " + err);
  }
}

function openFormatModal(part) {
  selectedFormatPartition = part;
  const modal = document.getElementById("format-modal");
  if (!modal) return;

  const devEl = document.getElementById("format-modal-dev");
  const sizeEl = document.getElementById("format-modal-size");
  const fsEl = document.getElementById("format-modal-current-fs");

  if (devEl) devEl.textContent = part.path;
  if (sizeEl) sizeEl.textContent = part.size;
  if (fsEl) fsEl.textContent = `Actuel : ${part.fstype || "Inconnu"}`;

  let defaultLabel = "Games";
  if (part.label && part.label.trim().length > 0) {
    defaultLabel = part.label.trim().replace(/[^a-zA-Z0-9_\-]/g, "");
  }
  const labelInput = document.getElementById("format-label-input");
  if (labelInput) labelInput.value = defaultLabel || "Games";

  const confirmCheck = document.getElementById("format-confirm-check");
  if (confirmCheck) confirmCheck.checked = false;

  const btn = document.getElementById("btn-confirm-format");
  if (btn) btn.disabled = true;

  selectFormatFs("btrfs");
  modal.classList.remove("hidden");
}

function closeFormatModal() {
  const modal = document.getElementById("format-modal");
  if (modal) modal.classList.add("hidden");
  selectedFormatPartition = null;
}

function selectFormatFs(fs) {
  currentFormatFs = fs;
  ["btrfs", "ext4"].forEach(f => {
    const card = document.getElementById(`fs-card-${f}`);
    const radio = card ? card.querySelector("input") : null;
    if (card) {
      if (f === fs) {
        card.classList.add("active");
        if (radio) radio.checked = true;
      } else {
        card.classList.remove("active");
        if (radio) radio.checked = false;
      }
    }
  });
}

function toggleFormatSubmitButton() {
  const confirmCheck = document.getElementById("format-confirm-check");
  const btn = document.getElementById("btn-confirm-format");
  if (btn && confirmCheck) {
    btn.disabled = !confirmCheck.checked;
  }
}

async function submitFormat() {
  if (!selectedFormatPartition) return;

  const confirmCheck = document.getElementById("format-confirm-check");
  if (!confirmCheck || !confirmCheck.checked) {
    alert("Veuillez cocher la case de confirmation pour continuer.");
    return;
  }

  const labelInput = document.getElementById("format-label-input");
  const label = labelInput ? labelInput.value.trim() : "Storage";

  const btn = document.getElementById("btn-confirm-format");
  const originalText = btn ? btn.innerHTML : "Formater";
  if (btn) {
    btn.disabled = true;
    btn.innerHTML = "<span>⏳</span> Formatage en cours...";
  }

  try {
    const res = await invoke("format_storage_device", {
      devicePath: selectedFormatPartition.path,
      fsType: currentFormatFs,
      label: label
    });

    closeFormatModal();
    await loadStorageDevices();
    alert(res || "Partition formatée avec succès !");
  } catch (err) {
    alert("Erreur lors du formatage : " + err);
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.innerHTML = originalText;
    }
  }
}

async function openFileManager(path) {
  try {
    await invoke("open_in_file_manager", { path });
  } catch (err) {
    alert("Impossible d'ouvrir l'explorateur de fichiers : " + err);
  }
}

// Window bindings for HTML event handlers
window.loadStorageDevices = loadStorageDevices;
window.openMountModal = openMountModal;
window.closeMountModal = closeMountModal;
window.selectMountPreset = selectMountPreset;
window.updateMountPreview = updateMountPreview;
window.submitMount = submitMount;
window.unmountDisk = unmountDisk;
window.openFormatModal = openFormatModal;
window.closeFormatModal = closeFormatModal;
window.selectFormatFs = selectFormatFs;
window.toggleFormatSubmitButton = toggleFormatSubmitButton;
window.submitFormat = submitFormat;
window.openFileManager = openFileManager;

// ==========================================================================
// 3b. Generations Selection & Management
// ==========================================================================

function getSelectedGenIds() {
  return Array.from(document.querySelectorAll(".gen-checkbox:checked")).map(cb => parseInt(cb.value));
}

function updateGenActionBar() {
  const selected = getSelectedGenIds();
  const bar = document.getElementById("gen-action-bar");
  const countEl = document.getElementById("gen-selected-count");
  const switchBtn = document.getElementById("btn-gen-switch");
  const selectAll = document.getElementById("gen-select-all");

  if (!bar) return;

  if (selected.length > 0) {
    bar.classList.remove("hidden");
    if (countEl) countEl.textContent = `${selected.length} sélectionnée${selected.length > 1 ? "s" : ""}`;

    // Bouton "Rebooter" visible uniquement si 1 seule sélectionnée
    if (switchBtn) {
      if (selected.length === 1) {
        switchBtn.classList.remove("hidden");
        switchBtn.innerHTML = `<span class="btn-icon">🔄</span> Rebooter sur l'image #${selected[0]}`;
      } else {
        switchBtn.classList.add("hidden");
      }
    }
  } else {
    bar.classList.add("hidden");
  }

  // Update "select all" checkbox state
  const allCheckboxes = document.querySelectorAll(".gen-checkbox");
  const allChecked = allCheckboxes.length > 0 && Array.from(allCheckboxes).every(cb => cb.checked);
  if (selectAll) selectAll.checked = allChecked;
}

function toggleSelectAllGens() {
  const selectAll = document.getElementById("gen-select-all");
  const checkboxes = document.querySelectorAll(".gen-checkbox");
  checkboxes.forEach(cb => { cb.checked = selectAll.checked; });
  updateGenActionBar();
}

function clearGenSelection() {
  const checkboxes = document.querySelectorAll(".gen-checkbox");
  checkboxes.forEach(cb => { cb.checked = false; });
  const selectAll = document.getElementById("gen-select-all");
  if (selectAll) selectAll.checked = false;
  updateGenActionBar();
}

function deleteSelectedGenerations() {
  const ids = getSelectedGenIds();
  if (ids.length === 0) return;

  const plural = ids.length > 1 ? "s" : "";
  const idsStr = ids.map(id => `#${id}`).join(", ");
  if (!confirm(`Voulez-vous vraiment supprimer ${ids.length} génération${plural} ?\n\n${idsStr}\n\n⚠️ Cette action est irréversible.`)) {
    return;
  }

  const idsParam = ids.join(" ");
  runTerminalTask(
    "delete-generations",
    `Suppression de ${ids.length} génération${plural} (${idsStr})`,
    idsParam
  );
  clearGenSelection();
}

function switchToSelectedGeneration() {
  const ids = getSelectedGenIds();
  if (ids.length !== 1) return;

  const id = ids[0];
  if (!confirm(`Voulez-vous rebooter sur la génération #${id} ?\n\nLa génération sera activée dans le bootloader au prochain redémarrage du système.`)) {
    return;
  }

  runTerminalTask(
    "switch-generation",
    `Bascule sur la génération #${id} pour le prochain reboot`,
    String(id)
  );
  clearGenSelection();
}

window.resetConfig = resetConfig;
window.submitConfigDeploy = submitConfigDeploy;
window.updateGenActionBar = updateGenActionBar;
window.toggleSelectAllGens = toggleSelectAllGens;
window.clearGenSelection = clearGenSelection;
window.deleteSelectedGenerations = deleteSelectedGenerations;
window.switchToSelectedGeneration = switchToSelectedGeneration;


// ==========================================================================
// 7. Update Checker Controller (Auto-check on startup)
// ==========================================================================

async function checkForUpdates() {
  try {
    const status = await invoke("check_system_updates");
    if (!status) return;

    const metaCommit = document.getElementById("meta-commit");
    if (metaCommit && status.github_local_commit) {
      metaCommit.textContent = "Commit: " + status.github_local_commit;
    }

    const alertBanner = document.getElementById("system-update-alert");
    const alertIcon = document.getElementById("update-suggestion-icon");
    const alertTitle = document.getElementById("update-suggestion-title");
    const alertDesc = document.getElementById("update-suggestion-desc");
    const alertBtn = document.getElementById("update-suggestion-btn");
    const navIndicator = document.getElementById("nav-update-indicator");

    if (status.github_has_updates) {
      if (alertBanner) {
        alertBanner.classList.remove("hidden");
        alertBanner.classList.add("is-github");
        alertBanner.classList.remove("is-dashboard");
      }
      if (alertIcon) alertIcon.textContent = "🐙";
      if (alertTitle) alertTitle.textContent = "Mises à jour GitHub disponibles !";
      if (alertDesc) {
        const remoteSha = status.github_remote_commit || "origin/main";
        alertDesc.innerHTML = "De nouvelles modifications sont disponibles sur GitHub (distant: <code>" + remoteSha + "</code>). Synchronisez votre système pour en bénéficier.";
      }
      if (alertBtn) {
        alertBtn.innerHTML = "<span>🐙</span> Synchroniser avec GitHub";
        alertBtn.onclick = () => runAction("sync-github");
      }
      if (navIndicator) {
        navIndicator.classList.remove("hidden");
        navIndicator.innerHTML = "<span>🐙</span> <span>Sync GitHub</span>";
      }
    } else if (status.dashboard_has_updates) {
      if (alertBanner) {
        alertBanner.classList.remove("hidden");
        alertBanner.classList.add("is-dashboard");
        alertBanner.classList.remove("is-github");
      }
      if (alertIcon) alertIcon.textContent = "✨";
      if (alertTitle) alertTitle.textContent = "Nouvelle version du Dashboard disponible !";
      if (alertDesc) {
        alertDesc.innerHTML = "Une nouvelle mise à jour du tableau de bord a été publiée. Mettez à jour vos paquets pour l'installer.";
      }
      if (alertBtn) {
        alertBtn.innerHTML = "<span>📦</span> Mettre à jour les paquets";
        alertBtn.onclick = () => runAction("switch-update");
      }
      if (navIndicator) {
        navIndicator.classList.remove("hidden");
        navIndicator.innerHTML = "<span>✨</span> <span>MAJ Dashboard</span>";
      }
    } else if (status.system_needs_switch) {
      if (alertBanner) {
        alertBanner.classList.remove("hidden");
        alertBanner.classList.add("is-github");
        alertBanner.classList.remove("is-dashboard");
      }
      if (alertIcon) alertIcon.textContent = "⚡";
      if (alertTitle) alertTitle.textContent = "Mise à jour prête à être déployée !";
      if (alertDesc) {
        alertDesc.innerHTML = "Une nouvelle version du tableau de bord ou de la configuration est prête. Déployez-la pour l'activer sur votre session.";
      }
      if (alertBtn) {
        alertBtn.innerHTML = "<span>⚡</span> Déployer (nh os switch)";
        alertBtn.onclick = () => runAction("switch");
      }
      if (navIndicator) {
        navIndicator.classList.remove("hidden");
        navIndicator.innerHTML = "<span>⚡</span> <span>Déployer MAJ</span>";
        navIndicator.onclick = () => runAction("switch");
      }
    } else {
      if (alertBanner) alertBanner.classList.add("hidden");
      if (navIndicator) navIndicator.classList.add("hidden");
    }
  } catch (err) {
    console.warn("Vérification des mises à jour ignorée:", err);
  }
}

function dismissUpdateAlert() {
  const alertBanner = document.getElementById("system-update-alert");
  if (alertBanner) alertBanner.classList.add("hidden");
}

async function restartDashboard() {
  try {
    await invoke("restart_dashboard");
  } catch (err) {
    console.warn("restart_dashboard invoke a échoué, rechargement web:", err);
    window.location.reload();
  }
}

window.checkForUpdates = checkForUpdates;
window.dismissUpdateAlert = dismissUpdateAlert;
window.restartDashboard = restartDashboard;

// =========================================================================
// ⚙️ Settings & GitHub Token Management
// =========================================================================
async function openSettingsModal() {
  const modal = document.getElementById("settings-modal");
  if (!modal) return;

  const input = document.getElementById("github-token-input");
  const badge = document.getElementById("github-token-status-badge");
  const deleteBtn = document.getElementById("btn-delete-token");
  const alertEl = document.getElementById("github-token-alert");

  if (alertEl) {
    alertEl.className = "hidden";
    alertEl.textContent = "";
  }

  try {
    const token = await invoke("get_github_token");
    if (token) {
      if (input) input.value = token;
      if (badge) {
        badge.className = "badge badge-accent";
        badge.textContent = "✅ Configuré";
      }
      if (deleteBtn) deleteBtn.style.display = "inline-flex";
    } else {
      if (input) input.value = "";
      if (badge) {
        badge.className = "badge";
        badge.textContent = "⚠️ Non configuré";
      }
      if (deleteBtn) deleteBtn.style.display = "none";
    }
  } catch (e) {
    console.error("Erreur lecture token GitHub:", e);
    if (badge) {
      badge.className = "badge";
      badge.textContent = "Erreur lecture";
    }
  }

  modal.classList.remove("hidden");
  if (input) input.focus();
}

function closeSettingsModal() {
  const modal = document.getElementById("settings-modal");
  if (modal) modal.classList.add("hidden");
}

function toggleTokenVisibility() {
  const input = document.getElementById("github-token-input");
  const btn = document.getElementById("toggle-token-visibility-btn");
  if (!input) return;

  if (input.type === "password") {
    input.type = "text";
    if (btn) btn.textContent = "🙈";
  } else {
    input.type = "password";
    if (btn) btn.textContent = "👁️";
  }
}

async function saveToken() {
  const input = document.getElementById("github-token-input");
  const alertEl = document.getElementById("github-token-alert");
  const token = input ? input.value.trim() : "";

  if (!token) {
    if (confirm("Le champ est vide. Souhaitez-vous supprimer le token existant ?")) {
      await deleteToken();
    }
    return;
  }

  try {
    await invoke("save_github_token", { token });
    if (alertEl) {
      alertEl.style.background = "rgba(166, 227, 161, 0.15)";
      alertEl.style.color = "var(--green)";
      alertEl.style.border = "1px solid var(--green)";
      alertEl.textContent = "✅ Token GitHub enregistré avec succès dans /etc/nixos/secrets/github-token.conf !";
      alertEl.classList.remove("hidden");
    }
    setTimeout(() => {
      closeSettingsModal();
    }, 1200);
  } catch (e) {
    if (alertEl) {
      alertEl.style.background = "rgba(243, 139, 168, 0.15)";
      alertEl.style.color = "var(--red)";
      alertEl.style.border = "1px solid var(--red)";
      alertEl.textContent = "❌ Erreur : " + e;
      alertEl.classList.remove("hidden");
    }
  }
}

async function deleteToken() {
  const alertEl = document.getElementById("github-token-alert");
  const input = document.getElementById("github-token-input");
  try {
    await invoke("delete_github_token");
    if (input) input.value = "";
    if (alertEl) {
      alertEl.style.background = "rgba(166, 227, 161, 0.15)";
      alertEl.style.color = "var(--green)";
      alertEl.style.border = "1px solid var(--green)";
      alertEl.textContent = "🗑️ Token supprimé avec succès.";
      alertEl.classList.remove("hidden");
    }
    setTimeout(() => {
      closeSettingsModal();
    }, 1000);
  } catch (e) {
    if (alertEl) {
      alertEl.style.background = "rgba(243, 139, 168, 0.15)";
      alertEl.style.color = "var(--red)";
      alertEl.style.border = "1px solid var(--red)";
      alertEl.textContent = "❌ Erreur : " + e;
      alertEl.classList.remove("hidden");
    }
  }
}

async function openExternalUrl(url) {
  try {
    await invoke("open_external_url", { url });
  } catch (e) {
    window.open(url, "_blank");
  }
}

window.openSettingsModal = openSettingsModal;
window.closeSettingsModal = closeSettingsModal;
window.toggleTokenVisibility = toggleTokenVisibility;
window.saveToken = saveToken;
window.deleteToken = deleteToken;
window.openExternalUrl = openExternalUrl;


// ==========================================================================
// 📦 Logithèque Nix & Paquets Personnalisés (Stable & Unstable) Controller
// ==========================================================================

let currentPackagesState = null;
let currentSearchResults = [];
let currentFilter = "all";
let searchDebounceTimeout = null;

function showToast(message, type = "info") {
  let container = document.getElementById("toast-container");
  if (!container) {
    container = document.createElement("div");
    container.id = "toast-container";
    container.className = "toast-container";
    document.body.appendChild(container);
  }
  const toast = document.createElement("div");
  toast.className = `toast toast-${type}`;
  const icon = type === "success" ? "✅" : type === "error" ? "❌" : type === "warning" ? "⚠️" : "💡";
  toast.innerHTML = `
    <span class="toast-icon">${icon}</span>
    <span class="toast-msg">${escapeHtml(message)}</span>
  `;
  container.appendChild(toast);
  setTimeout(() => toast.classList.add("show"), 10);
  setTimeout(() => {
    toast.classList.remove("show");
    setTimeout(() => toast.remove(), 300);
  }, 4000);
}

async function loadCustomPackages() {
  try {
    const state = await invoke("get_custom_packages");
    currentPackagesState = state;

    const stableCount = state.custom && state.custom.stable ? state.custom.stable.length : 0;
    const unstableCount = state.custom && state.custom.unstable ? state.custom.unstable.length : 0;
    const totalCount = stableCount + unstableCount;

    const elStable = document.getElementById("pkg-stat-stable");
    const elUnstable = document.getElementById("pkg-stat-unstable");
    const elFilterCount = document.getElementById("filter-installed-count");
    const elConflicts = document.getElementById("pkg-stat-conflicts");

    if (elStable) elStable.textContent = stableCount;
    if (elUnstable) elUnstable.textContent = unstableCount;
    if (elFilterCount) elFilterCount.textContent = totalCount;

    if (elConflicts) {
      const conflictNames = Object.keys(state.system_conflicts || {});
      elConflicts.textContent = `Protégé (${conflictNames.length} règles)`;
    }

    const input = document.getElementById("pkg-search-input");
    if (!input || !input.value.trim() || currentFilter === "installed") {
      renderCustomPackagesList();
    }
  } catch (err) {
    console.error("Erreur lors du chargement des paquets personnalisés :", err);
    showToast("Impossible de lire /etc/nixos/custom-packages.nix : " + err, "error");
  }
}

function initPackageSearch() {
  const input = document.getElementById("pkg-search-input");
  const clearBtn = document.getElementById("pkg-search-clear");
  if (!input) return;

  input.addEventListener("input", (e) => {
    const val = e.target.value;
    if (clearBtn) {
      if (val.trim()) clearBtn.classList.remove("hidden");
      else clearBtn.classList.add("hidden");
    }

    if (searchDebounceTimeout) clearTimeout(searchDebounceTimeout);

    if (!val.trim()) {
      const feedback = document.getElementById("pkg-search-feedback");
      const timing = document.getElementById("pkg-search-timing");
      if (feedback) feedback.textContent = "💡 Tapez un mot-clé pour rechercher en direct dans Nixpkgs Stable & Unstable.";
      if (timing) timing.textContent = "";
      renderCustomPackagesList();
      return;
    }

    searchDebounceTimeout = setTimeout(() => {
      executeSearch(val.trim());
    }, 320);
  });

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      if (searchDebounceTimeout) clearTimeout(searchDebounceTimeout);
      const val = input.value.trim();
      if (val) executeSearch(val);
    }
  });
}

function clearPackageSearch() {
  const input = document.getElementById("pkg-search-input");
  const clearBtn = document.getElementById("pkg-search-clear");
  if (input) {
    input.value = "";
    input.focus();
  }
  if (clearBtn) clearBtn.classList.add("hidden");

  const feedback = document.getElementById("pkg-search-feedback");
  const timing = document.getElementById("pkg-search-timing");
  if (feedback) feedback.textContent = "💡 Tapez un mot-clé pour rechercher en direct dans Nixpkgs Stable & Unstable.";
  if (timing) timing.textContent = "";

  renderCustomPackagesList();
}

function quickSearch(query) {
  const input = document.getElementById("pkg-search-input");
  const clearBtn = document.getElementById("pkg-search-clear");
  if (input) {
    input.value = query;
    if (clearBtn) clearBtn.classList.remove("hidden");
    executeSearch(query);
  }
}

async function executeSearch(query) {
  const feedback = document.getElementById("pkg-search-feedback");
  const timing = document.getElementById("pkg-search-timing");
  const grid = document.getElementById("pkg-cards-grid");

  if (feedback) feedback.innerHTML = `<span>⏳</span> Recherche de <strong>« ${escapeHtml(query)} »</strong> dans les catalogues Stable 26.05 & Unstable...`;
  if (timing) timing.textContent = "";

  if (grid) {
    grid.innerHTML = `
      <div class="pkg-empty-state">
        <div class="pkg-empty-icon">⏳</div>
        <h3>Interrogation des index NixOS Search...</h3>
        <p>Comparaison des versions Stable 26.05 et Unstable en cours.</p>
      </div>
    `;
  }

  const startTime = performance.now();
  try {
    const results = await invoke("search_nix_packages", { query });
    const elapsed = Math.round(performance.now() - startTime);

    currentSearchResults = results;

    if (timing) timing.textContent = `${elapsed} ms`;
    if (feedback) {
      if (results.length === 0) {
        feedback.innerHTML = `❌ Aucun paquet trouvé pour <strong>« ${escapeHtml(query)} »</strong>.`;
      } else {
        feedback.innerHTML = `✅ <strong>${results.length} paquet(s)</strong> trouvé(s) pour « ${escapeHtml(query)} ».`;
      }
    }

    applyFilterAndRender();
  } catch (err) {
    console.error("Erreur de recherche Nix :", err);
    if (feedback) feedback.innerHTML = `<span style="color: var(--red);">❌ Erreur de recherche : ${escapeHtml(String(err))}</span>`;
    showToast("Échec de la recherche dans Nixpkgs : " + err, "error");
  }
}

function setPackageFilter(filter) {
  currentFilter = filter;
  const pills = document.querySelectorAll(".pkg-filter-pills .filter-pill");
  pills.forEach(p => {
    if (p.getAttribute("data-filter") === filter) p.classList.add("active");
    else p.classList.remove("active");
  });

  if (filter === "installed") {
    renderCustomPackagesList();
  } else {
    const input = document.getElementById("pkg-search-input");
    if (input && input.value.trim()) {
      applyFilterAndRender();
    } else {
      renderCustomPackagesList();
    }
  }
}

function applyFilterAndRender() {
  let list = currentSearchResults;
  if (currentFilter === "stable-only") {
    list = list.filter(p => !!p.stable_version);
  } else if (currentFilter === "unstable-only") {
    list = list.filter(p => !!p.unstable_version);
  } else if (currentFilter === "installed") {
    list = list.filter(p => p.is_custom_stable || p.is_custom_unstable);
  }

  renderPackageCards(list, false);
}

function renderCustomPackagesList() {
  const grid = document.getElementById("pkg-cards-grid");
  if (!grid) return;

  if (!currentPackagesState || !currentPackagesState.custom_details || currentPackagesState.custom_details.length === 0) {
    grid.innerHTML = `
      <div class="pkg-empty-state">
        <div class="pkg-empty-icon">📦</div>
        <h3 style="color: var(--text); margin-bottom: 8px;">Aucun paquet personnalisé configuré</h3>
        <p style="color: var(--subtext0); max-width: 500px; margin: 0 auto 18px auto; line-height: 1.5;">
          Votre fichier <code>/etc/nixos/custom-packages.nix</code> est vierge. Recherchez une application dans la barre ci-dessus pour l'ajouter en version Stable ou Unstable !
        </p>
        <div style="font-size: 13px; color: var(--subtext1); margin-bottom: 8px;">💡 Recherches populaires :</div>
        <div class="pkg-quick-tags">
          <button class="pkg-quick-tag" onclick="quickSearch('neovim')">neovim</button>
          <button class="pkg-quick-tag" onclick="quickSearch('spotify')">spotify</button>
          <button class="pkg-quick-tag" onclick="quickSearch('obsidian')">obsidian</button>
          <button class="pkg-quick-tag" onclick="quickSearch('btop')">btop</button>
          <button class="pkg-quick-tag" onclick="quickSearch('micro')">micro</button>
          <button class="pkg-quick-tag" onclick="quickSearch('fastfetch')">fastfetch</button>
          <button class="pkg-quick-tag" onclick="quickSearch('zen-browser')">zen-browser</button>
        </div>
      </div>
    `;
    return;
  }

  renderPackageCards(currentPackagesState.custom_details, true);
}

function renderPackageCards(packages, isCustomView) {
  const grid = document.getElementById("pkg-cards-grid");
  if (!grid) return;

  if (packages.length === 0) {
    grid.innerHTML = `
      <div class="pkg-empty-state">
        <div class="pkg-empty-icon">🔍</div>
        <h3>Aucun paquet ne correspond au filtre</h3>
        <p>Essayez de changer de filtre ou de modifier votre terme de recherche.</p>
      </div>
    `;
    return;
  }

  let html = "";
  for (const pkg of packages) {
    const isStable = pkg.is_custom_stable;
    const isUnstable = pkg.is_custom_unstable;
    const isConflict = pkg.is_system_conflict;

    let cardClass = "pkg-card";
    if (isStable) cardClass += " installed-stable";
    else if (isUnstable) cardClass += " installed-unstable";
    else if (isConflict) cardClass += " conflict";

    let badgeStatus = "";
    if (isStable) {
      badgeStatus = `<span class="badge" style="background: rgba(166, 227, 161, 0.18); color: var(--green); border: 1px solid var(--green); font-size: 11px;">🌱 Custom Stable</span>`;
    } else if (isUnstable) {
      badgeStatus = `<span class="badge" style="background: rgba(203, 166, 247, 0.18); color: var(--mauve); border: 1px solid var(--mauve); font-size: 11px;">⚡ Custom Unstable</span>`;
    } else if (isConflict) {
      badgeStatus = `<span class="badge" style="background: rgba(249, 226, 175, 0.18); color: var(--yellow); border: 1px solid var(--yellow); font-size: 11px;">⚠️ Système</span>`;
    }

    const stableAvail = !!pkg.stable_version;
    const unstableAvail = !!pkg.unstable_version;

    const stableClass = stableAvail ? "stable available" : "stable unavailable";
    const unstableClass = unstableAvail ? "unstable available" : "unstable unavailable";

    const stableVal = pkg.stable_version || "Non disponible";
    const unstableVal = pkg.unstable_version || "Non disponible";

    let conflictHtml = "";
    if (isConflict) {
      conflictHtml = `
        <div class="pkg-conflict-box">
          <span>⚠️</span>
          <div>
            <strong>Conflit avec le système :</strong> ${escapeHtml(pkg.conflict_reason || "Paquet système")}
          </div>
        </div>
      `;
    }

    let actionsHtml = "";
    if (isConflict) {
      actionsHtml = `
        <button class="btn btn-outline btn-sm" disabled title="Ce paquet est déjà géré par la configuration système">
          🔒 Géré par le système
        </button>
      `;
    } else if (isStable) {
      actionsHtml = `
        <button class="btn btn-outline btn-sm" onclick="switchPackageBranch('${escapeHtml(pkg.attr_name)}', 'unstable')" title="Basculer vers la version Unstable">
          ⚡ Basculer Unstable
        </button>
        <button class="btn btn-danger btn-sm" onclick="deleteCustomPackage('${escapeHtml(pkg.attr_name)}')" title="Retirer du fichier custom-packages.nix">
          🗑️ Retirer
        </button>
      `;
    } else if (isUnstable) {
      actionsHtml = `
        <button class="btn btn-outline btn-sm" onclick="switchPackageBranch('${escapeHtml(pkg.attr_name)}', 'stable')" title="Basculer vers la version Stable">
          🌱 Basculer Stable
        </button>
        <button class="btn btn-danger btn-sm" onclick="deleteCustomPackage('${escapeHtml(pkg.attr_name)}')" title="Retirer du fichier custom-packages.nix">
          🗑️ Retirer
        </button>
      `;
    } else {
      let addStableBtn = "";
      let addUnstableBtn = "";

      if (stableAvail) {
        addStableBtn = `
          <button class="btn btn-success btn-sm" onclick="installCustomPackage('${escapeHtml(pkg.attr_name)}', 'stable')">
            🌱 + Stable
          </button>
        `;
      }
      if (unstableAvail) {
        addUnstableBtn = `
          <button class="btn btn-primary btn-sm" onclick="installCustomPackage('${escapeHtml(pkg.attr_name)}', 'unstable')">
            ⚡ + Unstable
          </button>
        `;
      }

      actionsHtml = `${addStableBtn} ${addUnstableBtn}`;
      if (!actionsHtml.trim()) {
        actionsHtml = `<span style="font-size: 11.5px; color: var(--subtext0);">Indisponible</span>`;
      }
    }

    html += `
      <div class="${cardClass}">
        <div class="pkg-card-header">
          <div class="pkg-title-wrap">
            <div class="pkg-icon">📦</div>
            <div>
              <h4 class="pkg-attr-name">${escapeHtml(pkg.attr_name)}</h4>
              <span class="pkg-pname">${escapeHtml(pkg.pname)}</span>
            </div>
          </div>
          <div>${badgeStatus}</div>
        </div>

        <p class="pkg-description" title="${escapeHtml(pkg.description)}">
          ${escapeHtml(pkg.description || "Aucune description fournie dans Nixpkgs.")}
        </p>

        <div class="pkg-versions-grid">
          <div class="pkg-version-pill ${stableClass}">
            <span class="version-label">🌱 Stable (26.05)</span>
            <span class="version-val" title="${escapeHtml(stableVal)}">${escapeHtml(stableVal)}</span>
          </div>
          <div class="pkg-version-pill ${unstableClass}">
            <span class="version-label">⚡ Unstable</span>
            <span class="version-val" title="${escapeHtml(unstableVal)}">${escapeHtml(unstableVal)}</span>
          </div>
        </div>

        ${conflictHtml}

        <div class="pkg-card-actions">
          ${actionsHtml}
        </div>
      </div>
    `;
  }

  grid.innerHTML = html;
}

async function installCustomPackage(name, channel) {
  try {
    await invoke("add_custom_package", { name, channel });
    const channelLabel = channel === "stable" ? "Stable (26.05)" : "Unstable";
    showToast(`Paquet « ${name} » ajouté en ${channelLabel} !`, "success");

    await loadCustomPackages();

    for (const item of currentSearchResults) {
      if (item.attr_name === name) {
        item.is_custom_stable = channel === "stable";
        item.is_custom_unstable = channel === "unstable";
      }
    }
    applyFilterAndRender();
  } catch (err) {
    console.error("Erreur ajout paquet :", err);
    showToast("Erreur : " + err, "error");
  }
}

async function deleteCustomPackage(name) {
  try {
    await invoke("remove_custom_package", { name, channel: null });
    showToast(`Paquet « ${name} » retiré de custom-packages.nix !`, "info");

    await loadCustomPackages();

    for (const item of currentSearchResults) {
      if (item.attr_name === name) {
        item.is_custom_stable = false;
        item.is_custom_unstable = false;
      }
    }
    applyFilterAndRender();
  } catch (err) {
    console.error("Erreur suppression paquet :", err);
    showToast("Erreur : " + err, "error");
  }
}

async function switchPackageBranch(name, targetBranch) {
  try {
    await invoke("add_custom_package", { name, channel: targetBranch });
    const channelLabel = targetBranch === "stable" ? "Stable (26.05)" : "Unstable";
    showToast(`Paquet « ${name} » basculé sur ${channelLabel} !`, "success");

    await loadCustomPackages();

    for (const item of currentSearchResults) {
      if (item.attr_name === name) {
        item.is_custom_stable = targetBranch === "stable";
        item.is_custom_unstable = targetBranch === "unstable";
      }
    }
    applyFilterAndRender();
  } catch (err) {
    console.error("Erreur bascule paquet :", err);
    showToast("Erreur : " + err, "error");
  }
}

function applyCustomPackagesDeploy() {
  runTerminalTask("apply-packages", "📦 Déploiement des paquets personnalisés (nh os switch)");
}

window.loadCustomPackages = loadCustomPackages;
window.clearPackageSearch = clearPackageSearch;
window.setPackageFilter = setPackageFilter;
window.installCustomPackage = installCustomPackage;
window.deleteCustomPackage = deleteCustomPackage;
window.switchPackageBranch = switchPackageBranch;
window.applyCustomPackagesDeploy = applyCustomPackagesDeploy;
window.quickSearch = quickSearch;
