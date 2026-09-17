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

async function initDashboardVersionBadge() {
  const navDashVer = document.getElementById("nav-dashboard-version-text");
  if (navDashVer) {
    try {
      const ver = await invoke("get_dashboard_version");
      if (ver) {
        navDashVer.textContent = `v${ver} • Stable`;
      }
    } catch (_) {
      navDashVer.textContent = "v0.4.2 • Stable";
    }
  }
}

document.addEventListener("DOMContentLoaded", () => {
  initDashboardVersionBadge();
  initTabs();
  startMetricsPolling();
  loadGenerations();
  loadConfig();
  initTerminal();
  loadStorageDevices();
  loadCustomPackages();
  initPackageSearch();
  initSpeedtest();
  loadFirewallState();
  initNetworkCenter();
  loadCommitSecurityInfo();
  loadUserShell();

  // Évaluation des mises à jour en arrière-plan sans bloquer l'affichage
  setTimeout(() => {
    checkForUpdates();
    setInterval(checkForUpdates, 45000);
  }, 200);
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
        } else if (targetId === "tab-system") {
          setTimeout(() => {
            if (typeof resizeSpeedtestCanvas === "function") {
              resizeSpeedtestCanvas();
              renderSpeedtestGraph();
            }
          }, 50);
        } else if (targetId === "tab-firewall") {
          loadFirewallState();
          initNetworkCenter();
        } else if (targetId === "tab-generations") {
          loadGenerations();
          loadUserShell();
          initFastfetchView();
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
    setupCategoryNavigation();
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

  // Gaming & Nvidia rule for Gamescope
  const isNvidia = (c.gpu_driver === "nvidia" || c.gpu_driver === "nvidia-legacy");
  const gamescopeCard = document.getElementById("cfg-card-gamescope");
  const gamescopeInput = document.getElementById("cfg-gamescope-session");
  const gamescopeWarning = document.getElementById("gamescope-nvidia-warning");

  if (isNvidia) {
    if (gamescopeInput) {
      gamescopeInput.checked = false;
      gamescopeInput.disabled = true;
    }
    if (gamescopeCard) {
      gamescopeCard.classList.add("disabled");
    }
    if (gamescopeWarning) {
      gamescopeWarning.classList.remove("hidden");
    }
  } else {
    if (gamescopeInput) {
      gamescopeInput.disabled = false;
      gamescopeInput.checked = !!c.gaming.gamescope_session;
    }
    if (gamescopeCard) {
      gamescopeCard.classList.remove("disabled");
    }
    if (gamescopeWarning) {
      gamescopeWarning.classList.add("hidden");
    }
  }

  setCheck("cfg-steam", c.gaming.steam);
  setCheck("cfg-goverlay", c.gaming.goverlay);
  setCheck("cfg-lutris", c.gaming.lutris);
  setCheck("cfg-heroic", c.gaming.heroic);
  setCheck("cfg-faugus", c.gaming.faugus);
  setCheck("cfg-decky", c.gaming.decky_loader);
  setCheck("cfg-geforce", c.gaming.geforce_now);
  setCheck("cfg-wheels", c.gaming.steering_wheels);
  setCheck("cfg-sunshine", c.gaming.sunshine);
  setCheck("cfg-sober", c.gaming.sober);

  // Emulation
  setCheck("cfg-esde", c.emulation.frontend === "es-de");
  setCheck("cfg-retroarch", c.emulation.retroarch);
  setCheck("cfg-duckstation", c.emulation.duckstation);
  setCheck("cfg-eden", c.emulation.eden);
  setCheck("cfg-dolphin", c.emulation.dolphin);
  setCheck("cfg-pcsx2", c.emulation.pcsx2);
  setCheck("cfg-ppsspp", c.emulation.ppsspp);
  setCheck("cfg-azahar", c.emulation.azahar);
  setCheck("cfg-melonds", c.emulation.melonds);
  setCheck("cfg-mgba", c.emulation.mgba);
  setCheck("cfg-rpcs3", c.emulation.rpcs3);
  setCheck("cfg-xemu", c.emulation.xemu);

  // Media
  setCheck("cfg-stremio", c.media.stremio);
  setCheck("cfg-vlc", c.media.vlc);
  setCheck("cfg-mpv", c.media.mpv);
  setCheck("cfg-pear", c.creation.pear_desktop);

  // Video & DaVinci Resolve
  setCheck("cfg-obs", c.creation.obs_studio);
  setCheck("cfg-kdenlive", c.creation.kdenlive);
  setRadioVal("davinci_resolve", c.creation.davinci_resolve || "none");

  // Audio Production
  setCheck("cfg-audacity", c.creation.audacity);
  setCheck("cfg-ardour", c.creation.ardour);

  // Creation 3D & Engine
  setCheck("cfg-blender", c.creation.blender);
  setCheck("cfg-godot", c.creation.godot);

  // 3D Printing & Slicers
  setCheck("cfg-orcaslicer", c.slicers ? c.slicers.orcaslicer : false);
  setCheck("cfg-prusaslicer", c.slicers ? c.slicers.prusaslicer : false);
  setCheck("cfg-cura", c.slicers ? c.slicers.cura : false);
  setCheck("cfg-bambustudio", c.slicers ? c.slicers.bambustudio : false);

  // System & Utilities
  setCheck("cfg-flatseal", c.media.flatseal);
  setCheck("cfg-tailscale", c.media.tailscale);
  setCheck("cfg-localsend", c.media.localsend);
  setCheck("cfg-motrix", c.media.motrix);
  setCheck("cfg-kvm", c.creation.virtualisation);
  setCheck("cfg-ide-zed", c.creation.zed);
  setCheck("cfg-ide-antigravity", c.creation.antigravity);
  setCheck("cfg-ide-vscode", c.creation.vscode);
  setCheck("cfg-omniroute", c.creation.omniroute);

  updateCategoryPillCounters();
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

  const isNvidia = (currentConfig.gpu_driver === "nvidia" || currentConfig.gpu_driver === "nvidia-legacy");
  currentConfig.gaming.gamescope_session = isNvidia ? false : isChecked("cfg-gamescope-session");
  currentConfig.gaming.steam = isChecked("cfg-steam");
  currentConfig.gaming.goverlay = isChecked("cfg-goverlay");
  currentConfig.gaming.lutris = isChecked("cfg-lutris");
  currentConfig.gaming.heroic = isChecked("cfg-heroic");
  currentConfig.gaming.faugus = isChecked("cfg-faugus");
  currentConfig.gaming.decky_loader = isChecked("cfg-decky");
  currentConfig.gaming.geforce_now = isChecked("cfg-geforce");
  currentConfig.gaming.steering_wheels = isChecked("cfg-wheels");
  currentConfig.gaming.sunshine = isChecked("cfg-sunshine");
  currentConfig.gaming.sober = isChecked("cfg-sober");

  currentConfig.emulation.frontend = isChecked("cfg-esde") ? "es-de" : "none";
  currentConfig.emulation.retroarch = isChecked("cfg-retroarch");
  currentConfig.emulation.duckstation = isChecked("cfg-duckstation");
  currentConfig.emulation.eden = isChecked("cfg-eden");
  currentConfig.emulation.dolphin = isChecked("cfg-dolphin");
  currentConfig.emulation.pcsx2 = isChecked("cfg-pcsx2");
  currentConfig.emulation.ppsspp = isChecked("cfg-ppsspp");
  currentConfig.emulation.azahar = isChecked("cfg-azahar");
  currentConfig.emulation.melonds = isChecked("cfg-melonds");
  currentConfig.emulation.mgba = isChecked("cfg-mgba");
  currentConfig.emulation.rpcs3 = isChecked("cfg-rpcs3");
  currentConfig.emulation.xemu = isChecked("cfg-xemu");

  currentConfig.media.stremio = isChecked("cfg-stremio");
  currentConfig.media.vlc = isChecked("cfg-vlc");
  currentConfig.media.mpv = isChecked("cfg-mpv");
  currentConfig.creation.pear_desktop = isChecked("cfg-pear");

  currentConfig.creation.obs_studio = isChecked("cfg-obs");
  currentConfig.creation.kdenlive = isChecked("cfg-kdenlive");
  const davinciRadio = document.querySelector('input[name="davinci_resolve"]:checked');
  if (davinciRadio) currentConfig.creation.davinci_resolve = davinciRadio.value;

  currentConfig.creation.audacity = isChecked("cfg-audacity");
  currentConfig.creation.ardour = isChecked("cfg-ardour");

  currentConfig.creation.blender = isChecked("cfg-blender");
  currentConfig.creation.godot = isChecked("cfg-godot");

  if (!currentConfig.slicers) currentConfig.slicers = {};
  currentConfig.slicers.orcaslicer = isChecked("cfg-orcaslicer");
  currentConfig.slicers.prusaslicer = isChecked("cfg-prusaslicer");
  currentConfig.slicers.cura = isChecked("cfg-cura");
  currentConfig.slicers.bambustudio = isChecked("cfg-bambustudio");

  currentConfig.media.flatseal = isChecked("cfg-flatseal");
  currentConfig.media.localsend = isChecked("cfg-localsend");
  currentConfig.media.tailscale = isChecked("cfg-tailscale");
  currentConfig.media.motrix = isChecked("cfg-motrix");

  currentConfig.creation.virtualisation = isChecked("cfg-kvm");
  currentConfig.creation.zed = isChecked("cfg-ide-zed");
  currentConfig.creation.antigravity = isChecked("cfg-ide-antigravity");
  currentConfig.creation.vscode = isChecked("cfg-ide-vscode");
  currentConfig.creation.omniroute = isChecked("cfg-omniroute");

  updateCategoryPillCounters();
}

function setupCategoryNavigation() {
  const pills = document.querySelectorAll(".cat-pill");
  pills.forEach(pill => {
    pill.onclick = () => {
      pills.forEach(p => p.classList.remove("active"));
      pill.classList.add("active");
      const cat = pill.getAttribute("data-category");
      filterConfigCategories(cat);
    };
  });
}

function filterConfigCategories(cat) {
  const panels = document.querySelectorAll(".config-category-panel");
  panels.forEach(panel => {
    if (cat === "all" || panel.getAttribute("data-category") === cat) {
      panel.classList.remove("hidden");
    } else {
      panel.classList.add("hidden");
    }
  });
}

function updateCategoryPillCounters() {
  const updateCount = (cat, total) => {
    const panel = document.querySelector(`.config-category-panel[data-category="${cat}"]`);
    const counter = document.getElementById(`cat-count-${cat}`);
    if (!panel || !counter) return;
    const checkedCount = panel.querySelectorAll('input[type="checkbox"]:checked').length;
    counter.textContent = `${checkedCount}/${total}`;
  };

  updateCount("gaming", 11);
  updateCount("emulation", 11);
  updateCount("multimedia", 4);
  updateCount("video", 2);
  updateCount("audio", 2);
  updateCount("creation3d", 2);
  updateCount("printing3d", 4);
  updateCount("system", 7);
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
    const confirmSwitch = await showConfirmModal({
      title: "Changement d'environnement de bureau",
      subtitle: `${initialCfg?.desktop_env || ""} ➔ ${currentConfig.desktop_env}`,
      icon: "⚠️",
      message: "L'application en direct ('nh os switch') va relancer le gestionnaire d'affichage et risque de fermer brutalement votre session graphique.",
      warning: "Voulez-vous plutôt l'appliquer en toute sécurité au prochain redémarrage ('nh os boot') ?",
      confirmText: "Appliquer au reboot (conseillé)",
      confirmIcon: "🔄",
      confirmClass: "btn-primary",
      cancelText: "Appliquer en direct (risqué)",
    });
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
    updatePackageCountBadge(true);
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

function runAction(action, customExtra = null) {
  let task = "";
  let title = "";
  let extra = customExtra;

  switch (action) {
    case "update-dashboard":
      task = "update-dashboard";
      title = "Mise à jour du Dashboard ChomiamOS";
      extra = customExtra || "stable";
      break;
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
  const confirmed = await showConfirmModal({
    title: "Démonter le disque",
    subtitle: "Gestion des volumes de stockage",
    icon: "⏏️",
    message: `Voulez-vous vraiment démonter le disque monté sur "${mountPoint}" ?`,
    warning: "S'il s'agit d'un montage permanent NixOS, il sera retiré de mount.nix.",
    confirmText: "Démonter le volume",
    confirmIcon: "⏏️",
    confirmClass: "btn-danger",
    isDanger: true,
  });

  if (!confirmed) return;

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

// ==========================================================================
// Generic Confirmation & Alert Modal System (Catppuccin Theme)
// ==========================================================================

function showConfirmModal({
  title = "Confirmation",
  subtitle = "",
  icon = "⚠️",
  message = "Êtes-vous sûr de vouloir continuer ?",
  details = "",
  warning = "",
  confirmText = "Confirmer",
  confirmIcon = "✓",
  confirmClass = "btn-primary",
  cancelText = "Annuler",
  isDanger = false,
} = {}) {
  return new Promise((resolve) => {
    const modal = document.getElementById("confirm-modal");
    const card = document.getElementById("confirm-modal-card");
    const iconEl = document.getElementById("confirm-modal-icon");
    const titleEl = document.getElementById("confirm-modal-title");
    const subtitleEl = document.getElementById("confirm-modal-subtitle");
    const messageEl = document.getElementById("confirm-modal-message");
    const detailsEl = document.getElementById("confirm-modal-details");
    const warningBoxEl = document.getElementById("confirm-modal-warning-box");
    const warningEl = document.getElementById("confirm-modal-warning");
    const okBtn = document.getElementById("confirm-modal-ok-btn");
    const okTextEl = document.getElementById("confirm-modal-ok-text");
    const okIconEl = document.getElementById("confirm-modal-ok-icon");
    const cancelBtn = document.getElementById("confirm-modal-cancel-btn");
    const closeBtn = document.getElementById("confirm-modal-close-btn");

    if (!modal) {
      resolve(window.confirm(message));
      return;
    }

    iconEl.textContent = icon;
    titleEl.textContent = title;
    subtitleEl.textContent = subtitle || "";
    subtitleEl.style.display = subtitle ? "block" : "none";
    messageEl.textContent = message;

    if (details) {
      detailsEl.textContent = details;
      detailsEl.style.display = "block";
    } else {
      detailsEl.style.display = "none";
    }

    if (warning) {
      warningEl.textContent = warning;
      warningBoxEl.style.display = "flex";
    } else {
      warningBoxEl.style.display = "none";
    }

    if (isDanger) {
      card.classList.add("modal-card-danger");
    } else {
      card.classList.remove("modal-card-danger");
    }

    okBtn.className = `btn ${confirmClass}`;
    okTextEl.textContent = confirmText;
    if (confirmIcon) {
      okIconEl.textContent = confirmIcon;
      okIconEl.style.display = "inline";
    } else {
      okIconEl.style.display = "none";
    }

    if (cancelText === null) {
      cancelBtn.style.display = "none";
    } else {
      cancelBtn.style.display = "inline-flex";
      cancelBtn.textContent = cancelText;
    }

    modal.classList.remove("hidden");

    function cleanup(result) {
      modal.classList.add("hidden");
      okBtn.removeEventListener("click", onOk);
      cancelBtn.removeEventListener("click", onCancel);
      closeBtn.removeEventListener("click", onCancel);
      document.removeEventListener("keydown", onKeydown);
      modal.removeEventListener("click", onBackdrop);
      resolve(result);
    }

    function onOk() { cleanup(true); }
    function onCancel() { cleanup(false); }
    function onKeydown(e) {
      if (e.key === "Escape") {
        e.preventDefault();
        cleanup(false);
      } else if (e.key === "Enter") {
        e.preventDefault();
        cleanup(true);
      }
    }
    function onBackdrop(e) {
      if (e.target === modal) cleanup(false);
    }

    okBtn.addEventListener("click", onOk);
    cancelBtn.addEventListener("click", onCancel);
    closeBtn.addEventListener("click", onCancel);
    document.addEventListener("keydown", onKeydown);
    modal.addEventListener("click", onBackdrop);
  });
}

function showAlertModal({
  title = "Information",
  subtitle = "",
  icon = "ℹ️",
  message = "",
  details = "",
  warning = "",
  okText = "D'accord",
  isDanger = false,
} = {}) {
  return showConfirmModal({
    title,
    subtitle,
    icon,
    message,
    details,
    warning,
    confirmText: okText,
    confirmIcon: "",
    confirmClass: isDanger ? "btn-danger" : "btn-primary",
    cancelText: null,
    isDanger,
  });
}

window.showConfirmModal = showConfirmModal;
window.showAlertModal = showAlertModal;

async function deleteSelectedGenerations() {
  const ids = getSelectedGenIds();
  if (ids.length === 0) return;

  const plural = ids.length > 1 ? "s" : "";
  const idsStr = ids.map(id => `#${id}`).join(", ");

  const confirmed = await showConfirmModal({
    title: "Supprimer les générations",
    subtitle: "Nettoyage du profil système NixOS",
    icon: "🗑️",
    message: `Voulez-vous vraiment supprimer ${ids.length} génération${plural} système ?`,
    details: idsStr,
    warning: "Cette action est irréversible et retirera ces entrées du chargeur de démarrage.",
    confirmText: `Supprimer ${ids.length} génération${plural}`,
    confirmIcon: "🗑️",
    confirmClass: "btn-danger",
    isDanger: true,
  });

  if (!confirmed) return;

  const idsParam = ids.join(" ");
  runTerminalTask(
    "delete-generations",
    `Suppression de ${ids.length} génération${plural} (${idsStr})`,
    idsParam
  );
  clearGenSelection();
}

async function switchToSelectedGeneration() {
  const ids = getSelectedGenIds();
  if (ids.length !== 1) return;

  const id = ids[0];
  const confirmed = await showConfirmModal({
    title: "Activer la génération",
    subtitle: "Changement de la version de démarrage",
    icon: "🔄",
    message: `Voulez-vous rebooter sur la génération #${id} ?`,
    details: `La génération #${id} sera configurée dans le bootloader (GRUB/systemd-boot) pour le prochain démarrage.`,
    confirmText: "Activer au prochain reboot",
    confirmIcon: "🔄",
    confirmClass: "btn-primary",
    isDanger: false,
  });

  if (!confirmed) return;

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

let currentDashboardStatus = null;
let selectedDashboardChannel = "stable";

async function checkForUpdates() {
  try {
    const status = await invoke("check_system_updates");
    if (!status) return;

    currentDashboardStatus = status;

    const metaCommit = document.getElementById("meta-commit");
    if (metaCommit && status.github_local_commit) {
      metaCommit.textContent = "Commit: " + status.github_local_commit;
    }
    loadCommitSecurityInfo();

    // Bulle orange si un nouveau commit distant est disponible sur GitHub
    const metaUpdateBadge = document.getElementById("meta-update-badge");
    const metaUpdateText = document.getElementById("meta-update-text");
    if (metaUpdateBadge) {
      if (status.github_has_updates) {
        metaUpdateBadge.classList.remove("hidden");
        if (metaUpdateText) {
          metaUpdateText.textContent = status.github_remote_commit
            ? `Nouveau commit : ${status.github_remote_commit}`
            : "Nouveau commit disponible";
        }
        metaUpdateBadge.setAttribute("title", `Un nouveau commit (${status.github_remote_commit || "distant"}) est disponible sur GitHub.\nCliquez pour synchroniser et appliquer la mise à jour.`);
      } else {
        metaUpdateBadge.classList.add("hidden");
      }
    }

    // 1. Mise à jour de la bulle verte de version et canal dans la navbar
    const navDashVer = document.getElementById("nav-dashboard-version-text");
    const navDashArrow = document.getElementById("nav-dashboard-update-arrow");
    const channel = status.dashboard_channel || "Stable";

    if (navDashVer) {
      navDashVer.textContent = `v${status.current_version} • ${channel}`;
    }

    if (navDashArrow) {
      if (status.dashboard_has_updates) {
        navDashArrow.classList.remove("hidden");
      } else {
        navDashArrow.classList.add("hidden");
      }
    }

    // 2. Indicateur de synchronisation globale NixOS si changements distants
    const navIndicator = document.getElementById("nav-update-indicator");
    if (navIndicator) {
      if (channel.toLowerCase() === "stable") {
        // Sur la branche stable, ne pas afficher le bouton "Déployer MAJ" en haut
        navIndicator.classList.add("hidden");
      } else if (status.github_has_updates) {
        navIndicator.classList.remove("hidden");
        navIndicator.innerHTML = "<span>🐙</span> <span>Sync GitHub</span>";
        navIndicator.onclick = () => runAction("sync-github");
      } else if (status.system_needs_switch) {
        navIndicator.classList.remove("hidden");
        navIndicator.innerHTML = "<span>⚡</span> <span>Déployer MAJ</span>";
        navIndicator.onclick = () => runAction("switch");
      } else {
        navIndicator.classList.add("hidden");
      }
    }

    // 3. Calcul et affichage du nombre de paquets à mettre à jour
    updatePackageCountBadge(false);
  } catch (err) {
    console.warn("Vérification des mises à jour ignorée:", err);
  }
}

// =========================================================================
// 📦 Calcul dynamique du nombre de paquets à mettre à jour (nh os switch -u)
// =========================================================================

async function updatePackageCountBadge(force = false) {
  const subtext = document.getElementById("subtext-pkg-update");
  const btn = document.getElementById("btn-pkg-switch-update");
  if (!subtext) return;

  if (force || !subtext.textContent || subtext.textContent === "nh os switch -u") {
    subtext.textContent = "⚡ Recherche des MAJ...";
  }

  try {
    const res = await invoke("get_package_update_count", { force: Boolean(force) });
    if (!res) return;

    subtext.textContent = res.status_text || "nh os switch -u";

    if (res.has_updates && res.count > 0) {
      subtext.classList.remove("up-to-date");
      subtext.classList.add("has-updates");
      if (btn) {
        let title = `Mise à jour disponible (${res.count} paquet${res.count > 1 ? "s" : ""}) :\n`;
        if (res.details && res.details.length > 0) {
          title += res.details.slice(0, 12).map((d) => `• ${d}`).join("\n");
          if (res.details.length > 12) {
            title += `\n... et ${res.details.length - 12} autres`;
          }
        }
        title += "\n\nCliquez pour appliquer les mises à jour via nh os switch -u";
        btn.setAttribute("title", title);
      }
    } else {
      subtext.classList.remove("has-updates");
      subtext.classList.add("up-to-date");
      if (btn) {
        btn.setAttribute(
          "title",
          "Tous les paquets du système et sources Flake sont à jour.\nCliquez pour forcer une vérification (nh os switch -u)."
        );
      }
    }
  } catch (err) {
    console.warn("Erreur lors de la récupération des paquets à mettre à jour:", err);
    if (!subtext.textContent || subtext.textContent.includes("Vérification")) {
      subtext.textContent = "nh os switch -u";
    }
  }
}

window.updatePackageCountBadge = updatePackageCountBadge;

function openDashboardUpdateModal() {
  const modal = document.getElementById("dashboard-update-modal");
  if (!modal) return;

  const currentVerEl = document.getElementById("modal-dash-current-ver");
  const channelTagEl = document.getElementById("modal-dash-current-channel-tag");
  const stableVerEl = document.getElementById("modal-dash-stable-ver");
  const testingVerEl = document.getElementById("modal-dash-testing-ver");
  const statusBadgeEl = document.getElementById("modal-dash-status-badge");
  const cardStableVer = document.getElementById("card-channel-stable-ver");
  const cardTestingVer = document.getElementById("card-channel-testing-ver");

  const status = currentDashboardStatus || {};
  const currentVer = status.current_version || "0.4.2";
  const lockedSha = status.dashboard_locked_commit ? ` (${status.dashboard_locked_commit})` : "";
  const channel = (status.dashboard_channel || "Stable").toLowerCase();

  selectedDashboardChannel = channel;

  if (currentVerEl) currentVerEl.textContent = `v${currentVer}${lockedSha}`;
  if (channelTagEl) {
    channelTagEl.textContent = channel === "testing" ? "Testing" : "Stable";
    channelTagEl.className = `dash-channel-tag tag-${channel}`;
  }

  // Version Stable disponible
  const stableVer = status.dashboard_stable_version ? `v${status.dashboard_stable_version}` : `v${currentVer}`;
  const stableSha = status.dashboard_stable_commit ? ` (${status.dashboard_stable_commit})` : "";
  if (stableVerEl) stableVerEl.textContent = `${stableVer}${stableSha}`;
  if (cardStableVer) cardStableVer.textContent = `${stableVer}${stableSha}`;

  // Version Testing disponible
  const testingVer = status.dashboard_testing_version ? `v${status.dashboard_testing_version}` : `v${currentVer}`;
  const testingSha = status.dashboard_testing_commit ? ` (${status.dashboard_testing_commit})` : "";
  if (testingVerEl) testingVerEl.textContent = `${testingVer}${testingSha}`;
  if (cardTestingVer) cardTestingVer.textContent = `${testingVer}${testingSha}`;

  if (statusBadgeEl) {
    if (status.dashboard_has_updates) {
      statusBadgeEl.className = "dash-status-badge update-available";
      statusBadgeEl.textContent = `Mise à jour disponible (${channel === "testing" ? testingVer : stableVer})`;
    } else {
      statusBadgeEl.className = "dash-status-badge up-to-date";
      statusBadgeEl.textContent = `À jour (${channel === "testing" ? "Canal Testing" : "Canal Stable"})`;
    }
  }

  // Radios de sélection de canal
  const radioStable = document.getElementById("radio-channel-stable");
  const radioTesting = document.getElementById("radio-channel-testing");
  const cardStable = document.getElementById("card-channel-stable");
  const cardTesting = document.getElementById("card-channel-testing");

  if (channel === "testing") {
    if (radioTesting) radioTesting.checked = true;
    if (cardTesting) cardTesting.classList.add("active");
    if (cardStable) cardStable.classList.remove("active");
  } else {
    if (radioStable) radioStable.checked = true;
    if (cardStable) cardStable.classList.add("active");
    if (cardTesting) cardTesting.classList.remove("active");
  }

  updateModalActionButton();
  modal.classList.remove("hidden");
}

function closeDashboardUpdateModal() {
  const modal = document.getElementById("dashboard-update-modal");
  if (modal) modal.classList.add("hidden");
}

function onDashboardChannelChange(channel) {
  selectedDashboardChannel = channel;
  const cardStable = document.getElementById("card-channel-stable");
  const cardTesting = document.getElementById("card-channel-testing");

  if (channel === "testing") {
    if (cardTesting) cardTesting.classList.add("active");
    if (cardStable) cardStable.classList.remove("active");
  } else {
    if (cardStable) cardStable.classList.add("active");
    if (cardTesting) cardTesting.classList.remove("active");
  }

  updateModalActionButton();
}

function updateModalActionButton() {
  const actionText = document.getElementById("modal-dash-action-text");
  if (!actionText) return;

  const currentChannel = (currentDashboardStatus?.dashboard_channel || "Stable").toLowerCase();
  if (selectedDashboardChannel !== currentChannel) {
    actionText.textContent = `Basculer vers le canal ${selectedDashboardChannel === 'testing' ? 'Testing' : 'Stable'}`;
  } else if (currentDashboardStatus?.dashboard_has_updates) {
    actionText.textContent = "Mettre à jour le Dashboard maintenant";
  } else {
    actionText.textContent = "Mettre à jour le Dashboard";
  }
}

function applyDashboardUpdateOrChannel() {
  closeDashboardUpdateModal();
  runAction("update-dashboard", selectedDashboardChannel);
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
    const confirmed = await showConfirmModal({
      title: "Supprimer le token",
      subtitle: "Paramètres GitHub",
      icon: "🗑️",
      message: "Le champ est vide. Souhaitez-vous supprimer le token existant ?",
      confirmText: "Supprimer le token",
      confirmIcon: "🗑️",
      confirmClass: "btn-danger",
      isDanger: true,
    });
    if (confirmed) {
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

    const elAnchorCount = document.getElementById("pkg-anchor-count");
    if (elAnchorCount) {
      elAnchorCount.textContent = `${totalCount} paquet(s) (${stableCount} stable, ${unstableCount} unstable)`;
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


// =============================================================================
// ⚡ SPEEDTEST COMPONENT ENGINE (Cloudflare Anycast + Real-time Canvas Graph)
// =============================================================================

let speedtestRunning = false;
let speedtestAbortController = null;
let currentPublicIp = "";
let ipHidden = true;

// Active speedtest server definition
let activeSpeedServer = {
  name: "Cloudflare Anycast (France/Europe)",
  endpoint: "https://speed.cloudflare.com",
};

// Graph history
let graphPoints = []; // { type: "down" | "up", mbps: number }
let graphMaxMbps = 100;

function initSpeedtest() {
  fetchSpeedtestMetadata();
  runNetworkCheckup(false);
  initSpeedtestCanvas();
  window.addEventListener("resize", () => {
    resizeSpeedtestCanvas();
    renderSpeedtestGraph();
  });
}

// 1. Fetch Location, ISP, CDN Node, and IP (via backend Rust natif pour fiabilité absolue sous KDE/GNOME)
async function fetchSpeedtestMetadata() {
  const locEl = document.getElementById("net-info-location");
  const ispEl = document.getElementById("net-info-isp");
  const srvEl = document.getElementById("net-info-server");
  const ipEl = document.getElementById("net-info-ip");

  if (locEl) locEl.textContent = "Détection de la ville...";
  if (ispEl) ispEl.textContent = "Recherche du FAI...";
  if (srvEl) srvEl.textContent = "Serveur : Optimisation...";

  // Méthode 1 : Invocation Rust native (utilise curl système, 100% insensible aux restrictions WebKit/KDE)
  try {
    const meta = await invoke("get_network_metadata");
    if (meta && meta.ip) {
      currentPublicIp = meta.ip;
      updateIpDisplay();

      const asnStr = meta.asn ? ` (AS${meta.asn})` : "";
      if (ispEl) ispEl.textContent = `Fournisseur : ${meta.isp}${asnStr}`;

      const locText = [meta.flag, meta.city, meta.country].filter(Boolean).join(" ");
      if (locEl) locEl.textContent = locText || "France";

      if (srvEl) srvEl.textContent = `Serveur : ${activeSpeedServer.name}`;
      return;
    }
  } catch (e) {
    console.warn("Échec get_network_metadata backend, tentative fallback webview fetch...", e);
  }

  // Méthode 2 : Fallback webview fetch ipwho.is
  let detected = false;
  try {
    const res = await fetch("https://ipwho.is/", {
      cache: "no-store",
      signal: AbortSignal.timeout(3000),
    });
    if (res.ok) {
      const data = await res.json();
      if (data && data.success !== false) {
        currentPublicIp = data.ip || "";
        updateIpDisplay();

        const ispName = data.connection?.isp || data.connection?.org || resolveIspFromAsn(data.connection?.asn);
        const asnStr = data.connection?.asn ? ` (AS${data.connection.asn})` : "";
        if (ispEl) ispEl.textContent = `Fournisseur : ${ispName}${asnStr}`;

        const city = data.city || "";
        const country = data.country || "";
        const flag = data.flag?.emoji || "";
        if (locEl) {
          const locText = [flag, city, country].filter(Boolean).join(" ");
          locEl.textContent = locText || "Localisation détectée";
        }
        detected = true;
      }
    }
  } catch (e) {
    console.warn("ipwho.is webview indisponible :", e);
  }

  if (!detected) {
    if (locEl) locEl.textContent = "Réseau local";
    if (ispEl) ispEl.textContent = "Fournisseur : Inconnu";
    if (ipEl) ipEl.textContent = "IP : Inconnue";
  }

  if (srvEl) {
    srvEl.textContent = `Serveur : ${activeSpeedServer.name}`;
  }
}

function resolveIspFromAsn(asn) {
  if (!asn) return "";
  const asns = {
    "3215": "Orange",
    "12322": "Free",
    "21502": "Bouygues Telecom",
    "5410": "Bouygues Telecom",
    "15557": "SFR / Altice",
    "6799": "OVHcloud",
    "13335": "Cloudflare",
    "15169": "Google",
    "8075": "Microsoft",
    "16509": "Amazon AWS",
    "5624": "Numericable",
    "5432": "Proximus",
    "3303": "Swisscom",
    "6830": "Liberty Global",
    "3320": "Deutsche Telekom",
  };
  return asns[String(asn)] || "";
}

function toggleIpVisibility() {
  ipHidden = !ipHidden;
  updateIpDisplay();
}

function updateIpDisplay() {
  const ipEl = document.getElementById("net-info-ip");
  if (!ipEl) return;
  if (!currentPublicIp) {
    ipEl.textContent = "IP : Non détectée";
    return;
  }
  if (ipHidden) {
    if (currentPublicIp.includes(":")) {
      const parts = currentPublicIp.split(":");
      ipEl.textContent = `IP : ${parts[0]}:••••:••••:${parts[parts.length - 1]}`;
    } else {
      const parts = currentPublicIp.split(".");
      ipEl.textContent = `IP : ${parts[0]}.•••.•••.${parts[3] || ""}`;
    }
  } else {
    ipEl.textContent = `IP : ${currentPublicIp}`;
  }
}

// 2. Toggle Start / Stop Speedtest
async function toggleSpeedtest() {
  if (speedtestRunning) {
    stopSpeedtest();
  } else {
    await startSpeedtest();
  }
}

function stopSpeedtest() {
  if (speedtestAbortController) {
    speedtestAbortController.abort();
    speedtestAbortController = null;
  }
  speedtestRunning = false;
  setSpeedtestUIState("ready");
}

async function runNetworkCheckup(manual = false) {
  const dotInternet = document.getElementById("dot-internet");
  const valInternet = document.getElementById("val-internet");
  const dotDns = document.getElementById("dot-dns");
  const valDns = document.getElementById("val-dns");
  const dotServer = document.getElementById("dot-server");
  const valServer = document.getElementById("val-server");

  if (!dotInternet || !dotDns || !dotServer) return null;

  // État initial : Pastilles bleues pulsantes "Test..."
  dotInternet.className = "checkup-dot dot-checking";
  if (valInternet) valInternet.textContent = "Test...";
  dotDns.className = "checkup-dot dot-checking";
  if (valDns) valDns.textContent = "Test...";
  dotServer.className = "checkup-dot dot-checking";
  if (valServer) valServer.textContent = "Test...";

  try {
    const diag = await invoke("diagnose_network");
    if (!diag) return null;

    // 1. Pastille Internet (IP)
    if (diag.internet_ip_ok) {
      dotInternet.className = "checkup-dot dot-ok";
      if (valInternet) valInternet.textContent = "Connecté";
    } else {
      dotInternet.className = "checkup-dot dot-error";
      if (valInternet) valInternet.textContent = "Déconnecté";
    }

    // 2. Pastille Résolution DNS
    if (diag.dns_ok) {
      dotDns.className = "checkup-dot dot-ok";
      if (valDns) valDns.textContent = "Résolu";
      dismissSpeedtestAlert();
    } else {
      dotDns.className = "checkup-dot dot-error";
      if (valDns) valDns.textContent = "Échec DNS";
      showSpeedtestAlert("🚨 Problème de Résolution DNS détecté", diag.details, true);
    }

    // 3. Pastille Serveur Speedtest
    if (diag.server_ok) {
      dotServer.className = "checkup-dot dot-ok";
      const lat = diag.server_latency_ms ? `${diag.server_latency_ms} ms` : "Prêt";
      if (valServer) valServer.textContent = `Prêt (${lat})`;
    } else if (diag.dns_ok) {
      dotServer.className = "checkup-dot dot-warn";
      if (valServer) valServer.textContent = "Injoignable";
      showSpeedtestAlert("⚠️ Serveur Speedtest Injoignable", "Le serveur de test ne répond pas sur le port 443.", false);
    } else {
      dotServer.className = "checkup-dot dot-error";
      if (valServer) valServer.textContent = "Bloqué (DNS)";
    }

    if (manual) {
      if (diag.internet_ip_ok && diag.dns_ok && diag.server_ok) {
        showToast("Diagnostic réseau : Tous les voyants sont au vert !", "success");
      } else {
        showToast(diag.details, "warning");
      }
    }

    return diag;
  } catch (err) {
    console.error("Échec runNetworkCheckup :", err);
    dotInternet.className = "checkup-dot dot-warn";
    dotDns.className = "checkup-dot dot-warn";
    dotServer.className = "checkup-dot dot-warn";
    return null;
  }
}

function dismissSpeedtestAlert() {
  const banner = document.getElementById("speedtest-alert-banner");
  if (banner) banner.style.display = "none";
}

function showSpeedtestAlert(title, desc, isDnsIssue = true) {
  const banner = document.getElementById("speedtest-alert-banner");
  const titleEl = document.getElementById("alert-banner-title");
  const descEl = document.getElementById("alert-banner-desc");
  const btn = document.getElementById("btn-repair-dns");
  const icon = document.getElementById("alert-banner-icon");

  if (!banner || !titleEl || !descEl) return;

  banner.classList.remove("resolved");
  titleEl.textContent = title;
  descEl.textContent = desc;
  if (icon) icon.textContent = isDnsIssue ? "⚠️" : "🌐";
  if (btn) {
    btn.style.display = isDnsIssue ? "inline-flex" : "none";
    const btnText = document.getElementById("btn-repair-dns-text");
    if (btnText) btnText.textContent = "Réparer le DNS";
    btn.disabled = false;
  }
  banner.style.display = "flex";
}

async function attemptDnsAutoRepair() {
  const btn = document.getElementById("btn-repair-dns");
  const btnText = document.getElementById("btn-repair-dns-text");
  const banner = document.getElementById("speedtest-alert-banner");
  const titleEl = document.getElementById("alert-banner-title");
  const descEl = document.getElementById("alert-banner-desc");

  if (btn) btn.disabled = true;
  if (btnText) btnText.textContent = "⏳ Réparation en cours...";
  showToast("Purge du cache DNS et réinitialisation de systemd-resolved...", "info");

  try {
    const res = await invoke("repair_network_dns");
    showToast(res || "Résolution DNS rétablie !", "success");

    if (banner) banner.classList.add("resolved");
    if (titleEl) titleEl.textContent = "✅ Résolution DNS rétablie avec succès !";
    if (descEl) descEl.textContent = "Le cache DNS a été purgé et le serveur DNS répond désormais. Relance automatique du speedtest...";
    if (btn) btn.style.display = "none";

    setTimeout(() => {
      dismissSpeedtestAlert();
      startSpeedtest();
    }, 1500);

  } catch (err) {
    console.error("Échec réparation automatique DNS :", err);
    showToast("Échec réparation : " + err, "error");
    if (btn) {
      btn.disabled = false;
      if (btnText) btnText.textContent = "Réessayer la réparation";
    }
    if (titleEl) titleEl.textContent = "❌ Serveur DNS toujours injoignable";
    if (descEl) descEl.textContent = (typeof err === "string" ? err : err.message) + " 💡 Conseil : Activez 'DNS Automatique (DHCP)' dans vos paramètres réseau de bureau pour utiliser le résolveur de votre box internet.";
  }
}

async function startSpeedtest() {
  if (speedtestRunning) return;
  speedtestRunning = true;
  speedtestAbortController = new AbortController();
  const signal = speedtestAbortController.signal;

  // Reset UI & masquage de l'alerte précédente
  dismissSpeedtestAlert();
  resetSpeedtestMetrics();
  setSpeedtestUIState("running");
  graphPoints = [];
  graphMaxMbps = 50;
  renderSpeedtestGraph();

  try {
    // Étape 0 : Diagnostic pré-vol complet avec pastilles
    const diag = await runNetworkCheckup(false);
    if (signal.aborted) return;

    if (diag && (!diag.internet_ip_ok || !diag.dns_ok)) {
      throw new Error(diag.details || "Échec du diagnostic de liaison réseau.");
    }

    const reachable = await verifySpeedtestServerConnectivity(signal);
    if (signal.aborted) return;
    if (!reachable) {
      // Diagnostic approfondi via le backend Rust
      let diagDetails = "Impossible de joindre le serveur de test.";
      try {
        const diag = await invoke("diagnose_network");
        if (diag) {
          if (diag.internet_ip_ok && !diag.dns_ok) {
            diagDetails = `Internet est accessible par adresse IP, mais votre résolveur DNS (${diag.current_dns}) ne répond pas ou bloque les requêtes.`;
            showSpeedtestAlert("🚨 Problème de Résolution DNS détecté", diagDetails, true);
          } else if (!diag.internet_ip_ok) {
            diagDetails = "Votre ordinateur n'a pas accès à internet (câble/Wi-Fi déconnecté ou passerelle injoignable).";
            showSpeedtestAlert("🌐 Pas d'accès internet", diagDetails, false);
          } else {
            diagDetails = diag.details || diagDetails;
            showSpeedtestAlert("⚠️ Serveur de test injoignable", diagDetails, true);
          }
        }
      } catch (diagErr) {
        console.warn("Échec diagnostic backend :", diagErr);
        showSpeedtestAlert("⚠️ Erreur de connexion DNS", "Impossible de joindre le serveur de test. Vérifiez votre connexion ou vos serveurs DNS.", true);
      }
      throw new Error(diagDetails);
    }

    // Phase 1 : Ping & Jitter
    await measurePingAndJitter(signal);
    if (signal.aborted) return;

    // Phase 2 : Download Speed (flux progressif adaptatif)
    await measureDownloadSpeed(signal);
    if (signal.aborted) return;

    // Phase 3 : Upload Speed (paquets adaptatifs 128 Ko / 256 Ko)
    await measureUploadSpeed(signal);
    if (signal.aborted) return;

    // Phase 4 : Diagnostic & Score
    finalizeSpeedtestScore();
    setSpeedtestUIState("finished");

  } catch (err) {
    if (signal.aborted) {
      console.log("Speedtest annulé par l'utilisateur");
    } else {
      console.error("Erreur durant le speedtest :", err);
      showToast(err.message, "error");
      const badge = document.getElementById("speedtest-status-badge");
      if (badge) {
        badge.textContent = "Erreur réseau / DNS";
        badge.className = "speedtest-status-badge";
      }
      setSpeedtestUIState("ready");
    }
  } finally {
    speedtestRunning = false;
    speedtestAbortController = null;
  }
}

// Vérification de la disponibilité du serveur et de la résolution DNS
async function verifySpeedtestServerConnectivity(signal) {
  const srvEl = document.getElementById("net-info-server");
  try {
    const res = await fetch("https://speed.cloudflare.com/__down?bytes=0&_check=" + Date.now(), {
      cache: "no-store",
      signal: AbortSignal.any([signal, AbortSignal.timeout(3500)]),
    });
    if (res.ok) {
      const colo = res.headers.get("cf-meta-colo") || res.headers.get("colo") || "";
      const coloCities = {
        CDG: "Paris Roissy", ORY: "Paris Orly", MRS: "Marseille", LYS: "Lyon",
        BOD: "Bordeaux", GVA: "Genève", BRU: "Bruxelles", LHR: "Londres", FRA: "Francfort",
      };
      const city = coloCities[colo] || colo || "Europe";
      activeSpeedServer.name = `Cloudflare Anycast (${city})`;
      if (srvEl) srvEl.textContent = `Serveur : ${activeSpeedServer.name}`;
      return true;
    }
  } catch (e) {
    console.warn("Échec connexion Cloudflare speedtest :", e);
  }
  return false;
}

function setSpeedtestUIState(state) {
  const btn = document.getElementById("btn-start-speedtest");
  const icon = document.getElementById("speedtest-btn-icon");
  const text = document.getElementById("speedtest-btn-text");
  const badge = document.getElementById("speedtest-status-badge");
  const liveDot = document.getElementById("graph-live-dot");

  if (!btn || !text || !badge) return;

  if (state === "running") {
    btn.classList.add("btn-running");
    icon.textContent = "⏹️";
    text.textContent = "Arrêter le Test";
    badge.textContent = "Test en cours";
    badge.className = "speedtest-status-badge running";
    if (liveDot) liveDot.classList.add("live");
  } else if (state === "finished") {
    btn.classList.remove("btn-running");
    icon.textContent = "🔄";
    text.textContent = "Relancer le Test";
    badge.textContent = "Terminé";
    badge.className = "speedtest-status-badge";
    if (liveDot) liveDot.classList.remove("live");
  } else {
    btn.classList.remove("btn-running");
    icon.textContent = "🚀";
    text.textContent = "Lancer le Speedtest";
    badge.textContent = "Prêt";
    badge.className = "speedtest-status-badge";
    if (liveDot) liveDot.classList.remove("live");
  }
}

function resetSpeedtestMetrics() {
  document.getElementById("speed-val-ping").textContent = "--";
  document.getElementById("speed-val-jitter").textContent = "-- ms";
  document.getElementById("ping-quality-tag").textContent = "Mesure en cours...";
  document.getElementById("ping-quality-tag").className = "speed-quality-tag";

  document.getElementById("speed-val-download").textContent = "--";
  document.getElementById("speed-transferred-down").textContent = "0 Mo";
  document.getElementById("phase-down-indicator").textContent = "En attente";
  document.getElementById("phase-down-indicator").className = "speed-phase-indicator";
  document.getElementById("progress-bar-down").style.width = "0%";

  document.getElementById("speed-val-upload").textContent = "--";
  document.getElementById("speed-transferred-up").textContent = "0 Mo";
  document.getElementById("phase-up-indicator").textContent = "En attente";
  document.getElementById("phase-up-indicator").className = "speed-phase-indicator";
  document.getElementById("progress-bar-up").style.width = "0%";

  // Reset usages
  ["gaming", "streaming", "cloud"].forEach(k => {
    const el = document.getElementById(`usage-${k}`);
    if (el) el.classList.remove("passed");
  });
  document.getElementById("usage-gaming-desc").textContent = "Évaluation en cours...";
  document.getElementById("usage-streaming-desc").textContent = "Évaluation en cours...";
  document.getElementById("usage-cloud-desc").textContent = "Évaluation en cours...";
}

// 3. Measure Ping & Jitter
async function measurePingAndJitter(signal) {
  const pingEl = document.getElementById("speed-val-ping");
  const jitterEl = document.getElementById("speed-val-jitter");
  const tagEl = document.getElementById("ping-quality-tag");
  const cardPing = document.getElementById("metric-card-ping");

  if (cardPing) cardPing.classList.add("active-measuring");

  const samples = [];
  const PING_COUNT = 8;

  for (let i = 0; i < PING_COUNT; i++) {
    if (signal.aborted) return;
    const t0 = performance.now();
    try {
      await fetch(`https://speed.cloudflare.com/__down?bytes=0&_t=${Date.now()}_${i}`, {
        cache: "no-store",
        signal: AbortSignal.any([signal, AbortSignal.timeout(2000)]),
      });
      const t1 = performance.now();
      const rtt = t1 - t0;
      samples.push(rtt);
      if (pingEl) pingEl.textContent = rtt.toFixed(0);
    } catch (e) {
      if (signal.aborted) return;
    }
    await new Promise(r => setTimeout(r, 60));
  }

  if (cardPing) cardPing.classList.remove("active-measuring");
  if (samples.length === 0) {
    if (pingEl) pingEl.textContent = "N/A";
    if (tagEl) tagEl.textContent = "Échec ping";
    return;
  }

  // Calcul du ping médian
  samples.sort((a, b) => a - b);
  const medianPing = samples[Math.floor(samples.length / 2)];

  // Calcul de la gigue (jitter) = moyenne des écarts consécutifs
  let jitterSum = 0;
  for (let i = 1; i < samples.length; i++) {
    jitterSum += Math.abs(samples[i] - samples[i - 1]);
  }
  const jitter = samples.length > 1 ? jitterSum / (samples.length - 1) : 0;

  if (pingEl) pingEl.textContent = medianPing.toFixed(1);
  if (jitterEl) jitterEl.textContent = `${jitter.toFixed(1)} ms`;

  if (tagEl) {
    if (medianPing < 20) {
      tagEl.textContent = "Excellent (< 20ms)";
      tagEl.className = "speed-quality-tag great";
    } else if (medianPing < 45) {
      tagEl.textContent = "Très bon (gaming)";
      tagEl.className = "speed-quality-tag great";
    } else if (medianPing < 90) {
      tagEl.textContent = "Correct";
      tagEl.className = "speed-quality-tag";
    } else {
      tagEl.textContent = "Élevé (> 90ms)";
      tagEl.className = "speed-quality-tag";
    }
  }
}

// 4. Measure Download Speed (Ramping & Adaptive Continuous Streams)
async function measureDownloadSpeed(signal) {
  const downEl = document.getElementById("speed-val-download");
  const transferredEl = document.getElementById("speed-transferred-down");
  const phaseEl = document.getElementById("phase-down-indicator");
  const barEl = document.getElementById("progress-bar-down");
  const cardDown = document.getElementById("metric-card-download");

  if (cardDown) cardDown.classList.add("active-measuring");
  if (phaseEl) {
    phaseEl.textContent = "En cours...";
    phaseEl.className = "speed-phase-indicator active";
  }

  const TEST_DURATION_MS = 7000;
  const startTime = performance.now();
  let totalBytes = 0;
  let lastBytes = 0;
  let lastTime = startTime;
  let currentMbps = 0;
  const speedSamples = [];

  const updateInterval = setInterval(() => {
    const now = performance.now();
    const elapsed = now - startTime;
    const dt = (now - lastTime) / 1000;
    const dBytes = totalBytes - lastBytes;

    if (dt > 0.05) {
      const instantMbps = (dBytes * 8) / (dt * 1000000);
      currentMbps = currentMbps === 0 ? instantMbps : (currentMbps * 0.6 + instantMbps * 0.4);

      if (elapsed > 400 && currentMbps > 0) {
        speedSamples.push(currentMbps);
      }

      if (downEl) downEl.textContent = currentMbps.toFixed(1);
      if (transferredEl) transferredEl.textContent = `${(totalBytes / 1048576).toFixed(1)} Mo`;

      const progressPct = Math.min(100, (elapsed / TEST_DURATION_MS) * 100);
      if (barEl) barEl.style.width = `${progressPct}%`;

      // Ajout du point au graphique
      graphPoints.push({ type: "down", mbps: currentMbps });
      if (currentMbps > graphMaxMbps) graphMaxMbps = currentMbps * 1.15;
      renderSpeedtestGraph();

      lastBytes = totalBytes;
      lastTime = now;
    }
  }, 60);

  // Worker adaptatif : démarre par de petits paquets pour décoller en < 50ms,
  // puis monte en puissance (1 Mo, 5 Mo, 15 Mo, 25 Mo) pour saturer la bande passante
  const downloadWorker = async (workerId) => {
    let chunkBytes = workerId === 0 ? 500000 : 1000000; // 500 Ko / 1 Mo au départ

    while (performance.now() - startTime < TEST_DURATION_MS && !signal.aborted) {
      const fetchUrl = `https://speed.cloudflare.com/__down?bytes=${chunkBytes}&_w=${workerId}&_t=${Date.now()}`;
      try {
        const res = await fetch(fetchUrl, {
          signal: AbortSignal.any([signal, AbortSignal.timeout(4000)]),
          cache: "no-store",
        });

        if (!res.ok) break;

        if (res.body && typeof res.body.getReader === "function") {
          const reader = res.body.getReader();
          while (performance.now() - startTime < TEST_DURATION_MS && !signal.aborted) {
            const { done, value } = await reader.read();
            if (done) break;
            if (value && value.byteLength) {
              totalBytes += value.byteLength;
            }
          }
          reader.cancel().catch(() => {});
        } else {
          // Fallback arrayBuffer si streaming non supporté par WebKit
          const buf = await res.arrayBuffer();
          totalBytes += buf.byteLength;
        }

        // Ramping : augmenter la taille des paquets suivants si le débit est élevé
        if (currentMbps > 50) {
          chunkBytes = Math.min(25000000, chunkBytes * 2);
        } else if (currentMbps > 15) {
          chunkBytes = Math.min(8000000, chunkBytes * 2);
        }
      } catch (e) {
        if (signal.aborted) break;
        // En cas d'erreur sur un gros chunk, redescendre à un paquet plus petit
        chunkBytes = 1000000;
        await new Promise(r => setTimeout(r, 80));
      }
    }
  };

  // Lancer 3 workers en parallèle
  await Promise.race([
    Promise.all([downloadWorker(0), downloadWorker(1), downloadWorker(2)]),
    new Promise(r => setTimeout(r, TEST_DURATION_MS)),
  ]);

  clearInterval(updateInterval);

  if (cardDown) cardDown.classList.remove("active-measuring");
  if (phaseEl) {
    phaseEl.textContent = "Terminé";
    phaseEl.className = "speed-phase-indicator";
  }
  if (barEl) barEl.style.width = "100%";

  let finalDownMbps = 0;
  if (speedSamples.length > 0) {
    speedSamples.sort((a, b) => a - b);
    const sliceStart = Math.floor(speedSamples.length * 0.25);
    const stableSamples = speedSamples.slice(sliceStart);
    finalDownMbps = stableSamples.reduce((a, b) => a + b, 0) / stableSamples.length;
  } else {
    finalDownMbps = currentMbps;
  }

  if (downEl) downEl.textContent = finalDownMbps.toFixed(1);
}

// 5. Measure Upload Speed (Paquets adaptatifs 128 Ko / 256 Ko)
async function measureUploadSpeed(signal) {
  const upEl = document.getElementById("speed-val-upload");
  const transferredEl = document.getElementById("speed-transferred-up");
  const phaseEl = document.getElementById("phase-up-indicator");
  const barEl = document.getElementById("progress-bar-up");
  const cardUp = document.getElementById("metric-card-upload");

  if (cardUp) cardUp.classList.add("active-upload");
  if (phaseEl) {
    phaseEl.textContent = "En cours...";
    phaseEl.className = "speed-phase-indicator active-up";
  }

  const TEST_DURATION_MS = 6000;
  const startTime = performance.now();
  let totalUploadedBytes = 0;
  let lastBytes = 0;
  let lastTime = startTime;
  let currentMbps = 0;
  const speedSamples = [];

  // Buffer de taille adaptée (256 Ko au lieu de 2 Mo) pour un décollage immédiat
  const CHUNK_SIZE = 256 * 1024;
  const uploadPayload = new Uint8Array(CHUNK_SIZE);
  for (let i = 0; i < CHUNK_SIZE; i += 2048) {
    uploadPayload[i] = (i * 17) & 0xff;
  }

  const updateInterval = setInterval(() => {
    const now = performance.now();
    const elapsed = now - startTime;
    const dt = (now - lastTime) / 1000;
    const dBytes = totalUploadedBytes - lastBytes;

    if (dt > 0.05) {
      const instantMbps = (dBytes * 8) / (dt * 1000000);
      currentMbps = currentMbps === 0 ? instantMbps : (currentMbps * 0.6 + instantMbps * 0.4);

      if (elapsed > 300 && currentMbps > 0) {
        speedSamples.push(currentMbps);
      }

      if (upEl) upEl.textContent = currentMbps.toFixed(1);
      if (transferredEl) transferredEl.textContent = `${(totalUploadedBytes / 1048576).toFixed(1)} Mo`;

      const progressPct = Math.min(100, (elapsed / TEST_DURATION_MS) * 100);
      if (barEl) barEl.style.width = `${progressPct}%`;

      // Ajout au graphique
      graphPoints.push({ type: "up", mbps: currentMbps });
      if (currentMbps > graphMaxMbps) graphMaxMbps = currentMbps * 1.15;
      renderSpeedtestGraph();

      lastBytes = totalUploadedBytes;
      lastTime = now;
    }
  }, 60);

  const uploadWorker = async () => {
    while (performance.now() - startTime < TEST_DURATION_MS && !signal.aborted) {
      try {
        const res = await fetch("https://speed.cloudflare.com/__up", {
          method: "POST",
          body: uploadPayload,
          signal: AbortSignal.any([signal, AbortSignal.timeout(3000)]),
          cache: "no-store",
        });
        if (res.ok) {
          totalUploadedBytes += CHUNK_SIZE;
        }
      } catch (e) {
        if (signal.aborted) break;
        await new Promise(r => setTimeout(r, 60));
      }
    }
  };

  // 2 workers concurrents d'envoi
  await Promise.race([
    Promise.all([uploadWorker(), uploadWorker()]),
    new Promise(r => setTimeout(r, TEST_DURATION_MS)),
  ]);

  clearInterval(updateInterval);

  if (cardUp) cardUp.classList.remove("active-upload");
  if (phaseEl) {
    phaseEl.textContent = "Terminé";
    phaseEl.className = "speed-phase-indicator";
  }
  if (barEl) barEl.style.width = "100%";

  let finalUpMbps = 0;
  if (speedSamples.length > 0) {
    speedSamples.sort((a, b) => a - b);
    const sliceStart = Math.floor(speedSamples.length * 0.25);
    const stableSamples = speedSamples.slice(sliceStart);
    finalUpMbps = stableSamples.reduce((a, b) => a + b, 0) / stableSamples.length;
  } else {
    finalUpMbps = currentMbps;
  }

  if (upEl) upEl.textContent = finalUpMbps.toFixed(1);
}

// 6. Quality of Experience Diagnostic
function finalizeSpeedtestScore() {
  const pingVal = parseFloat(document.getElementById("speed-val-ping").textContent) || 50;
  const downVal = parseFloat(document.getElementById("speed-val-download").textContent) || 0;
  const upVal = parseFloat(document.getElementById("speed-val-upload").textContent) || 0;

  // Gaming
  const gameBadge = document.getElementById("usage-gaming");
  const gameDesc = document.getElementById("usage-gaming-desc");
  if (pingVal <= 25) {
    gameBadge.classList.add("passed");
    gameDesc.textContent = `🌟 Idéal eSport & FPS compétitifs (Ping: ${pingVal.toFixed(0)} ms)`;
  } else if (pingVal <= 50) {
    gameBadge.classList.add("passed");
    gameDesc.textContent = `Très bon pour le multijoueur (${pingVal.toFixed(0)} ms)`;
  } else {
    gameDesc.textContent = `Latence modérée (${pingVal.toFixed(0)} ms)`;
  }

  // Streaming 4K / 8K
  const streamBadge = document.getElementById("usage-streaming");
  const streamDesc = document.getElementById("usage-streaming-desc");
  if (downVal >= 100) {
    streamBadge.classList.add("passed");
    streamDesc.textContent = "🌟 4K HDR & 8K Ultra HD multi-écrans sans chargement";
  } else if (downVal >= 30) {
    streamBadge.classList.add("passed");
    streamDesc.textContent = "Parfait pour Netflix & YouTube en 4K UHD";
  } else {
    streamDesc.textContent = "Idéal streaming HD 1080p";
  }

  // Cloud Gaming & P2P
  const cloudBadge = document.getElementById("usage-cloud");
  const cloudDesc = document.getElementById("usage-cloud-desc");
  if (downVal >= 150 && upVal >= 40 && pingVal <= 35) {
    cloudBadge.classList.add("passed");
    cloudDesc.textContent = "🌟 GeForce NOW / Steam Remote Play Ultra fluide";
  } else if (downVal >= 50 && upVal >= 15) {
    cloudBadge.classList.add("passed");
    cloudDesc.textContent = "Expérience Cloud Gaming 1080p 60 FPS optimale";
  } else {
    cloudDesc.textContent = "Téléchargements standards";
  }
}

function initSpeedtestCanvas() {
  resizeSpeedtestCanvas();
  renderSpeedtestGraph();
}

function resizeSpeedtestCanvas() {
  const canvas = document.getElementById("speedtest-canvas");
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
}

function renderSpeedtestGraph() {
  const canvas = document.getElementById("speedtest-canvas");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const width = canvas.width;
  const height = canvas.height;

  ctx.clearRect(0, 0, width, height);

  // Background grid
  ctx.strokeStyle = "rgba(255, 255, 255, 0.04)";
  ctx.lineWidth = 1;
  const gridLines = 3;
  for (let i = 1; i <= gridLines; i++) {
    const y = (height / (gridLines + 1)) * i;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();

    // Value indicator on right
    const val = (graphMaxMbps * (1 - i / (gridLines + 1))).toFixed(0);
    ctx.fillStyle = "rgba(166, 173, 200, 0.35)";
    ctx.font = `${Math.max(10, 11 * (window.devicePixelRatio || 1))}px 'JetBrains Mono', monospace`;
    ctx.textAlign = "right";
    ctx.fillText(`${val}M`, width - 8, y - 4);
  }

  if (graphPoints.length < 2) return;

  // Separate points by phase
  const downPoints = [];
  const upPoints = [];

  graphPoints.forEach((p, idx) => {
    const x = (idx / Math.max(graphPoints.length - 1, 1)) * width;
    const y = height - (Math.min(p.mbps, graphMaxMbps) / graphMaxMbps) * (height - 15) - 8;
    if (p.type === "down") {
      downPoints.push({ x, y, mbps: p.mbps });
    } else {
      upPoints.push({ x, y, mbps: p.mbps });
    }
  });

  // Render Download Curve (Sapphire / Cyan)
  if (downPoints.length > 1) {
    drawCurve(ctx, downPoints, "#74c7ec", "rgba(116, 199, 236, 0.22)", height);
  }

  // Render Upload Curve (Mauve / Lavender)
  if (upPoints.length > 1) {
    drawCurve(ctx, upPoints, "#cba6f7", "rgba(203, 166, 247, 0.22)", height);
  }
}

function drawCurve(ctx, pts, strokeColor, fillColor, canvasHeight) {
  ctx.save();

  // Draw Area Fill
  ctx.beginPath();
  ctx.moveTo(pts[0].x, canvasHeight);
  ctx.lineTo(pts[0].x, pts[0].y);

  for (let i = 1; i < pts.length; i++) {
    const prev = pts[i - 1];
    const curr = pts[i];
    const cpx = (prev.x + curr.x) / 2;
    ctx.quadraticCurveTo(prev.x, prev.y, cpx, (prev.y + curr.y) / 2);
  }
  ctx.lineTo(pts[pts.length - 1].x, pts[pts.length - 1].y);
  ctx.lineTo(pts[pts.length - 1].x, canvasHeight);
  ctx.closePath();

  const grad = ctx.createLinearGradient(0, 0, 0, canvasHeight);
  grad.addColorStop(0, fillColor);
  grad.addColorStop(1, "rgba(0, 0, 0, 0)");
  ctx.fillStyle = grad;
  ctx.fill();

  // Draw Stroke
  ctx.beginPath();
  ctx.moveTo(pts[0].x, pts[0].y);
  for (let i = 1; i < pts.length; i++) {
    const prev = pts[i - 1];
    const curr = pts[i];
    const cpx = (prev.x + curr.x) / 2;
    ctx.quadraticCurveTo(prev.x, prev.y, cpx, (prev.y + curr.y) / 2);
  }
  ctx.lineTo(pts[pts.length - 1].x, pts[pts.length - 1].y);
  ctx.strokeStyle = strokeColor;
  ctx.lineWidth = 2.5 * (window.devicePixelRatio || 1);
  ctx.shadowColor = strokeColor;
  ctx.shadowBlur = 8;
  ctx.stroke();

  // Glowing Head dot
  const last = pts[pts.length - 1];
  ctx.beginPath();
  ctx.arc(last.x, last.y, 4 * (window.devicePixelRatio || 1), 0, Math.PI * 2);
  ctx.fillStyle = "#ffffff";
  ctx.shadowBlur = 12;
  ctx.fill();

  ctx.restore();
}

window.toggleSpeedtest = toggleSpeedtest;
window.toggleIpVisibility = toggleIpVisibility;
window.initSpeedtest = initSpeedtest;


// =============================================================================
// 🛡️ FIREWALL & SECURITY MANAGEMENT COMPONENT
// =============================================================================

let currentFirewallState = {
  enabled: false,
  rules: []
};
let firewallFormType = "single";
let firewallFormProto = "both";
let currentFirewallFilter = "all";
let firewallDirty = false;

async function loadFirewallState() {
  try {
    const state = await invoke("get_firewall_state");
    if (!state) return;

    currentFirewallState = state;
    firewallDirty = false;
    updateFirewallUI();
  } catch (err) {
    console.error("Erreur chargement pare-feu :", err);
    showToast("Erreur chargement pare-feu : " + err, "error");
  }
}

function updateFirewallUI() {
  // 1. Global Switch & Pill
  const sw = document.getElementById("fw-global-switch");
  const pill = document.getElementById("fw-status-pill");
  const icon = document.getElementById("fw-shield-icon");
  const label = document.getElementById("fw-toggle-label");
  const sub = document.getElementById("fw-toggle-sub");

  if (sw) sw.checked = currentFirewallState.enabled;

  if (currentFirewallState.enabled) {
    if (pill) {
      pill.textContent = "Actif (Filtrage Actif)";
      pill.className = "fw-status-pill active";
    }
    if (icon) icon.className = "fw-shield-icon-box active";
    if (label) label.textContent = "Pare-feu Activé";
    if (sub) sub.textContent = "Filtrage réseau strict NixOS activé";
  } else {
    if (pill) {
      pill.textContent = "Désactivé";
      pill.className = "fw-status-pill disabled";
    }
    if (icon) icon.className = "fw-shield-icon-box";
    if (label) label.textContent = "Pare-feu Désactivé";
    if (sub) sub.textContent = "Tous les ports entrants sont non filtrés";
  }

  // 2. Stats
  let tcpCount = 0;
  let udpCount = 0;
  let rangeCount = 0;

  currentFirewallState.rules.forEach(r => {
    if (r.rule_type === "single") {
      if (r.protocol === "tcp" || r.protocol === "both") tcpCount++;
      if (r.protocol === "udp" || r.protocol === "both") udpCount++;
    } else if (r.rule_type === "range") {
      rangeCount++;
    }
  });

  const tcpEl = document.getElementById("fw-stat-tcp");
  const udpEl = document.getElementById("fw-stat-udp");
  const rangesEl = document.getElementById("fw-stat-ranges");

  if (tcpEl) tcpEl.textContent = tcpCount;
  if (udpEl) udpEl.textContent = udpCount;
  if (rangesEl) rangesEl.textContent = rangeCount;

  // 3. Render Rules
  renderFirewallRules();
  updateFirewallDirtyUI();
}

function toggleFirewallGlobal(checked) {
  currentFirewallState.enabled = checked;
  firewallDirty = true;
  updateFirewallUI();
}

function setFirewallFormType(type) {
  firewallFormType = type;

  const btnSingle = document.querySelector('#fw-type-selector [data-type="single"]');
  const btnRange = document.querySelector('#fw-type-selector [data-type="range"]');
  const wrapSingle = document.getElementById("fw-wrap-single-port");
  const wrapRange = document.getElementById("fw-wrap-range-ports");

  if (btnSingle && btnRange) {
    btnSingle.classList.toggle("active", type === "single");
    btnRange.classList.toggle("active", type === "range");
  }

  if (wrapSingle && wrapRange) {
    if (type === "single") {
      wrapSingle.classList.remove("hidden");
      wrapRange.classList.add("hidden");
    } else {
      wrapSingle.classList.add("hidden");
      wrapRange.classList.remove("hidden");
    }
  }
}

function setFirewallFormProto(proto) {
  firewallFormProto = proto;

  const btns = document.querySelectorAll("#fw-proto-selector .fw-pill-btn");
  btns.forEach(b => {
    b.classList.toggle("active", b.getAttribute("data-proto") === proto);
  });
}

function applyFirewallPreset(type, proto, port, from, to, label) {
  setFirewallFormType(type);
  setFirewallFormProto(proto);

  if (type === "single") {
    const portInput = document.getElementById("fw-input-port");
    if (portInput) portInput.value = port;
  } else {
    const fromInput = document.getElementById("fw-input-from");
    const toInput = document.getElementById("fw-input-to");
    if (fromInput) fromInput.value = from;
    if (toInput) toInput.value = to;
  }

  const labelInput = document.getElementById("fw-input-label");
  if (labelInput) labelInput.value = label;

  showToast(`Preset « ${label} » chargé !`, "info");
}

function addFirewallRuleFromUI() {
  const labelInput = document.getElementById("fw-input-label");
  let label = labelInput ? labelInput.value.trim() : "";

  if (firewallFormType === "single") {
    const portInput = document.getElementById("fw-input-port");
    const portVal = parseInt(portInput ? portInput.value : "0", 10);

    if (isNaN(portVal) || portVal < 1 || portVal > 65535) {
      showToast("Veuillez saisir un numéro de port valide (1 à 65535)", "error");
      return;
    }

    if (!label) label = `Port ${portVal}`;

    // Vérifier si la règle existe déjà
    const existing = currentFirewallState.rules.find(
      r => r.rule_type === "single" && r.port === portVal && (r.protocol === firewallFormProto || r.protocol === "both" || firewallFormProto === "both")
    );

    if (existing) {
      showToast(`Le port ${portVal} est déjà ouvert (${existing.protocol.toUpperCase()})`, "error");
      return;
    }

    const newRule = {
      id: `single-${firewallFormProto}-${portVal}-${Date.now()}`,
      rule_type: "single",
      protocol: firewallFormProto,
      port: portVal,
      from_port: null,
      to_port: null,
      label,
      is_default: false
    };

    currentFirewallState.rules.push(newRule);
    if (portInput) portInput.value = "";
    if (labelInput) labelInput.value = "";

  } else {
    const fromInput = document.getElementById("fw-input-from");
    const toInput = document.getElementById("fw-input-to");
    const fromVal = parseInt(fromInput ? fromInput.value : "0", 10);
    const toVal = parseInt(toInput ? toInput.value : "0", 10);

    if (isNaN(fromVal) || fromVal < 1 || fromVal > 65535 || isNaN(toVal) || toVal < 1 || toVal > 65535) {
      showToast("Veuillez renseigner des ports de début et de fin valides (1 à 65535)", "error");
      return;
    }

    if (fromVal > toVal) {
      showToast("Le port de début doit être inférieur ou égal au port de fin", "error");
      return;
    }

    if (!label) label = `Plage ${fromVal} - ${toVal}`;

    const newRule = {
      id: `range-${firewallFormProto}-${fromVal}-${toVal}-${Date.now()}`,
      rule_type: "range",
      protocol: firewallFormProto,
      port: null,
      from_port: fromVal,
      to_port: toVal,
      label,
      is_default: false
    };

    currentFirewallState.rules.push(newRule);
    if (fromInput) fromInput.value = "";
    if (toInput) toInput.value = "";
    if (labelInput) labelInput.value = "";
  }

  firewallDirty = true;
  updateFirewallUI();
  showToast("Règle ajoutée avec succès ! Pensez à enregistrer et appliquer.", "success");
}

async function deleteFirewallRule(id) {
  const rule = currentFirewallState.rules.find(r => r.id === id);
  if (!rule) return;

  const desc = rule.rule_type === "single"
    ? `le port ${rule.port} (${rule.protocol.toUpperCase()})`
    : `la plage ${rule.from_port} - ${rule.to_port} (${rule.protocol.toUpperCase()})`;

  const confirmed = await showConfirmModal({
    title: "Supprimer l'ouverture de port",
    message: `Voulez-vous vraiment retirer ${desc} ?`,
    detail: rule.label,
    confirmText: "Supprimer la règle",
    cancelText: "Annuler",
    isDanger: true
  });

  if (!confirmed) return;

  currentFirewallState.rules = currentFirewallState.rules.filter(r => r.id !== id);
  firewallDirty = true;
  updateFirewallUI();
  showToast("Règle retirée. N'oubliez pas d'enregistrer et appliquer.", "info");
}

function filterFirewallRules(filter) {
  currentFirewallFilter = filter;
  const btns = document.querySelectorAll(".fw-filter-btn");
  btns.forEach(b => {
    b.classList.toggle("active", b.getAttribute("data-filter") === filter);
  });
  renderFirewallRules();
}

function renderFirewallRules() {
  const container = document.getElementById("fw-rules-list");
  const countBadge = document.getElementById("fw-rules-count-badge");
  if (!container) return;

  let filtered = currentFirewallState.rules;
  if (currentFirewallFilter === "single") {
    filtered = filtered.filter(r => r.rule_type === "single");
  } else if (currentFirewallFilter === "range") {
    filtered = filtered.filter(r => r.rule_type === "range");
  }

  if (countBadge) {
    countBadge.textContent = `${currentFirewallState.rules.length} règle(s)`;
  }

  if (filtered.length === 0) {
    container.innerHTML = `
      <div class="fw-empty-state">
        <div class="fw-empty-icon">🛡️</div>
        <h4>Aucune règle correspondant au filtre</h4>
        <p>Utilisez le formulaire ci-dessus pour ouvrir des ports dans modules/core/firewall.nix</p>
      </div>
    `;
    return;
  }

  let html = "";
  filtered.forEach(r => {
    let protoClass = "proto-both";
    let protoText = "TCP + UDP";
    if (r.protocol === "tcp") {
      protoClass = "proto-tcp";
      protoText = "TCP";
    } else if (r.protocol === "udp") {
      protoClass = "proto-udp";
      protoText = "UDP";
    }

    const portDisplay = r.rule_type === "single"
      ? `<span class="fw-port-num">${r.port}</span>`
      : `<span class="fw-port-num">${r.from_port} ➔ ${r.to_port}</span>`;

    const statusTag = r.is_default
      ? `<span class="fw-default-tag" title="Règle système intégrée (modules/core/firewall.nix)">🔒 Système</span>`
      : `<span class="fw-custom-tag" title="Règle personnalisée (firewall-user.nix)">Personnalisé</span>`;

    const actionBtn = r.is_default
      ? `<button type="button" class="btn-del-rule disabled" disabled title="Règle système protégée (modules/core/firewall.nix)">🔒</button>`
      : `<button type="button" class="btn-del-rule" onclick="deleteFirewallRule('${r.id}')" title="Supprimer cette ouverture de port">🗑️</button>`;

    html += `
      <div class="fw-rule-item" id="fw-rule-${r.id}">
        <div class="fw-rule-left">
          <span class="fw-proto-badge ${protoClass}">${protoText}</span>
          <div class="fw-port-display">
            ${portDisplay}
          </div>
          <div class="fw-rule-info">
            <span class="fw-rule-label">${escapeHtml(r.label || "Sans libellé")}</span>
            <span class="fw-rule-meta">${r.rule_type === "single" ? "Port unique" : "Plage de ports"}</span>
          </div>
        </div>
        <div class="fw-rule-right">
          ${statusTag}
          ${actionBtn}
        </div>
      </div>
    `;
  });

  container.innerHTML = html;
}

function updateFirewallDirtyUI() {
  const badge = document.getElementById("firewall-dirty-badge");
  const anchor = document.getElementById("fw-floating-anchor");
  const title = document.getElementById("fw-anchor-title");
  const sub = document.getElementById("fw-anchor-sub");

  if (badge) badge.classList.toggle("hidden", !firewallDirty);

  if (anchor) {
    if (firewallDirty) {
      anchor.classList.add("visible");
      if (title) title.textContent = "Modifications pare-feu en attente";
      if (sub) sub.textContent = "Cliquez sur Appliquer pour sauvegarder firewall.nix et reconstruire";
    } else {
      anchor.classList.remove("visible");
      if (title) title.textContent = "Configuration Pare-feu Synchronisée";
      if (sub) sub.textContent = "Prêt pour déploiement immédiat dans le noyau NixOS";
    }
  }
}

async function applyFirewallDeploy() {
  try {
    showToast("Enregistrement de firewall-user.nix...", "info");

    await invoke("save_firewall_state", {
      enabled: currentFirewallState.enabled,
      rules: currentFirewallState.rules
    });

    firewallDirty = false;
    updateFirewallDirtyUI();

    showToast("Fichier firewall-user.nix mis à jour avec succès !", "success");

    // Lancer la reconstruction NixOS
    runTerminalTask("apply-firewall", "🛡️ Application des règles du pare-feu NixOS (nh os switch)");

  } catch (err) {
    console.error("Erreur enregistrement pare-feu :", err);
    showToast("Erreur lors de l'enregistrement : " + err, "error");
  }
}

window.loadFirewallState = loadFirewallState;
window.toggleFirewallGlobal = toggleFirewallGlobal;
window.setFirewallFormType = setFirewallFormType;
window.setFirewallFormProto = setFirewallFormProto;
window.applyFirewallPreset = applyFirewallPreset;
window.addFirewallRuleFromUI = addFirewallRuleFromUI;
window.deleteFirewallRule = deleteFirewallRule;
window.filterFirewallRules = filterFirewallRules;
window.applyFirewallDeploy = applyFirewallDeploy;


// ==========================================================================
// 8. Commit Cryptographic Security & Anti-MitM Inspection
// ==========================================================================

async function loadCommitSecurityInfo() {
  try {
    const info = await invoke("get_commit_security_info");
    if (!info) return;

    const badge = document.getElementById("meta-security-badge");
    const icon = document.getElementById("meta-sec-icon");
    const text = document.getElementById("meta-sec-text");
    if (!badge) return;

    badge.classList.remove("hidden", "verified", "unverified", "bad");

    if (info.status === "verified") {
      badge.classList.add("verified");
      if (icon) icon.textContent = "🔒";
      if (text) text.textContent = "Signé (" + info.short_hash + " • " + info.signer + ")";
      badge.title = "Commit authentifié cryptographiquement par " + info.signer + "\nDate : " + info.date + "\nMessage : " + info.subject;
    } else if (info.status === "bad") {
      badge.classList.add("bad");
      if (icon) icon.textContent = "🚨";
      if (text) text.textContent = "Signature Invalide (" + info.short_hash + ")";
      badge.title = "ALERTE CRITIQUE : La signature de ce commit est corrompue ou falsifiée !";
    } else {
      badge.classList.add("unverified");
      if (icon) icon.textContent = "ℹ️";
      if (text) text.textContent = "Non signé (" + info.short_hash + ")";
      badge.title = "Commit non signé cryptographiquement (" + info.date + ")";
    }
  } catch (e) {
    console.warn("Échec inspection signature commit:", e);
  }
}

window.loadCommitSecurityInfo = loadCommitSecurityInfo;


// =========================================================================
// 🌐 CENTRE RÉSEAU : SOUS-NAVIGATION, GESTION DNS & CONTENEURS PODMAN
// =========================================================================

let currentDnsCatalog = null;
let selectedDnsProviderId = "default";
let activeDnsFilterCategory = "all";
let dnsPingCache = {};
let currentPodmanOverview = null;
let currentPodmanLogUnit = null;

function initNetworkCenter() {
  loadDnsCatalog(false);
  loadPodmanOverview(false);
  loadSftpOverview(false);
}

function switchNetworkSubtab(subtabId) {
  const subnavBtns = document.querySelectorAll(".net-subnav-btn");
  subnavBtns.forEach(btn => {
    if (btn.getAttribute("data-subtab") === subtabId) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
  });

  const panes = document.querySelectorAll(".net-subtab-pane");
  panes.forEach(pane => pane.classList.remove("active"));

  const targetPane = document.getElementById(subtabId);
  if (targetPane) {
    targetPane.classList.add("active");
  }

  if (subtabId === "net-subtab-dns") {
    if (!currentDnsCatalog) {
      loadDnsCatalog(true);
    } else if (Object.keys(dnsPingCache).length === 0) {
      pingAllDnsServers();
    }
  } else if (subtabId === "net-subtab-podman") {
    loadPodmanOverview(false);
  } else if (subtabId === "net-subtab-sftp") {
    loadSftpOverview(false);
  }
}

// -------------------------------------------------------------------------
// GESTION DES RÉSOLVEURS DNS
// -------------------------------------------------------------------------

async function loadDnsCatalog(autoPing = false) {
  try {
    const catalog = await invoke("get_dns_catalog");
    if (!catalog) return;

    currentDnsCatalog = catalog;

    // Mise à jour de la bannière DNS actif
    const displayEl = document.getElementById("dns-current-display");
    const tagEl = document.getElementById("dns-current-provider-tag");
    if (displayEl) displayEl.textContent = catalog.current_dns || "Automatique (DHCP)";

    const activeProvider = catalog.providers.find(p => p.id === catalog.active_provider_id);
    if (tagEl) {
      tagEl.textContent = activeProvider ? activeProvider.name : "Personnalisé / Système";
    }

    // Fournisseur sélectionné par défaut
    selectedDnsProviderId = catalog.active_provider_id || "default";

    // Remplir les champs personnalisés si existants
    const inputPrim = document.getElementById("dns-input-primary");
    const inputSec = document.getElementById("dns-input-secondary");
    if (inputPrim && catalog.custom_primary) inputPrim.value = catalog.custom_primary;
    if (inputSec && catalog.custom_secondary) inputSec.value = catalog.custom_secondary;

    renderDnsCards();
    updateDnsApplyAnchor();

    if (autoPing) {
      pingAllDnsServers();
    }
  } catch (err) {
    console.error("Erreur chargement catalogue DNS :", err);
  }
}

function renderDnsCards() {
  const container = document.getElementById("dns-cards-container");
  if (!container || !currentDnsCatalog) return;

  container.innerHTML = "";

  currentDnsCatalog.providers.forEach(p => {
    // Filtrage par catégorie
    if (activeDnsFilterCategory !== "all" && p.category !== activeDnsFilterCategory && p.id !== "custom" && p.id !== "default") {
      return;
    }

    const isSelected = p.id === selectedDnsProviderId;
    const isSystemActive = p.id === currentDnsCatalog.active_provider_id;

    const card = document.createElement("div");
    card.className = `dns-card ${isSelected ? "selected" : ""} ${isSystemActive ? "active-system" : ""}`;
    card.setAttribute("data-provider-id", p.id);
    card.onclick = () => selectDnsProvider(p.id);

    // Ping badge
    let pingText = "-- ms";
    let pingClass = "testing";
    const lookupKey = p.id === "custom" ? (p.primary_ip || "custom") : p.primary_ip;
    const cachedPing = dnsPingCache[lookupKey] !== undefined ? dnsPingCache[lookupKey] : (p.id === "custom" ? dnsPingCache["custom"] : undefined);
    if (cachedPing !== undefined) {
      if (cachedPing === null) {
        pingText = "Injoignable";
        pingClass = "slow";
      } else {
        pingText = `⚡ ${cachedPing} ms`;
        if (cachedPing < 30) pingClass = "fast";
        else if (cachedPing < 70) pingClass = "good";
        else if (cachedPing < 150) pingClass = "medium";
        else pingClass = "slow";
      }
    } else if (p.id === "default") {
      pingText = "Système (DHCP)";
      pingClass = "good";
    } else if (p.id === "custom") {
      pingText = "Manuel";
      pingClass = "testing";
    }

    const tagsHtml = (p.tags || []).map(t => `<span class="dns-tag-pill">${escapeHtml(t)}</span>`).join("");

    const ipsDisplay = p.primary_ip && p.primary_ip !== "Automatique" 
      ? `<span>${escapeHtml(p.primary_ip)}${p.secondary_ip ? ' &nbsp;|&nbsp; ' + escapeHtml(p.secondary_ip) : ''}</span>`
      : `<span>Gestion DHCP / Automatique</span>`;

    card.innerHTML = `
      <div class="dns-card-header">
        <div class="dns-card-title-group">
          <span class="dns-card-icon">${p.icon || '⚡'}</span>
          <div>
            <h4>${escapeHtml(p.name)}</h4>
          </div>
        </div>
        <div class="dns-card-badges">
          ${isSystemActive ? '<span class="dns-card-active-tag">Actif</span>' : ''}
          <span class="dns-ping-badge ${pingClass}" id="dns-badge-${p.id}">${pingText}</span>
        </div>
      </div>
      <p class="dns-card-desc">${escapeHtml(p.description)}</p>
      <div class="dns-ips-box">
        <small>Adresses IPv4 :</small>
        ${ipsDisplay}
      </div>
      <div class="dns-tags-row">
        ${tagsHtml}
      </div>
      <div class="dns-card-radio-wrap">
        <button type="button" class="dns-select-btn">
          ${isSelected ? '✓ Sélectionné' : 'Sélectionner'}
        </button>
      </div>
    `;

    container.appendChild(card);
  });
}

function filterDnsCategory(category) {
  activeDnsFilterCategory = category;
  const buttons = document.querySelectorAll(".dns-filter-btn");
  buttons.forEach(btn => {
    if (btn.getAttribute("data-cat") === category) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
  });
  renderDnsCards();
}

function selectDnsProvider(id) {
  selectedDnsProviderId = id;
  const customBox = document.getElementById("dns-custom-box");
  if (customBox) {
    if (id === "custom") {
      customBox.classList.remove("hidden");
    } else {
      customBox.classList.add("hidden");
    }
  }

  renderDnsCards();
  updateDnsApplyAnchor();
}

function onCustomDnsChange() {
  updateDnsApplyAnchor();
}

function updateDnsApplyAnchor() {
  if (!currentDnsCatalog) return;
  const provider = currentDnsCatalog.providers.find(p => p.id === selectedDnsProviderId);
  const titleEl = document.getElementById("dns-anchor-selected-title");
  const subEl = document.getElementById("dns-anchor-selected-sub");

  if (titleEl) {
    if (selectedDnsProviderId === "custom") {
      const prim = document.getElementById("dns-input-primary")?.value.trim() || "";
      titleEl.textContent = prim ? `DNS Personnalisé : ${prim}` : "DNS Personnalisé (Saisir IP)";
    } else if (provider) {
      titleEl.textContent = `Fournisseur sélectionné : ${provider.name}`;
    }
  }

  if (subEl) {
    if (selectedDnsProviderId === currentDnsCatalog.active_provider_id) {
      subEl.textContent = "Actuellement configuré sur votre système NixOS";
    } else {
      subEl.textContent = "Prêt pour application immédiate (à chaud) ou enregistrement permanent";
    }
  }
}

async function pingAllDnsServers() {
  if (!currentDnsCatalog) return;

  const spinIcon = document.getElementById("dns-ping-spin");
  if (spinIcon) spinIcon.classList.add("spinning");

  // Collecter toutes les adresses IP uniques
  const ips = [];
  currentDnsCatalog.providers.forEach(p => {
    if (p.primary_ip && p.primary_ip !== "Automatique") {
      if (!ips.includes(p.primary_ip)) ips.push(p.primary_ip);
    }
  });

  // Mettre à jour l'état visuel en test
  currentDnsCatalog.providers.forEach(p => {
    const badge = document.getElementById(`dns-badge-${p.id}`);
    if (badge && p.primary_ip && p.primary_ip !== "Automatique") {
      badge.textContent = "Test...";
      badge.className = "dns-ping-badge testing";
    }
  });

  try {
    const results = await invoke("ping_dns_servers", { ips });
    if (results) {
      Object.assign(dnsPingCache, results);
      renderDnsCards();
      showToast("Latence des résolveurs DNS actualisée !", "info");
    }
  } catch (err) {
    console.error("Erreur lors du ping des serveurs DNS :", err);
  } finally {
    if (spinIcon) spinIcon.classList.remove("spinning");
  }
}

async function pingCustomDns() {
  const prim = document.getElementById("dns-input-primary")?.value.trim();
  const sec = document.getElementById("dns-input-secondary")?.value.trim();
  const ips = [];
  if (prim) ips.push(prim);
  if (sec) ips.push(sec);

  if (ips.length === 0) {
    showToast("Veuillez saisir au moins une adresse IPv4 valide", "error");
    return;
  }

  try {
    const results = await invoke("ping_dns_servers", { ips });
    if (results) {
      Object.assign(dnsPingCache, results);
      if (prim && results[prim] !== undefined) {
        dnsPingCache["custom"] = results[prim];
      }
      renderDnsCards();
      let msg = "";
      ips.forEach(ip => {
        const ms = results[ip];
        msg += `${ip} : ${ms !== null && ms !== undefined ? ms + " ms" : "Injoignable"} | `;
      });
      showToast(msg.slice(0, -3), "info");
    }
  } catch (err) {
    showToast("Erreur lors du test de latence : " + err, "error");
  }
}

async function applySelectedDns(applyRuntime, savePermanent) {
  const customIps = [];
  if (selectedDnsProviderId === "custom") {
    const prim = document.getElementById("dns-input-primary")?.value.trim() || "";
    const sec = document.getElementById("dns-input-secondary")?.value.trim() || "";
    if (!prim) {
      showToast("Veuillez renseigner au moins l'adresse IP primaire", "error");
      return;
    }
    customIps.push(prim);
    if (sec) customIps.push(sec);
  }

  try {
    const msg = await invoke("apply_dns_server", {
      providerId: selectedDnsProviderId,
      customIps,
      applyRuntime,
      savePermanent,
    });

    showToast(msg || "Serveurs DNS appliqués avec succès !", "success");
    await loadDnsCatalog(false);
  } catch (err) {
    console.error("Erreur application DNS :", err);
    showToast("Erreur application DNS : " + err, "error");
  }
}

// -------------------------------------------------------------------------
// GESTION & ACTIVITÉ DES CONTENEURS PODMAN
// -------------------------------------------------------------------------

async function loadPodmanOverview(showToastFlag = false) {
  try {
    const overview = await invoke("get_podman_overview");
    if (!overview) return;

    currentPodmanOverview = overview;

    // 1. Stats Bar
    const runningEl = document.getElementById("podman-stat-running");
    const memoryEl = document.getElementById("podman-stat-memory");
    const portsEl = document.getElementById("podman-stat-ports");
    const versionEl = document.getElementById("podman-stat-version");
    const badgeEl = document.getElementById("podman-badge-count");
    const countBadgeEl = document.getElementById("podman-containers-badge");

    if (runningEl) runningEl.textContent = `${overview.running_containers} / ${overview.total_containers}`;
    if (memoryEl) memoryEl.textContent = overview.total_memory_human || "0 Mo";

    let totalPorts = 0;
    overview.containers.forEach(c => {
      totalPorts += (c.ports || []).length;
    });
    if (portsEl) portsEl.textContent = totalPorts.toString();
    if (versionEl) versionEl.textContent = overview.engine_version.replace("podman version ", "Podman ");
    if (badgeEl) badgeEl.textContent = overview.running_containers.toString();
    if (countBadgeEl) countBadgeEl.textContent = `${overview.total_containers} service(s) OCI`;

    renderPodmanContainers();
    populatePodmanLogSelect();

    if (showToastFlag) {
      showToast("Activité des conteneurs Podman actualisée !", "info");
    }
  } catch (err) {
    console.error("Erreur chargement aperçu Podman :", err);
  }
}

function renderPodmanContainers() {
  const grid = document.getElementById("podman-containers-grid");
  if (!grid || !currentPodmanOverview) return;

  grid.innerHTML = "";

  if (currentPodmanOverview.containers.length === 0) {
    grid.innerHTML = `
      <div class="card" style="padding: 24px; text-align: center; grid-column: 1 / -1;">
        <span style="font-size: 2rem; display: block; margin-bottom: 8px;">🦭</span>
        <p style="color: var(--subtext0);">Aucun conteneur OCI/Podman configuré ou actif actuellement.</p>
        <small style="color: var(--overlay1);">Les conteneurs de la Suite IA locale (Open WebUI, Hermes Agent) apparaîtront ici dès leur activation.</small>
      </div>
    `;
    return;
  }

  currentPodmanOverview.containers.forEach(c => {
    const card = document.createElement("div");
    card.className = `podman-card ${c.is_active ? "running" : "stopped"}`;

    const icon = c.name.includes("open-webui") ? "🌐" : c.name.includes("hermes") ? "🤖" : "📦";

    // Ports chips
    let portsHtml = "";
    if (c.ports && c.ports.length > 0) {
      portsHtml = c.ports.map(p => {
        if (p.url && c.is_active) {
          return `<a href="#" class="podman-port-chip" onclick="openExternalBrowserUrl('${p.url}'); return false;" title="Ouvrir dans le navigateur">
            <span>🚪 Port ${p.host_port} (${escapeHtml(p.protocol)})</span>
            <span>↗</span>
          </a>`;
        } else {
          return `<span class="podman-port-chip-plain">🚪 Port ${p.host_port} (${escapeHtml(p.protocol)})</span>`;
        }
      }).join("");
    } else {
      portsHtml = `<span style="font-size: 0.78rem; color: var(--overlay1);">Aucun port mappé ou mode host direct</span>`;
    }

    // Config parameters
    let configItemsHtml = "";
    if (c.env_summary) {
      const keys = Object.keys(c.env_summary);
      configItemsHtml = keys.slice(0, 4).map(k => `
        <div class="podman-config-item">
          <span class="podman-config-key">${escapeHtml(k)} :</span>
          <span class="podman-config-val">${escapeHtml(c.env_summary[k])}</span>
        </div>
      `).join("");
    }

    card.innerHTML = `
      <div class="podman-card-header">
        <div class="podman-card-identity">
          <span class="podman-card-icon">${icon}</span>
          <div>
            <h4>${escapeHtml(c.display_name)}</h4>
            <small>${escapeHtml(c.unit_name)}</small>
          </div>
        </div>
        <span class="podman-status-badge ${c.is_active ? "active" : "stopped"}">
          ● ${escapeHtml(c.status_text)}
        </span>
      </div>

      <div class="podman-card-image" title="${escapeHtml(c.image)}">
        🐳 ${escapeHtml(c.image)}
      </div>

      <div class="podman-metrics-row">
        <div class="podman-metric-item">
          <span>💾</span>
          <div>
            <strong>${escapeHtml(c.memory_human)}</strong>
            <small>Mémoire RAM</small>
          </div>
        </div>
        <div class="podman-metric-item">
          <span>⚙️</span>
          <div>
            <strong>${c.cpu_usage_sec} s</strong>
            <small>Temps CPU</small>
          </div>
        </div>
        <div class="podman-metric-item">
          <span>🌐</span>
          <div>
            <strong>${escapeHtml(c.network_mode.split(' ')[0])}</strong>
            <small>Mode Réseau</small>
          </div>
        </div>
      </div>

      <div class="podman-ports-group">
        <span class="podman-ports-label">Accès Réseau & Interfaces :</span>
        <div class="podman-ports-list">
          ${portsHtml}
        </div>
      </div>

      ${configItemsHtml ? `
        <div class="podman-config-box">
          <span class="podman-config-title">Paramètres & Choix de Configuration</span>
          <div class="podman-config-list">
            ${configItemsHtml}
          </div>
        </div>
      ` : ''}

      <div class="podman-card-actions">
        <button type="button" class="btn btn-secondary btn-sm" onclick="viewContainerLogs('${escapeHtml(c.unit_name)}')">
          <span>📜</span> Logs
        </button>
        ${c.is_active ? `
          <button type="button" class="btn btn-secondary btn-sm" onclick="restartPodmanUnit('${escapeHtml(c.unit_name)}')">
            <span>🔄</span> Redémarrer
          </button>
          <button type="button" class="btn btn-danger-outline btn-sm" onclick="stopPodmanUnit('${escapeHtml(c.unit_name)}')">
            <span>⏹️</span> Arrêter
          </button>
        ` : `
          <button type="button" class="btn btn-success-outline btn-sm" onclick="startPodmanUnit('${escapeHtml(c.unit_name)}')">
            <span>▶️</span> Démarrer
          </button>
        `}
      </div>
    `;

    grid.appendChild(card);
  });
}

function populatePodmanLogSelect() {
  const select = document.getElementById("podman-logs-container-select");
  if (!select || !currentPodmanOverview) return;

  const prevValue = select.value;
  select.innerHTML = "";

  currentPodmanOverview.containers.forEach(c => {
    const opt = document.createElement("option");
    opt.value = c.unit_name;
    opt.textContent = `${c.name} (${c.is_active ? 'Actif' : 'Arrêté'})`;
    select.appendChild(opt);
  });

  if (prevValue && currentPodmanOverview.containers.some(c => c.unit_name === prevValue)) {
    select.value = prevValue;
  } else if (currentPodmanOverview.containers.length > 0) {
    select.value = currentPodmanOverview.containers[0].unit_name;
    currentPodmanLogUnit = select.value;
    fetchPodmanLogs();
  }
}

function onPodmanLogContainerChange() {
  const select = document.getElementById("podman-logs-container-select");
  if (select) {
    currentPodmanLogUnit = select.value;
    fetchPodmanLogs();
  }
}

function viewContainerLogs(unitName) {
  const select = document.getElementById("podman-logs-container-select");
  if (select) {
    select.value = unitName;
    currentPodmanLogUnit = unitName;
    fetchPodmanLogs();
  }

  const logsSection = document.getElementById("podman-logs-section");
  if (logsSection) {
    logsSection.scrollIntoView({ behavior: "smooth" });
  }
}

async function fetchPodmanLogs() {
  const select = document.getElementById("podman-logs-container-select");
  const linesSelect = document.getElementById("podman-logs-lines-select");
  const output = document.getElementById("podman-terminal-output");

  const unitName = select ? select.value : currentPodmanLogUnit;
  const lines = linesSelect ? parseInt(linesSelect.value, 10) : 100;

  if (!unitName || !output) return;

  output.textContent = `Chargement des logs pour ${unitName}...`;

  try {
    const logs = await invoke("get_podman_logs", { unitName, lines });
    if (logs) {
      output.textContent = logs;
      // Défilement automatique vers le bas
      const container = output.parentElement;
      if (container) {
        container.scrollTop = container.scrollHeight;
      }
    } else {
      output.textContent = `Aucun journal disponible pour ${unitName}.`;
    }
  } catch (err) {
    output.textContent = `Erreur lors de la lecture des logs : ${err}`;
  }
}

function copyPodmanLogs() {
  const output = document.getElementById("podman-terminal-output");
  const copyIcon = document.getElementById("podman-copy-icon");
  if (!output) return;

  navigator.clipboard.writeText(output.textContent).then(() => {
    showToast("Logs copiés dans le presse-papier !", "info");
    if (copyIcon) {
      copyIcon.textContent = "✅";
      setTimeout(() => { copyIcon.textContent = "📋"; }, 1500);
    }
  }).catch(err => {
    showToast("Impossible de copier les logs : " + err, "error");
  });
}

async function restartPodmanUnit(unitName) {
  showToast(`Redémarrage de ${unitName}...`, "info");
  try {
    const msg = await invoke("restart_podman_container", { unitName });
    showToast(msg || "Conteneur redémarré avec succès !", "success");
    await loadPodmanOverview(false);
    await fetchPodmanLogs();
  } catch (err) {
    showToast("Erreur lors du redémarrage : " + err, "error");
  }
}

async function stopPodmanUnit(unitName) {
  showToast(`Arrêt de ${unitName}...`, "info");
  try {
    const msg = await invoke("stop_podman_container", { unitName });
    showToast(msg || "Conteneur arrêté avec succès !", "success");
    await loadPodmanOverview(false);
    await fetchPodmanLogs();
  } catch (err) {
    showToast("Erreur lors de l'arrêt : " + err, "error");
  }
}

async function startPodmanUnit(unitName) {
  showToast(`Démarrage de ${unitName}...`, "info");
  try {
    const msg = await invoke("start_podman_container", { unitName });
    showToast(msg || "Conteneur démarré avec succès !", "success");
    await loadPodmanOverview(false);
    await fetchPodmanLogs();
  } catch (err) {
    showToast("Erreur lors du démarrage : " + err, "error");
  }
}

function openExternalBrowserUrl(url) {
  invoke("open_external_url", { url }).catch(err => {
    console.error("Erreur ouverture URL :", err);
    window.open(url, "_blank");
  });
}


// =========================================================================
// ❄️ NIX & SHELL : SOUS-NAVIGATION & GESTION DU SHELL UTILISATEUR
// =========================================================================

let currentConfiguredShell = "fish";
let selectedShellChoice = "fish";

function switchNixShellSubtab(subtabId) {
  const subnavBtns = document.querySelectorAll(".nix-shell-subnav-btn");
  subnavBtns.forEach(btn => {
    if (btn.getAttribute("data-subtab") === subtabId) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
  });

  const panes = document.querySelectorAll(".nix-shell-subtab-pane");
  panes.forEach(pane => pane.classList.remove("active"));

  const targetPane = document.getElementById(subtabId);
  if (targetPane) {
    targetPane.classList.add("active");
  }

  if (subtabId === "nix-subtab-images") {
    loadGenerations();
  } else if (subtabId === "nix-subtab-shell") {
    loadUserShell();
  } else if (subtabId === "nix-subtab-systemd") {
    loadSystemdServices(false);
  } else if (subtabId === "nix-subtab-fastfetch") {
    initFastfetchView();
    loadFastfetchState(false);
    setTimeout(() => {
      if (typeof fastfetchFitAddon !== "undefined" && fastfetchFitAddon) {
        try { fastfetchFitAddon.fit(); } catch (_) {}
      }
    }, 50);
  }
}

async function loadUserShell() {
  try {
    const shell = await invoke("get_user_shell");
    currentConfiguredShell = (shell || "fish").trim().toLowerCase();
    selectedShellChoice = currentConfiguredShell;
    updateShellUI(currentConfiguredShell);
  } catch (err) {
    console.error("Erreur lors de la récupération du shell utilisateur:", err);
  }
}

function selectShell(shellName) {
  selectedShellChoice = shellName.toLowerCase();
  updateShellUI(selectedShellChoice);
}

function updateShellUI(activeShell) {
  const shells = ["fish", "zsh", "bash"];
  shells.forEach(s => {
    const card = document.getElementById(`shell-card-${s}`);
    const badge = document.getElementById(`shell-badge-${s}`);
    const btn = document.getElementById(`btn-select-${s}`);

    const isSelected = s === activeShell;
    const isSaved = s === currentConfiguredShell;

    if (card) {
      if (isSelected) {
        card.classList.add("active");
      } else {
        card.classList.remove("active");
      }
    }

    if (badge) {
      if (isSaved && isSelected) {
        badge.textContent = "Actif (vars.nix)";
        badge.style.color = "var(--green)";
        badge.style.borderColor = "rgba(166, 227, 161, 0.4)";
      } else if (isSelected) {
        badge.textContent = "Sélectionné";
        badge.style.color = "var(--peach)";
        badge.style.borderColor = "rgba(250, 179, 135, 0.4)";
      } else {
        badge.textContent = "Disponible";
        badge.style.color = "var(--subtext0)";
        badge.style.borderColor = "transparent";
      }
    }

    if (btn) {
      if (isSaved && isSelected) {
        btn.textContent = "✓ Shell Actif";
        btn.classList.add("btn-primary");
        btn.classList.remove("btn-outline");
      } else if (isSelected) {
        btn.textContent = "Enregistrer ce shell";
        btn.classList.add("btn-primary");
        btn.classList.remove("btn-outline");
      } else {
        btn.textContent = `Choisir ${s.toUpperCase()}`;
        btn.classList.remove("btn-primary");
        btn.classList.add("btn-outline");
      }
    }
  });

  const display = document.getElementById("current-shell-display");
  if (display) {
    const capitalized = activeShell.charAt(0).toUpperCase() + activeShell.slice(1);
    const isUnsaved = activeShell !== currentConfiguredShell;
    display.innerHTML = `${capitalized} ${isUnsaved ? "<span style=\"color: var(--peach); font-size: 0.85rem;\">(non sauvegardé)</span>" : "<span style=\"color: var(--green); font-size: 0.85rem;\">(configuré)</span>"}`;
  }

  const sidebarBadge = document.getElementById("sidebar-active-shell-badge");
  if (sidebarBadge) {
    sidebarBadge.textContent = currentConfiguredShell.toUpperCase();
  }
}

async function applySelectedShell() {
  try {
    const saved = await invoke("set_user_shell", { shell: selectedShellChoice });
    currentConfiguredShell = saved;
    updateShellUI(currentConfiguredShell);
    showToast(`✓ Shell par défaut défini sur "${saved}" dans /etc/nixos/vars.nix !`, "success");
  } catch (err) {
    showToast(`Erreur lors de la configuration du shell : ${err}`, "error");
  }
}

async function applyShellAndRebuild() {
  try {
    const saved = await invoke("set_user_shell", { shell: selectedShellChoice });
    currentConfiguredShell = saved;
    updateShellUI(currentConfiguredShell);
    showToast(`✓ Shell configuré sur "${saved}". Démarrage du switch NixOS...`, "success");
    runAction("switch");
  } catch (err) {
    showToast(`Erreur : ${err}`, "error");
  }
}

window.switchNixShellSubtab = switchNixShellSubtab;
window.selectShell = selectShell;
window.applySelectedShell = applySelectedShell;
window.applyShellAndRebuild = applyShellAndRebuild;
window.loadUserShell = loadUserShell;


// =========================================================================
// ⚙️ SYSTEMD SERVICES MANAGER : GESTIONNAIRE DES SERVICES EN DIRECT
// =========================================================================

let currentSystemdOverview = null;
let currentSystemdScope = "system";
let currentSystemdFilter = "all";
let currentSystemdLogUnit = null;
let currentSystemdLogIsUser = false;
let systemdSearchDebounce = null;

async function loadSystemdServices(forceRefresh = false) {
  const loading = document.getElementById("systemd-loading");
  const empty = document.getElementById("systemd-empty");
  const table = document.getElementById("systemd-table");

  if (!currentSystemdOverview || forceRefresh) {
    if (loading) loading.classList.remove("hidden");
    if (empty) empty.classList.add("hidden");
    if (table) table.classList.add("hidden");
  }

  try {
    const overview = await invoke("get_systemd_services", { scope: currentSystemdScope });
    currentSystemdOverview = overview;

    const statTotal = document.getElementById("systemd-stat-total");
    const statActive = document.getElementById("systemd-stat-active");
    const statInactive = document.getElementById("systemd-stat-inactive");
    const statFailed = document.getElementById("systemd-stat-failed");

    if (statTotal) statTotal.textContent = overview.total;
    if (statActive) statActive.textContent = overview.active_count;
    if (statInactive) statInactive.textContent = overview.inactive_count;
    if (statFailed) statFailed.textContent = overview.failed_count;

    const pillAll = document.getElementById("pill-count-all");
    const pillActive = document.getElementById("pill-count-active");
    const pillInactive = document.getElementById("pill-count-inactive");
    const pillFailed = document.getElementById("pill-count-failed");

    if (pillAll) pillAll.textContent = overview.total;
    if (pillActive) pillActive.textContent = overview.active_count;
    if (pillInactive) pillInactive.textContent = overview.inactive_count;
    if (pillFailed) pillFailed.textContent = overview.failed_count;

    if (forceRefresh) {
      showToast("Liste des services systemd actualisée", "info");
    }

    renderSystemdServices();
  } catch (err) {
    console.error("Erreur chargement systemd:", err);
    if (loading) loading.classList.add("hidden");
    showToast("Erreur lors de la récupération des services : " + err, "error");
  }
}

function switchSystemdScope(scope) {
  if (scope === currentSystemdScope) return;
  currentSystemdScope = scope;

  const btnSys = document.getElementById("btn-scope-system");
  const btnUsr = document.getElementById("btn-scope-user");

  if (btnSys) btnSys.classList.toggle("active", scope === "system");
  if (btnUsr) btnUsr.classList.toggle("active", scope === "user");

  loadSystemdServices(true);
}

function setSystemdStatusFilter(status) {
  currentSystemdFilter = status;

  const pills = document.querySelectorAll(".systemd-filter-pills .filter-pill");
  pills.forEach(p => {
    if (p.getAttribute("data-status") === status) {
      p.classList.add("active");
    } else {
      p.classList.remove("active");
    }
  });

  renderSystemdServices();
}

function filterSystemdServices() {
  const searchInput = document.getElementById("systemd-search-input");
  const clearBtn = document.getElementById("systemd-search-clear");
  if (clearBtn && searchInput) {
    if (searchInput.value.trim().length > 0) {
      clearBtn.classList.remove("hidden");
    } else {
      clearBtn.classList.add("hidden");
    }
  }

  if (systemdSearchDebounce) clearTimeout(systemdSearchDebounce);
  systemdSearchDebounce = setTimeout(() => {
    renderSystemdServices();
  }, 150);
}

function clearSystemdSearch() {
  const searchInput = document.getElementById("systemd-search-input");
  const clearBtn = document.getElementById("systemd-search-clear");
  if (searchInput) searchInput.value = "";
  if (clearBtn) clearBtn.classList.add("hidden");
  renderSystemdServices();
}

function renderSystemdServices() {
  const loading = document.getElementById("systemd-loading");
  const empty = document.getElementById("systemd-empty");
  const table = document.getElementById("systemd-table");
  const tbody = document.getElementById("systemd-tbody");

  if (loading) loading.classList.add("hidden");

  if (!currentSystemdOverview || !currentSystemdOverview.services) {
    if (empty) empty.classList.remove("hidden");
    if (table) table.classList.add("hidden");
    return;
  }

  const searchInput = document.getElementById("systemd-search-input");
  const query = searchInput ? searchInput.value.trim().toLowerCase() : "";

  const filtered = currentSystemdOverview.services.filter(svc => {
    if (query) {
      const matchName = svc.name.toLowerCase().includes(query);
      const matchUnit = svc.unit.toLowerCase().includes(query);
      const matchDesc = svc.description.toLowerCase().includes(query);
      if (!matchName && !matchUnit && !matchDesc) return false;
    }

    const isFailed = svc.active === "failed" || svc.sub === "failed";
    const isActive = svc.active === "active";

    if (currentSystemdFilter === "active") return isActive;
    if (currentSystemdFilter === "inactive") return !isActive && !isFailed;
    if (currentSystemdFilter === "failed") return isFailed;

    return true;
  });

  if (filtered.length === 0) {
    if (empty) empty.classList.remove("hidden");
    if (table) table.classList.add("hidden");
    return;
  }

  if (empty) empty.classList.add("hidden");
  if (table) table.classList.remove("hidden");

  if (!tbody) return;

  const isUser = currentSystemdScope === "user";

  tbody.innerHTML = filtered.map(svc => {
    const isFailed = svc.active === "failed" || svc.sub === "failed";
    const isActive = svc.active === "active";

    let badgeClass = "badge-inactive";
    let badgeLabel = "● Arrêté";
    let icon = "⚪";

    if (isFailed) {
      badgeClass = "badge-failed";
      badgeLabel = "✕ En échec";
      icon = "🔴";
    } else if (isActive) {
      badgeClass = "badge-active";
      badgeLabel = "● Actif";
      icon = "🟢";
    }

    let actionButtons = "";
    if (isActive) {
      actionButtons += `
        <button type="button" class="btn-action-sm btn-stop" onclick="controlSystemdUnit('${escapeHtml(svc.unit)}', 'stop', ${isUser})" title="Arrêter le service">
          <span>⏹️</span> <span class="btn-label">Arrêter</span>
        </button>
        <button type="button" class="btn-action-sm btn-restart" onclick="controlSystemdUnit('${escapeHtml(svc.unit)}', 'restart', ${isUser})" title="Redémarrer le service">
          <span>🔄</span> <span class="btn-label">Redémarrer</span>
        </button>
      `;
    } else if (isFailed) {
      actionButtons += `
        <button type="button" class="btn-action-sm btn-start" onclick="controlSystemdUnit('${escapeHtml(svc.unit)}', 'start', ${isUser})" title="Démarrer le service">
          <span>▶️</span> <span class="btn-label">Démarrer</span>
        </button>
        <button type="button" class="btn-action-sm btn-restart" onclick="controlSystemdUnit('${escapeHtml(svc.unit)}', 'restart', ${isUser})" title="Relancer le service">
          <span>🔄</span> <span class="btn-label">Relancer</span>
        </button>
      `;
    } else {
      actionButtons += `
        <button type="button" class="btn-action-sm btn-start" onclick="controlSystemdUnit('${escapeHtml(svc.unit)}', 'start', ${isUser})" title="Démarrer le service">
          <span>▶️</span> <span class="btn-label">Démarrer</span>
        </button>
      `;
    }

    actionButtons += `
      <button type="button" class="btn-action-sm btn-logs" onclick="viewSystemdLogs('${escapeHtml(svc.unit)}', ${isUser})" title="Afficher les journaux (journalctl)">
        <span>📋</span> <span class="btn-label">Logs</span>
      </button>
    `;

    return `
      <tr>
        <td class="col-service-main">
          <div class="systemd-unit-title">
            <span class="unit-icon">${icon}</span>
            <span class="unit-name-text">${escapeHtml(svc.name)}</span>
            <span class="unit-code-badge">${escapeHtml(svc.unit)}</span>
          </div>
          <div class="systemd-unit-desc" title="${escapeHtml(svc.description || svc.unit)}">
            ${escapeHtml(svc.description || svc.unit)}
          </div>
        </td>
        <td class="col-status">
          <span class="badge-systemd ${badgeClass}">${badgeLabel}</span>
        </td>
        <td class="col-substate">
          <code class="systemd-sub-state">${escapeHtml(svc.sub || "inconnu")}</code>
        </td>
        <td class="col-actions">
          <div class="systemd-actions-cell">
            ${actionButtons}
          </div>
        </td>
      </tr>
    `;
  }).join("");
}

async function controlSystemdUnit(unitName, action, isUser) {
  const actionLabels = {
    start: "Démarrage",
    stop: "Arrêt",
    restart: "Redémarrage"
  };
  const label = actionLabels[action] || action;

  showToast(`${label} de ${unitName}...`, "info");

  try {
    const msg = await invoke("control_systemd_service", { unitName, action, isUser });
    showToast(msg || `Opération ${action} réussie sur ${unitName}`, "success");
    await loadSystemdServices(false);
  } catch (err) {
    showToast(`Erreur sur ${unitName} : ${err}`, "error");
  }
}

function viewSystemdLogs(unitName, isUser) {
  currentSystemdLogUnit = unitName;
  currentSystemdLogIsUser = isUser;

  const modal = document.getElementById("systemd-logs-modal");
  const title = document.getElementById("systemd-modal-title");
  const subtitle = document.getElementById("systemd-modal-subtitle");

  if (title) title.textContent = `Journaux : ${unitName}`;
  if (subtitle) subtitle.textContent = `Journalctl ${isUser ? "--user" : "--system"} -u ${unitName}`;
  if (modal) modal.classList.remove("hidden");

  fetchSystemdLogs();
}

async function fetchSystemdLogs() {
  if (!currentSystemdLogUnit) return;

  const output = document.getElementById("systemd-modal-output");
  const linesSelect = document.getElementById("systemd-log-lines");
  const lines = linesSelect ? parseInt(linesSelect.value, 10) : 100;

  if (output) output.textContent = `Chargement des logs pour ${currentSystemdLogUnit}...`;

  try {
    const logs = await invoke("get_systemd_service_logs", {
      unitName: currentSystemdLogUnit,
      lines,
      isUser: currentSystemdLogIsUser
    });

    if (output) {
      output.textContent = logs || `Aucun journal disponible pour ${currentSystemdLogUnit}.`;
      const container = output.parentElement;
      if (container) {
        container.scrollTop = container.scrollHeight;
      }
    }
  } catch (err) {
    if (output) output.textContent = `Erreur lors de la récupération des journaux : ${err}`;
  }
}

function refreshSystemdLogs() {
  fetchSystemdLogs();
}

function copySystemdLogs() {
  const output = document.getElementById("systemd-modal-output");
  const icon = document.getElementById("systemd-copy-icon");
  if (!output) return;

  navigator.clipboard.writeText(output.textContent).then(() => {
    if (icon) icon.textContent = "✅";
    showToast("Journaux copiés dans le presse-papier !", "success");
    setTimeout(() => {
      if (icon) icon.textContent = "📋";
    }, 2000);
  }).catch(err => {
    showToast("Impossible de copier : " + err, "error");
  });
}

function closeSystemdLogsModal() {
  const modal = document.getElementById("systemd-logs-modal");
  if (modal) modal.classList.add("hidden");
  currentSystemdLogUnit = null;
}

window.loadSystemdServices = loadSystemdServices;
window.switchSystemdScope = switchSystemdScope;
window.setSystemdStatusFilter = setSystemdStatusFilter;
window.filterSystemdServices = filterSystemdServices;
window.clearSystemdSearch = clearSystemdSearch;
window.controlSystemdUnit = controlSystemdUnit;
window.viewSystemdLogs = viewSystemdLogs;
window.fetchSystemdLogs = fetchSystemdLogs;
window.refreshSystemdLogs = refreshSystemdLogs;
window.copySystemdLogs = copySystemdLogs;
window.closeSystemdLogsModal = closeSystemdLogsModal;


// =========================================================================
// 📁 GESTIONNAIRE SFTP & ACCÈS SSH PAR MOT DE PASSE
// =========================================================================

let currentSftpOverview = null;

async function loadSftpOverview(showFeedback = false) {
  try {
    const overview = await invoke("get_sftp_overview");
    if (!overview) return;

    currentSftpOverview = overview;

    // 1. Mise à jour de l'interrupteur OpenSSH
    const toggle = document.getElementById("sftp-toggle-enable");
    if (toggle) {
      toggle.checked = overview.status.openssh_configured;
    }

    // 2. Mise à jour de la pastille d'état du service sshd
    const servicePill = document.getElementById("sftp-service-pill");
    const serviceStatusText = document.getElementById("sftp-service-status-text");
    const navBadge = document.getElementById("sftp-nav-badge");
    const btnStopStart = document.getElementById("btn-sftp-stop-start");

    if (overview.status.sshd_active) {
      if (servicePill) {
        servicePill.className = "sftp-status-pill active";
        servicePill.innerHTML = `<span class="sftp-dot online"></span><span id="sftp-service-status-text">Service sshd Actif</span>`;
      }
      if (navBadge) {
        navBadge.className = "net-subnav-pill sftp-pill-ok";
        navBadge.textContent = "Actif";
      }
      if (btnStopStart) {
        btnStopStart.innerHTML = "<span>⏹️</span> Arrêter sshd";
        btnStopStart.title = "Arrêter le service OpenSSH";
      }
    } else {
      if (servicePill) {
        servicePill.className = "sftp-status-pill inactive";
        servicePill.innerHTML = `<span class="sftp-dot offline"></span><span id="sftp-service-status-text">Service sshd Arrêté</span>`;
      }
      if (navBadge) {
        navBadge.className = "net-subnav-pill sftp-pill-warn";
        navBadge.textContent = "Arrêté";
      }
      if (btnStopStart) {
        btnStopStart.innerHTML = "<span>▶️</span> Démarrer sshd";
        btnStopStart.title = "Démarrer le service OpenSSH";
      }
    }

    // 3. Port 22 & Pare-feu
    const statusPort22 = document.getElementById("sftp-status-port22");
    const btnFixFirewall = document.getElementById("btn-fix-firewall-22");

    if (overview.status.port_22_firewall_open && overview.status.port_22_accessible) {
      if (statusPort22) {
        statusPort22.className = "text-success";
        statusPort22.textContent = "Ouvert & Accessible (TCP)";
      }
      if (btnFixFirewall) btnFixFirewall.classList.add("d-none");
    } else if (overview.status.port_22_firewall_open) {
      if (statusPort22) {
        statusPort22.className = "text-warning";
        statusPort22.textContent = "Ouvert (sshd non démarré)";
      }
      if (btnFixFirewall) btnFixFirewall.classList.add("d-none");
    } else {
      if (statusPort22) {
        statusPort22.className = "text-danger";
        statusPort22.textContent = "Fermé / Non détecté";
      }
      if (btnFixFirewall) btnFixFirewall.classList.remove("d-none");
    }

    // 4. URL de connexion
    const uriEl = document.getElementById("sftp-connection-uri");
    if (uriEl) {
      const defaultUser = overview.users && overview.users.length > 0 ? overview.users[0].username : "utilisateur";
      uriEl.textContent = `sftp://${defaultUser}@${overview.status.local_ip}:22`;
    }

    // 5. Rendu des dossiers partagés
    renderSftpShares(overview.shares, overview.users);

    // 6. Rendu des utilisateurs sFTP
    renderSftpUsers(overview.users, overview.shares);

    if (showFeedback) {
      showToast("État du partage sFTP actualisé !", "info");
    }
  } catch (err) {
    console.error("Erreur chargement sFTP:", err);
    if (showFeedback) {
      showToast("Impossible d'actualiser l'espace sFTP: " + err, "error");
    }
  }
}

function renderSftpShares(shares, users) {
  const container = document.getElementById("sftp-shares-grid");
  if (!container) return;

  if (!shares || shares.length === 0) {
    container.innerHTML = `
      <div class="sftp-empty-placeholder">
        <p>📁 Aucun dossier de partage sFTP n'a encore été configuré.</p>
        <button type="button" class="btn btn-primary btn-sm" onclick="openAddShareModal()">
          <span>➕</span> Créer le premier partage
        </button>
      </div>
    `;
    return;
  }

  let html = "";
  for (const s of shares) {
    const assignedUsers = (users || []).filter(u => u.share_id === s.id);
    let usersHtml = "";
    if (assignedUsers.length > 0) {
      usersHtml = assignedUsers.map(u => `<span class="sftp-user-tag">👤 ${escapeHtml(u.username)}</span>`).join(" ");
    } else {
      usersHtml = `<span class="text-muted" style="font-size:0.75rem;">Aucun utilisateur rattaché</span>`;
    }

    html += `
      <div class="sftp-share-card">
        <div class="sftp-share-top">
          <div class="sftp-share-meta">
            <div class="sftp-share-folder-icon">📂</div>
            <div>
              <h4 class="sftp-share-name">${escapeHtml(s.name)}</h4>
              <p class="sftp-share-desc">${escapeHtml(s.description || "Partage de dossier sFTP")}</p>
            </div>
          </div>
        </div>

        <div class="sftp-share-path-box">
          <span>${escapeHtml(s.path)}</span>
          <button type="button" class="btn btn-xs btn-secondary" onclick="copyToClipboard('${escapeHtml(s.path)}', this)" title="Copier le chemin">📋</button>
        </div>

        <div class="sftp-share-stats-row">
          <div class="sftp-share-stats-item">
            <span>📊 Espace :</span>
            <strong>${escapeHtml(s.size_human || "0 B")}</strong>
          </div>
          <div class="sftp-share-stats-item">
            <span>📄 Fichiers :</span>
            <strong>${s.item_count || 0}</strong>
          </div>
        </div>

        <div class="sftp-share-users-row">
          <span style="font-size:0.76rem; color:#9399b2; margin-right:4px;">Accès :</span>
          ${usersHtml}
        </div>

        <div class="sftp-share-bottom">
          <button type="button" class="btn btn-xs btn-secondary" onclick="openFolderInDolphinUI('${escapeHtml(s.path)}')" title="Ouvrir dans le gestionnaire de fichiers">
            <span>📂</span> Ouvrir dans Dolphin
          </button>
          <button type="button" class="btn btn-xs btn-secondary" onclick="editSftpShare('${escapeHtml(s.id)}')" title="Modifier les paramètres">
            <span>✏️</span> Modifier
          </button>
          <button type="button" class="btn btn-xs btn-danger" onclick="deleteSftpShareUI('${escapeHtml(s.id)}')" title="Supprimer ce partage">
            <span>🗑️</span>
          </button>
        </div>
      </div>
    `;
  }

  container.innerHTML = html;
}

function renderSftpUsers(users, shares) {
  const container = document.getElementById("sftp-users-table-wrap");
  if (!container) return;

  if (!users || users.length === 0) {
    container.innerHTML = `
      <div class="sftp-empty-placeholder">
        <p>👤 Aucun utilisateur sFTP configuré pour le moment.</p>
        <button type="button" class="btn btn-primary btn-sm" onclick="openAddUserModal()">
          <span>➕</span> Créer un accès utilisateur
        </button>
      </div>
    `;
    return;
  }

  const sharesMap = {};
  if (shares) {
    for (const s of shares) {
      sharesMap[s.id] = s.name;
    }
  }

  let rows = "";
  for (const u of users) {
    const shareName = sharesMap[u.share_id] || u.share_id || "Dossier par défaut";
    
    let permBadge = "";
    if (u.permission === "ro") {
      permBadge = `<span class="sftp-perm-badge sftp-perm-ro">👁️ Lecture Seule (-R)</span>`;
    } else if (u.permission === "rw") {
      permBadge = `<span class="sftp-perm-badge sftp-perm-rw">✍️ Écriture sans Suppression</span>`;
    } else {
      permBadge = `<span class="sftp-perm-badge sftp-perm-full">⚡ Contrôle Total</span>`;
    }

    const statusBadge = u.enabled 
      ? `<span class="badge badge-success" style="font-size:0.75rem;">Actif</span>`
      : `<span class="badge badge-warning" style="font-size:0.75rem;">Suspendu</span>`;

    const initial = u.username.charAt(0).toUpperCase();

    rows += `
      <tr>
        <td>
          <div class="sftp-user-profile">
            <div class="sftp-user-avatar">${initial}</div>
            <div class="sftp-user-info-wrap">
              <strong>${escapeHtml(u.username)}</strong>
              <small>Créé le ${escapeHtml(u.created_at || "Récemment")}</small>
            </div>
          </div>
        </td>
        <td>
          <span style="font-weight:600; color:#cdd6f4;">📂 ${escapeHtml(shareName)}</span>
        </td>
        <td>
          ${permBadge}
        </td>
        <td>
          ${statusBadge}
        </td>
        <td>
          <div style="display:flex; gap:6px; justify-content:flex-end;">
            <button type="button" class="btn btn-xs btn-secondary" onclick="editSftpUser('${escapeHtml(u.username)}')" title="Modifier les permissions ou le mot de passe">
              <span>✏️</span> Modifier
            </button>
            <button type="button" class="btn btn-xs btn-danger" onclick="deleteSftpUserUI('${escapeHtml(u.username)}')" title="Supprimer cet utilisateur">
              <span>🗑️</span>
            </button>
          </div>
        </td>
      </tr>
    `;
  }

  container.innerHTML = `
    <table class="sftp-users-table">
      <thead>
        <tr>
          <th>Utilisateur</th>
          <th>Dossier Partagé</th>
          <th>Permissions d'Accès</th>
          <th>Statut</th>
          <th style="text-align:right;">Actions</th>
        </tr>
      </thead>
      <tbody>
        ${rows}
      </tbody>
    </table>
  `;
}

// -------------------------------------------------------------------------
// ACTIONS SERVICES & PARE-FEU
// -------------------------------------------------------------------------

async function toggleSftpServiceFromUI(enable) {
  try {
    showToast(enable ? "Activation du service OpenSSH..." : "Désactivation du service OpenSSH...", "info");
    const res = await invoke("toggle_sftp_service", { enable });
    showToast(res, "success");
    await loadSftpOverview(false);
  } catch (err) {
    showToast("Erreur bascule OpenSSH : " + err, "error");
    await loadSftpOverview(false);
  }
}

async function controlSftpServiceUI(action) {
  try {
    showToast(`Exécution de ${action} sur sshd...`, "info");
    const res = await invoke("control_sftp_service", { action });
    showToast(res, "success");
    await loadSftpOverview(false);
  } catch (err) {
    showToast("Erreur action sshd : " + err, "error");
  }
}

async function toggleSftpRunningState() {
  if (!currentSftpOverview) return;
  const isRunning = currentSftpOverview.status.sshd_active;
  await controlSftpServiceUI(isRunning ? "stop" : "start");
}

async function openSftpFirewallUI() {
  try {
    showToast("Autorisation du port 22 dans le pare-feu...", "info");
    const res = await invoke("open_sftp_firewall_port");
    showToast(res, "success");
    await loadSftpOverview(false);
  } catch (err) {
    showToast("Erreur pare-feu : " + err, "error");
  }
}

function copySftpConnectionUri() {
  const uriEl = document.getElementById("sftp-connection-uri");
  const icon = document.getElementById("sftp-copy-icon");
  if (!uriEl) return;

  navigator.clipboard.writeText(uriEl.textContent).then(() => {
    if (icon) icon.textContent = "✅";
    showToast("URL sFTP copiée dans le presse-papier !", "success");
    setTimeout(() => {
      if (icon) icon.textContent = "📋";
    }, 2000);
  }).catch(err => {
    showToast("Erreur copie : " + err, "error");
  });
}

function openFolderInDolphinUI(path) {
  invoke("open_folder_in_dolphin", { path }).then(() => {
    showToast("Dossier ouvert dans Dolphin", "info");
  }).catch(err => {
    showToast("Erreur ouverture dossier : " + err, "error");
  });
}

// -------------------------------------------------------------------------
// MODALES & GESTION DES PARTAGES
// -------------------------------------------------------------------------

function closeSftpModals() {
  const m1 = document.getElementById("modal-sftp-share");
  const m2 = document.getElementById("modal-sftp-user");
  if (m1) m1.classList.add("hidden");
  if (m2) m2.classList.add("hidden");
}

function openAddShareModal() {
  const modal = document.getElementById("modal-sftp-share");
  const title = document.getElementById("modal-sftp-share-title");
  const idInput = document.getElementById("sftp-share-edit-id");
  const nameInput = document.getElementById("sftp-share-name");
  const pathInput = document.getElementById("sftp-share-path");
  const descInput = document.getElementById("sftp-share-desc");

  if (title) title.textContent = "📁 Nouveau Dossier Partagé sFTP";
  if (idInput) idInput.value = "";
  if (nameInput) nameInput.value = "";
  if (pathInput) pathInput.value = "/home/chomiam/Partages/nouveau_partage";
  if (descInput) descInput.value = "";

  if (modal) modal.classList.remove("hidden");
}

function editSftpShare(id) {
  if (!currentSftpOverview) return;
  const share = currentSftpOverview.shares.find(s => s.id === id);
  if (!share) return;

  const modal = document.getElementById("modal-sftp-share");
  const title = document.getElementById("modal-sftp-share-title");
  const idInput = document.getElementById("sftp-share-edit-id");
  const nameInput = document.getElementById("sftp-share-name");
  const pathInput = document.getElementById("sftp-share-path");
  const descInput = document.getElementById("sftp-share-desc");

  if (title) title.textContent = "✏️ Modifier le Dossier Partagé";
  if (idInput) idInput.value = share.id;
  if (nameInput) nameInput.value = share.name;
  if (pathInput) pathInput.value = share.path;
  if (descInput) descInput.value = share.description || "";

  if (modal) modal.classList.remove("hidden");
}

async function submitSaveSftpShare() {
  const idInput = document.getElementById("sftp-share-edit-id");
  const nameInput = document.getElementById("sftp-share-name");
  const pathInput = document.getElementById("sftp-share-path");
  const descInput = document.getElementById("sftp-share-desc");

  const name = nameInput ? nameInput.value.trim() : "";
  const path = pathInput ? pathInput.value.trim() : "";
  const desc = descInput ? descInput.value.trim() : "";
  const id = idInput && idInput.value ? idInput.value : ("share_" + Date.now());

  if (!name) {
    showToast("Veuillez saisir un nom pour le partage.", "warning");
    return;
  }
  if (!path) {
    showToast("Veuillez spécifier le chemin du dossier sur la machine.", "warning");
    return;
  }

  const shareObj = {
    id: id,
    name: name,
    path: path,
    description: desc,
    created_at: new Date().toISOString().split("T")[0],
    authorized_users: [],
    item_count: 0,
    size_human: "0 B"
  };

  try {
    const res = await invoke("save_sftp_share", { share: shareObj });
    showToast(res, "success");
    closeSftpModals();
    await loadSftpOverview(false);
  } catch (err) {
    showToast("Erreur enregistrement partage : " + err, "error");
  }
}

async function deleteSftpShareUI(id) {
  if (!confirm("Voulez-vous vraiment supprimer ce dossier partagé sFTP ? Les fichiers physiques ne seront pas supprimés.")) {
    return;
  }

  try {
    const res = await invoke("delete_sftp_share", { shareId: id });
    showToast(res, "success");
    await loadSftpOverview(false);
  } catch (err) {
    showToast("Erreur suppression partage : " + err, "error");
  }
}

// -------------------------------------------------------------------------
// MODALES & GESTION DES UTILISATEURS
// -------------------------------------------------------------------------

function openAddUserModal() {
  const modal = document.getElementById("modal-sftp-user");
  const title = document.getElementById("modal-sftp-user-title");
  const nameInput = document.getElementById("sftp-user-name");
  const pwdInput = document.getElementById("sftp-user-password");
  const pwdHint = document.getElementById("sftp-user-password-hint");
  const shareSelect = document.getElementById("sftp-user-share");
  const enabledCheck = document.getElementById("sftp-user-enabled");

  if (title) title.textContent = "👤 Nouvel Utilisateur sFTP";
  if (nameInput) {
    nameInput.value = "";
    nameInput.disabled = false;
  }
  if (pwdInput) pwdInput.value = "";
  if (pwdHint) pwdHint.textContent = "La connexion sFTP se fera avec ce mot de passe sans nécessiter de clé SSH.";
  if (enabledCheck) enabledCheck.checked = true;

  // Options de dossiers partagés
  if (shareSelect && currentSftpOverview && currentSftpOverview.shares) {
    shareSelect.innerHTML = currentSftpOverview.shares.map(s => 
      `<option value="${escapeHtml(s.id)}">${escapeHtml(s.name)} (${escapeHtml(s.path)})</option>`
    ).join("");
  }

  // Permission par défaut : Lecture seule
  const roRadio = document.querySelector('input[name="sftp-user-perm"][value="ro"]');
  if (roRadio) roRadio.checked = true;

  if (modal) modal.classList.remove("hidden");
}

function editSftpUser(username) {
  if (!currentSftpOverview) return;
  const user = currentSftpOverview.users.find(u => u.username === username);
  if (!user) return;

  const modal = document.getElementById("modal-sftp-user");
  const title = document.getElementById("modal-sftp-user-title");
  const nameInput = document.getElementById("sftp-user-name");
  const pwdInput = document.getElementById("sftp-user-password");
  const pwdHint = document.getElementById("sftp-user-password-hint");
  const shareSelect = document.getElementById("sftp-user-share");
  const enabledCheck = document.getElementById("sftp-user-enabled");

  if (title) title.textContent = `✏️ Modifier l'Utilisateur sFTP : ${username}`;
  if (nameInput) {
    nameInput.value = user.username;
    nameInput.disabled = true; // Pas de renommage d'identifiant système direct
  }
  if (pwdInput) pwdInput.value = "";
  if (pwdHint) pwdHint.textContent = "Laissez vide pour conserver le mot de passe actuel inchangé.";
  if (enabledCheck) enabledCheck.checked = user.enabled;

  if (shareSelect && currentSftpOverview && currentSftpOverview.shares) {
    shareSelect.innerHTML = currentSftpOverview.shares.map(s => 
      `<option value="${escapeHtml(s.id)}" ${s.id === user.share_id ? "selected" : ""}>${escapeHtml(s.name)} (${escapeHtml(s.path)})</option>`
    ).join("");
  }

  const permRadio = document.querySelector(`input[name="sftp-user-perm"][value="${user.permission}"]`);
  if (permRadio) permRadio.checked = true;

  if (modal) modal.classList.remove("hidden");
}

async function submitSaveSftpUser() {
  const nameInput = document.getElementById("sftp-user-name");
  const pwdInput = document.getElementById("sftp-user-password");
  const shareSelect = document.getElementById("sftp-user-share");
  const enabledCheck = document.getElementById("sftp-user-enabled");
  const permRadio = document.querySelector('input[name="sftp-user-perm"]:checked');

  const username = nameInput ? nameInput.value.trim().toLowerCase() : "";
  const password = pwdInput ? pwdInput.value : "";
  const shareId = shareSelect ? shareSelect.value : "";
  const enabled = enabledCheck ? enabledCheck.checked : true;
  const permission = permRadio ? permRadio.value : "ro";

  if (!username) {
    showToast("Veuillez indiquer un nom d'utilisateur.", "warning");
    return;
  }

  // Si c'est une création (input non désactivé), le mot de passe est obligatoire
  const isNew = nameInput && !nameInput.disabled;
  if (isNew && !password) {
    showToast("Un mot de passe est requis pour la création de l'accès sFTP.", "warning");
    return;
  }

  const userObj = {
    username: username,
    share_id: shareId,
    permission: permission,
    enabled: enabled,
    created_at: new Date().toISOString().split("T")[0]
  };

  try {
    showToast("Configuration du compte sFTP en cours...", "info");
    const res = await invoke("save_sftp_user", { 
      user: userObj, 
      password: password || null 
    });
    showToast(res, "success");
    closeSftpModals();
    await loadSftpOverview(false);
  } catch (err) {
    showToast("Erreur configuration utilisateur : " + err, "error");
  }
}

async function deleteSftpUserUI(username) {
  if (!confirm(`Voulez-vous vraiment supprimer définitivement l'utilisateur sFTP "${username}" ?`)) {
    return;
  }

  try {
    showToast(`Suppression de ${username}...`, "info");
    const res = await invoke("delete_sftp_user", { username });
    showToast(res, "success");
    await loadSftpOverview(false);
  } catch (err) {
    showToast("Erreur suppression utilisateur : " + err, "error");
  }
}

function toggleSftpPasswordVisibility() {
  const pwdInput = document.getElementById("sftp-user-password");
  if (!pwdInput) return;
  pwdInput.type = pwdInput.type === "password" ? "text" : "password";
}

function generateSftpRandomPassword() {
  const pwdInput = document.getElementById("sftp-user-password");
  if (!pwdInput) return;

  const chars = "abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#$%&*";
  let pwd = "";
  for (let i = 0; i < 12; i++) {
    pwd += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  pwdInput.value = pwd;
  pwdInput.type = "text";
  showToast("Mot de passe sécurisé généré !", "info");
}

window.loadSftpOverview = loadSftpOverview;
window.toggleSftpServiceFromUI = toggleSftpServiceFromUI;
window.controlSftpServiceUI = controlSftpServiceUI;
window.toggleSftpRunningState = toggleSftpRunningState;
window.openSftpFirewallUI = openSftpFirewallUI;
window.copySftpConnectionUri = copySftpConnectionUri;
window.openFolderInDolphinUI = openFolderInDolphinUI;
window.closeSftpModals = closeSftpModals;
window.openAddShareModal = openAddShareModal;
window.editSftpShare = editSftpShare;
window.submitSaveSftpShare = submitSaveSftpShare;
window.deleteSftpShareUI = deleteSftpShareUI;
window.openAddUserModal = openAddUserModal;
window.editSftpUser = editSftpUser;
window.submitSaveSftpUser = submitSaveSftpUser;
window.deleteSftpUserUI = deleteSftpUserUI;
window.toggleSftpPasswordVisibility = toggleSftpPasswordVisibility;
window.generateSftpRandomPassword = generateSftpRandomPassword;

function copyToClipboard(text, btn) {
  navigator.clipboard.writeText(text).then(() => {
    if (btn) {
      const old = btn.textContent;
      btn.textContent = "✅";
      setTimeout(() => { btn.textContent = old; }, 1500);
    }
    showToast("Copié dans le presse-papier !", "info");
  }).catch(e => {
    showToast("Erreur copie : " + e, "error");
  });
}
window.copyToClipboard = copyToClipboard;


// =========================================================================
// 🚀 GESTIONNAIRE DE PROFILS FASTFETCH (VALIDATION JSONC & TERMINAL INTÉGRÉ)
// =========================================================================

let fastfetchTerm = null;
let fastfetchFitAddon = null;
let fastfetchTermInitialized = false;
let fastfetchLastValidatedContent = null;
let fastfetchLastValidationSuccess = false;
let fastfetchLastPreviewSuccess = false;
let fastfetchRawTerminalOutput = "";
let currentFastfetchState = null;

function initFastfetchView() {
  initFastfetchTerminal();
  setupFastfetchDropzone();
  setupFastfetchEditor();
  loadFastfetchState(false);
}

function initFastfetchTerminal() {
  if (fastfetchTermInitialized) return;
  const container = document.getElementById("fastfetch-terminal-output");
  if (!container) return;

  fastfetchTermInitialized = true;
  container.innerHTML = "";

  fastfetchTerm = new Terminal({
    theme: {
      background: "#11111b",
      foreground: "#cdd6f4",
      cursor: "#f5e0dc",
      cursorAccent: "#11111b",
      selectionBackground: "#585b7066",
      black: "#45475a",
      red: "#f38ba8",
      green: "#a6e3a1",
      yellow: "#f9e2af",
      blue: "#89b4fa",
      magenta: "#f5c2e7",
      cyan: "#94e2d5",
      white: "#bac2de",
      brightBlack: "#585b70",
      brightRed: "#f38ba8",
      brightGreen: "#a6e3a1",
      brightYellow: "#f9e2af",
      brightBlue: "#89b4fa",
      brightMagenta: "#f5c2e7",
      brightCyan: "#94e2d5",
      brightWhite: "#a6adc8",
    },
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    fontSize: 12.5,
    lineHeight: 1.25,
    cursorBlink: false,
    convertEol: true,
    disableStdin: true,
  });

  if (window.FitAddon && window.FitAddon.FitAddon) {
    fastfetchFitAddon = new window.FitAddon.FitAddon();
    fastfetchTerm.loadAddon(fastfetchFitAddon);
  }

  fastfetchTerm.open(container);

  setTimeout(() => {
    if (fastfetchFitAddon) fastfetchFitAddon.fit();
  }, 100);

  window.addEventListener("resize", () => {
    if (fastfetchFitAddon && fastfetchTerm) {
      fastfetchFitAddon.fit();
    }
  });

  fastfetchTerm.writeln("\x1b[38;2;180;190;254m╔════════════════════════════════════════════════════════════════╗\x1b[0m");
  fastfetchTerm.writeln("\x1b[38;2;180;190;254m║\x1b[0m   \x1b[1;38;2;203;166;247m🚀 Terminal de Prévisualisation Fastfetch (ChomiamOS)\x1b[0m       \x1b[38;2;180;190;254m║\x1b[0m");
  fastfetchTerm.writeln("\x1b[38;2;180;190;254m╚════════════════════════════════════════════════════════════════╝\x1b[0m");
  fastfetchTerm.writeln("\x1b[38;2;166;173;200mPrêt pour la prévisualisation. Importez un profil puis cliquez sur\x1b[0m");
  fastfetchTerm.writeln("\x1b[38;2;137;180;250m« Prévisualiser »\x1b[0m \x1b[38;2;166;173;200mpour observer le rendu complet avec codes ANSI.\x1b[0m\n");
}

function setupFastfetchDropzone() {
  const dropzone = document.getElementById("fastfetch-dropzone");
  if (!dropzone) return;

  ["dragenter", "dragover"].forEach(eventName => {
    dropzone.addEventListener(eventName, (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropzone.classList.add("drag-over");
    }, false);
  });

  ["dragleave", "drop"].forEach(eventName => {
    dropzone.addEventListener(eventName, (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropzone.classList.remove("drag-over");
    }, false);
  });

  dropzone.addEventListener("drop", (e) => {
    const dt = e.dataTransfer;
    const files = dt.files;
    if (files && files.length > 0) {
      loadFastfetchFile(files[0]);
    }
  }, false);
}

function setupFastfetchEditor() {
  const editor = document.getElementById("fastfetch-editor");
  if (!editor) return;

  editor.addEventListener("input", () => {
    updateFastfetchEditorLines();
    fastfetchLastPreviewSuccess = false;
    const btnApply = document.getElementById("btn-fastfetch-apply");
    if (btnApply) btnApply.disabled = true;
    debounceFastfetchValidation();
  });
}

let fastfetchDebounceTimer = null;
function debounceFastfetchValidation() {
  clearTimeout(fastfetchDebounceTimer);
  fastfetchDebounceTimer = setTimeout(() => {
    triggerFastfetchValidation(false);
  }, 400);
}

function updateFastfetchEditorLines() {
  const editor = document.getElementById("fastfetch-editor");
  const linesEl = document.getElementById("fastfetch-editor-lines");
  if (!editor || !linesEl) return;
  const lines = editor.value ? editor.value.split("\n").length : 0;
  linesEl.textContent = `${lines} ligne${lines > 1 ? "s" : ""}`;
}

async function loadFastfetchState(showToastFeedback = false) {
  try {
    const state = await invoke("get_fastfetch_state");
    currentFastfetchState = state;

    const statusPill = document.getElementById("fastfetch-status-pill");
    const navBadge = document.getElementById("fastfetch-nav-badge");

    if (statusPill) {
      statusPill.className = "fastfetch-status-pill";
      if (state.is_nix_store) {
        statusPill.textContent = "Géré par Nix (officiel)";
        statusPill.classList.add("pill-nix");
      } else if (state.is_custom_dashboard) {
        statusPill.textContent = "Profil Personnalisé (Dashboard)";
        statusPill.classList.add("pill-custom");
      } else {
        statusPill.textContent = "Profil Manuel";
        statusPill.classList.add("pill-manual");
      }
    }

    if (navBadge) {
      navBadge.className = "nix-shell-subnav-pill";
      if (state.is_custom_dashboard) {
        navBadge.textContent = "Perso";
        navBadge.classList.add("fastfetch-pill-ok");
      } else {
        navBadge.textContent = "Nix";
        navBadge.classList.add("fastfetch-pill-ok");
      }
    }

    if (showToastFeedback) {
      showToast("État Fastfetch actualisé.", "info");
    }
  } catch (err) {
    console.error("Erreur chargement état Fastfetch :", err);
  }
}

async function loadActiveFastfetchConfig() {
  try {
    const state = await invoke("get_fastfetch_state");
    currentFastfetchState = state;
    const editor = document.getElementById("fastfetch-editor");

    if (state.current_content && editor) {
      editor.value = state.current_content;
      updateFastfetchEditorLines();
      showToast("Configuration Fastfetch active chargée dans l'éditeur !", "success");
      triggerFastfetchValidation(false);
    } else {
      showToast("Aucune configuration active n'a pu être lue.", "warning");
    }
  } catch (err) {
    showToast("Erreur lors du chargement : " + err, "error");
  }
}

function loadSampleFastfetchConfig() {
  const sample = `{
  "$schema": "https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
  "logo": {
    "type": "small",
    "padding": {
      "top": 1,
      "left": 2,
      "right": 2
    }
  },
  "display": {
    "separator": "  ",
    "constants": [
      "────────────────────────────────"
    ],
    "key": {
      "type": "icon",
      "paddingLeft": 2
    }
  },
  "modules": [
    "title",
    {
      "type": "custom",
      "format": "{$1}"
    },
    {
      "type": "os",
      "keyColor": "blue"
    },
    {
      "type": "host",
      "keyColor": "blue"
    },
    {
      "type": "kernel",
      "keyColor": "blue"
    },
    {
      "type": "uptime",
      "keyColor": "blue"
    },
    {
      "type": "packages",
      "keyColor": "blue"
    },
    {
      "type": "shell",
      "keyColor": "blue"
    },
    {
      "type": "de",
      "keyColor": "magenta"
    },
    {
      "type": "wm",
      "keyColor": "magenta"
    },
    {
      "type": "terminal",
      "keyColor": "magenta"
    },
    {
      "type": "cpu",
      "keyColor": "green"
    },
    {
      "type": "gpu",
      "keyColor": "green"
    },
    {
      "type": "memory",
      "keyColor": "green"
    },
    {
      "type": "disk",
      "keyColor": "green"
    },
    "break",
    {
      "type": "colors",
      "symbol": "circle"
    }
  ]
}`;
  const editor = document.getElementById("fastfetch-editor");
  if (editor) {
    editor.value = sample;
    updateFastfetchEditorLines();
    showToast("Modèle officiel Catppuccin chargé !", "info");
    triggerFastfetchValidation(false);
  }
}

function handleFastfetchFileSelected(event) {
  const input = event.target;
  if (input.files && input.files[0]) {
    loadFastfetchFile(input.files[0]);
  }
  input.value = "";
}

function loadFastfetchFile(file) {
  if (!file) return;
  const reader = new FileReader();
  reader.onload = (e) => {
    const text = e.target.result;
    const editor = document.getElementById("fastfetch-editor");
    if (editor) {
      editor.value = text;
      updateFastfetchEditorLines();
      showToast(`Fichier « ${file.name} » importé avec succès !`, "success");
      triggerFastfetchValidation(true);
    }
  };
  reader.onerror = () => {
    showToast("Erreur lors de la lecture du fichier.", "error");
  };
  reader.readAsText(file);
}

async function triggerFastfetchValidation(showToastOnSuccess = false) {
  const editor = document.getElementById("fastfetch-editor");
  if (!editor) return;
  const content = editor.value.trim();

  const diagCard = document.getElementById("fastfetch-diag-card");
  const diagIcon = document.getElementById("fastfetch-diag-icon");
  const diagTitle = document.getElementById("fastfetch-diag-title");
  const diagSub = document.getElementById("fastfetch-diag-subtitle");
  const diagBadge = document.getElementById("fastfetch-diag-badge");
  const errorsContainer = document.getElementById("fastfetch-errors-container");
  const errorsList = document.getElementById("fastfetch-errors-list");
  const btnPreview = document.getElementById("btn-fastfetch-preview");
  const btnApply = document.getElementById("btn-fastfetch-apply");

  if (!content) {
    diagCard.className = "card fastfetch-diag-card diag-idle";
    diagIcon.textContent = "⏳";
    diagTitle.textContent = "Éditeur vide";
    diagSub.textContent = "Collez ou importez une configuration Fastfetch pour l'analyser.";
    diagBadge.className = "badge-diag badge-idle";
    diagBadge.textContent = "Vide";
    errorsContainer.classList.add("hidden");
    if (btnPreview) btnPreview.disabled = true;
    if (btnApply) btnApply.disabled = true;
    fastfetchLastValidationSuccess = false;
    return;
  }

  try {
    const report = await invoke("validate_fastfetch_config", { content });
    fastfetchLastValidatedContent = content;
    fastfetchLastValidationSuccess = report.valid;

    if (report.valid) {
      diagCard.className = "card fastfetch-diag-card diag-valid";
      diagIcon.textContent = "✅";
      diagTitle.textContent = "Syntaxe et schéma 100% valides";
      diagSub.textContent = "Prêt pour la prévisualisation dans le terminal intégré.";
      diagBadge.className = "badge-diag badge-valid";
      diagBadge.textContent = "Conforme";
      errorsContainer.classList.add("hidden");

      if (btnPreview) btnPreview.disabled = false;
      if (btnApply) btnApply.disabled = !fastfetchLastPreviewSuccess;

      if (showToastOnSuccess) {
        showToast("Configuration Fastfetch analysée et validée avec succès !", "success");
      }
    } else {
      diagCard.className = "card fastfetch-diag-card diag-invalid";
      diagIcon.textContent = "❌";

      if (!report.syntax_valid) {
        diagTitle.textContent = "Erreur de syntaxe JSONC";
        diagSub.textContent = "Le fichier contient des erreurs de structure ou de format.";
        diagBadge.className = "badge-diag badge-error";
        diagBadge.textContent = "Syntaxe invalide";
      } else {
        diagTitle.textContent = "Erreur de schéma Fastfetch";
        diagSub.textContent = "Propriétés ou modules non conformes à la spécification officielle.";
        diagBadge.className = "badge-diag badge-error";
        diagBadge.textContent = "Schéma non respecté";
      }

      errorsContainer.classList.remove("hidden");
      errorsList.innerHTML = "";

      report.errors.forEach(err => {
        const item = document.createElement("div");
        item.className = "fastfetch-error-item" + (err.kind === "schema" ? " is-schema" : "");

        const badge = document.createElement("span");
        badge.className = "fastfetch-error-badge" + (err.kind === "schema" ? " badge-schema" : "");
        badge.textContent = err.kind === "schema" ? "Schéma" : "Syntaxe";
        item.appendChild(badge);

        if (err.line !== null && err.line !== undefined) {
          const loc = document.createElement("span");
          loc.className = "fastfetch-error-loc";
          loc.textContent = `Ligne ${err.line}:${err.column || 1}`;
          item.appendChild(loc);
        }

        if (err.instance_path) {
          const path = document.createElement("span");
          path.className = "fastfetch-error-path";
          path.textContent = err.instance_path;
          item.appendChild(path);
        }

        const msg = document.createElement("span");
        msg.className = "fastfetch-error-msg";
        msg.textContent = err.message;
        item.appendChild(msg);

        errorsList.appendChild(item);
      });

      if (btnPreview) btnPreview.disabled = true;
      if (btnApply) btnApply.disabled = true;
      fastfetchLastPreviewSuccess = false;
    }
  } catch (err) {
    console.error("Erreur validation :", err);
    showToast("Erreur lors de la validation : " + err, "error");
  }
}

async function triggerFastfetchPreview() {
  const editor = document.getElementById("fastfetch-editor");
  if (!editor) return;
  const content = editor.value.trim();
  if (!content) return;

  const btnPreview = document.getElementById("btn-fastfetch-preview");
  const btnApply = document.getElementById("btn-fastfetch-apply");
  const statusEl = document.getElementById("fastfetch-term-status");
  const durationEl = document.getElementById("fastfetch-exec-duration");

  if (btnPreview) {
    btnPreview.disabled = true;
    btnPreview.innerHTML = "<span>⏳</span> Exécution...";
  }
  if (statusEl) {
    statusEl.textContent = "Exécution...";
    statusEl.className = "fastfetch-term-pill running";
  }

  const startTime = performance.now();

  try {
    const output = await invoke("preview_fastfetch_config", { content });
    const elapsed = Math.round(performance.now() - startTime);

    if (durationEl) durationEl.textContent = `${elapsed} ms`;
    fastfetchRawTerminalOutput = output;

    if (fastfetchTerm) {
      fastfetchTerm.clear();
      fastfetchTerm.write(output);
    }

    fastfetchLastPreviewSuccess = true;
    if (btnApply) btnApply.disabled = false;

    if (statusEl) {
      statusEl.textContent = "Rendu OK";
      statusEl.className = "fastfetch-term-pill";
    }

    showToast("Prévisualisation Fastfetch générée avec succès !", "success");
  } catch (err) {
    console.error("Erreur preview fastfetch :", err);
    showToast("Erreur lors de la prévisualisation : " + err, "error");
    if (statusEl) {
      statusEl.textContent = "Erreur";
      statusEl.className = "fastfetch-term-pill running";
    }
  } finally {
    if (btnPreview) {
      btnPreview.disabled = false;
      btnPreview.innerHTML = "<span>▶️</span> Prévisualiser";
    }
  }
}

async function triggerFastfetchApply() {
  const editor = document.getElementById("fastfetch-editor");
  if (!editor) return;
  const content = editor.value.trim();
  if (!content) return;

  if (!fastfetchLastValidationSuccess) {
    showToast("Veuillez valider rigoureusement le profil avant de l'appliquer.", "warning");
    return;
  }

  if (!confirm("Voulez-vous appliquer ce profil Fastfetch pour votre session utilisateur ?\n\n- Le profil sera enregistré dans ~/.config/fastfetch/profiles/dashboard.jsonc\n- Si un fichier manuel existe, une sauvegarde horodatée sera créée.\n- Vos déclarations NixOS restent intactes et protégées.")) {
    return;
  }

  const btnApply = document.getElementById("btn-fastfetch-apply");
  if (btnApply) {
    btnApply.disabled = true;
    btnApply.innerHTML = "<span>⏳</span> Application...";
  }

  try {
    const res = await invoke("apply_fastfetch_profile", { content });
    showToast(res, "success");
    await loadFastfetchState(false);
  } catch (err) {
    console.error("Erreur application Fastfetch :", err);
    showToast("Erreur lors de l'application : " + err, "error");
  } finally {
    if (btnApply) {
      btnApply.disabled = false;
      btnApply.innerHTML = "<span>💾</span> Appliquer le profil";
    }
  }
}

async function restoreDefaultFastfetchConfig() {
  if (!confirm("Voulez-vous rétablir le profil Fastfetch officiel par défaut de ChomiamOS ?")) {
    return;
  }

  try {
    const res = await invoke("restore_fastfetch_default");
    showToast(res, "success");
    await loadFastfetchState(false);
    await loadActiveFastfetchConfig();
  } catch (err) {
    showToast("Erreur lors du rétablissement : " + err, "error");
  }
}

function clearFastfetchTerminal() {
  if (fastfetchTerm) {
    fastfetchTerm.clear();
  }
}

function copyFastfetchTerminalOutput() {
  if (!fastfetchRawTerminalOutput) {
    showToast("Aucune sortie de terminal à copier.", "info");
    return;
  }
  const plainText = fastfetchRawTerminalOutput.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, "");
  navigator.clipboard.writeText(plainText).then(() => {
    showToast("Rendu Fastfetch copié (texte brut) !", "success");
  }).catch(err => {
    showToast("Impossible de copier : " + err, "error");
  });
}

function clearFastfetchEditor() {
  const editor = document.getElementById("fastfetch-editor");
  if (editor) {
    editor.value = "";
    updateFastfetchEditorLines();
    triggerFastfetchValidation(false);
  }
}

function formatFastfetchEditor() {
  const editor = document.getElementById("fastfetch-editor");
  if (!editor || !editor.value.trim()) return;

  try {
    const cleaned = editor.value.replace(/\/\/.*$/gm, "").replace(/\/\*[\s\S]*?\*\//g, "").replace(/,(\s*[}\]])/g, "$1");
    const obj = JSON.parse(cleaned);
    editor.value = JSON.stringify(obj, null, 2);
    updateFastfetchEditorLines();
    showToast("Configuration formatée avec succès !", "info");
    triggerFastfetchValidation(false);
  } catch (err) {
    showToast("Formatage impossible : la syntaxe doit être valide.", "warning");
  }
}

window.initFastfetchView = initFastfetchView;
window.loadActiveFastfetchConfig = loadActiveFastfetchConfig;
window.loadSampleFastfetchConfig = loadSampleFastfetchConfig;
window.handleFastfetchFileSelected = handleFastfetchFileSelected;
window.triggerFastfetchValidation = triggerFastfetchValidation;
window.triggerFastfetchPreview = triggerFastfetchPreview;
window.triggerFastfetchApply = triggerFastfetchApply;
window.restoreDefaultFastfetchConfig = restoreDefaultFastfetchConfig;
window.clearFastfetchTerminal = clearFastfetchTerminal;
window.copyFastfetchTerminalOutput = copyFastfetchTerminalOutput;
window.clearFastfetchEditor = clearFastfetchEditor;
window.formatFastfetchEditor = formatFastfetchEditor;
