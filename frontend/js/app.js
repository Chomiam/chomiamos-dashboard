// ==========================================================================
// ChomiamOS Dashboard Frontend JavaScript
// ==========================================================================

let currentConfig = null;
let initialConfigStr = "";
let autoScroll = true;
let eventSource = null;

document.addEventListener("DOMContentLoaded", () => {
  initTabs();
  startMetricsPolling();
  loadGenerations();
  loadConfig();
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
    const res = await fetch("/api/system/metrics");
    if (!res.ok) return;
    const m = await res.json();
    renderMetrics(m);
  } catch (err) {
    console.error("Erreur métriques:", err);
  }
}

function renderMetrics(m) {
  // Nav items
  document.getElementById("nav-kernel").textContent = m.kernel_version;
  document.getElementById("nav-uptime").textContent = formatUptime(m.uptime_seconds);
  document.getElementById("meta-host").textContent = m.hostname;

  // CPU
  document.getElementById("cpu-model").textContent = m.cpu_model;
  document.getElementById("cpu-freq").textContent = `${(m.cpu_freq_mhz / 1000).toFixed(2)} GHz`;
  document.getElementById("cpu-percent").textContent = `${Math.round(m.cpu_usage_percent)}%`;
  setGauge("cpu-gauge", m.cpu_usage_percent);

  // Cores meter
  const coresBar = document.getElementById("cpu-cores-bar");
  if (coresBar && m.cpu_cores_usage) {
    if (coresBar.children.length !== m.cpu_cores_usage.length) {
      coresBar.innerHTML = m.cpu_cores_usage.map(() => `
        <div class="core-bar"><div class="core-bar-fill"></div></div>
      `).join("");
    }
    const fills = coresBar.querySelectorAll(".core-bar-fill");
    m.cpu_cores_usage.forEach((pct, i) => {
      if (fills[i]) fills[i].style.transform = `scaleY(${pct / 100})`;
    });
  }

  // GPU
  if (m.gpu) {
    document.getElementById("gpu-name").textContent = m.gpu.name;
    document.getElementById("gpu-driver").textContent = m.gpu.driver;
    if (m.gpu.usage_percent !== null) {
      document.getElementById("gpu-percent").textContent = `${Math.round(m.gpu.usage_percent)}%`;
      setGauge("gpu-gauge", m.gpu.usage_percent);
    }
    if (m.gpu.temp_celsius !== null) {
      document.getElementById("gpu-temp").textContent = `${m.gpu.temp_celsius} °C`;
    }
    if (m.gpu.vram_used_bytes && m.gpu.vram_total_bytes) {
      const usedGb = (m.gpu.vram_used_bytes / (1024**3)).toFixed(1);
      const totalGb = (m.gpu.vram_total_bytes / (1024**3)).toFixed(1);
      document.getElementById("vram-val").textContent = `${usedGb} / ${totalGb} Go`;
    }
  }

  // RAM
  const ramUsedGb = (m.ram_used_bytes / (1024**3)).toFixed(1);
  const ramTotalGb = (m.ram_total_bytes / (1024**3)).toFixed(1);
  document.getElementById("ram-numbers").textContent = `${ramUsedGb} / ${ramTotalGb} Go`;
  document.getElementById("ram-percent").textContent = `${Math.round(m.ram_used_percent)}%`;
  document.getElementById("ram-bar").style.width = `${m.ram_used_percent}%`;
  setGauge("ram-gauge", m.ram_used_percent);

  const swapUsedGb = (m.swap_used_bytes / (1024**3)).toFixed(1);
  document.getElementById("swap-val").textContent = `Swap: ${swapUsedGb} Go`;

  // Root Storage
  const rootDisk = m.disks.find(d => d.mount_point === "/") || m.disks[0];
  if (rootDisk) {
    const dUsedGb = (rootDisk.used_bytes / (1024**3)).toFixed(0);
    const dTotalGb = (rootDisk.total_bytes / (1024**3)).toFixed(0);
    document.getElementById("root-disk-numbers").textContent = `${dUsedGb} / ${dTotalGb} Go`;
    document.getElementById("root-disk-percent").textContent = `${Math.round(rootDisk.used_percent)}%`;
    document.getElementById("root-disk-bar").style.width = `${rootDisk.used_percent}%`;
    setGauge("disk-gauge", rootDisk.used_percent);
  }

  // Render Disks in Tab 2
  renderDisksTab(m.disks);
}

function renderDisksTab(disks) {
  const container = document.getElementById("disks-list");
  if (!container) return;

  container.innerHTML = disks.map(d => {
    const usedGb = (d.used_bytes / (1024**3)).toFixed(1);
    const totalGb = (d.total_bytes / (1024**3)).toFixed(1);
    const freeGb = (d.available_bytes / (1024**3)).toFixed(1);
    return `
      <div class="card">
        <div class="card-header">
          <div class="card-title-wrap">
            <span class="card-icon">📁</span>
            <div>
              <h3>${d.mount_point}</h3>
              <p class="card-subtitle">${d.name} (${d.file_system})</p>
            </div>
          </div>
          <span class="metric-tag">${d.used_percent}%</span>
        </div>
        <div class="progress-bar-wrap" style="height: 10px; margin-top: 14px;">
          <div class="progress-bar-inner disk-fill" style="width: ${d.used_percent}%"></div>
        </div>
        <div class="submetric-box" style="margin-top: 12px;">
          <span>${usedGb} Go occupés sur ${totalGb} Go</span>
          <span style="color: var(--green);">${freeGb} Go libres</span>
        </div>
      </div>
    `;
  }).join("");
}

function setGauge(id, percent) {
  const circle = document.getElementById(id);
  if (!circle) return;
  const radius = circle.r.baseVal.value;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference - (Math.min(100, Math.max(0, percent)) / 100) * circumference;
  circle.style.strokeDashoffset = offset;
}

function formatUptime(sec) {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

// 3. Generations Management
async function loadGenerations() {
  try {
    const res = await fetch("/api/generations");
    if (!res.ok) return;
    const data = await res.json();

    document.getElementById("store-size-val").textContent = data.store_size;
    document.getElementById("generations-count-val").textContent = data.count;

    const tbody = document.getElementById("generations-tbody");
    if (!tbody) return;

    if (data.generations.length === 0) {
      tbody.innerHTML = `<tr><td colspan="5" style="text-align:center; color:var(--text-muted);">Aucune génération détectée</td></tr>`;
      return;
    }

    tbody.innerHTML = data.generations.map(g => `
      <tr>
        <td><strong>#${g.id}</strong></td>
        <td>${g.date}</td>
        <td><code>${g.nixos_version}</code></td>
        <td><code>${g.kernel}</code></td>
        <td>
          <span class="badge-gen ${g.current ? 'badge-active' : 'badge-inactive'}">
            ${g.current ? '✔ Actuelle' : 'Archive'}
          </span>
        </td>
      </tr>
    `).join("");
  } catch (e) {
    console.error("Erreur générations:", e);
  }
}

// 4. Configuration Management
async function loadConfig() {
  try {
    const res = await fetch("/api/config");
    if (!res.ok) return;
    currentConfig = await res.json();
    initialConfigStr = JSON.stringify(currentConfig);
    populateConfigUI(currentConfig);
    attachConfigChangeListeners();
  } catch (e) {
    console.error("Erreur chargement configuration:", e);
  }
}

function populateConfigUI(c) {
  // Radio: Browser
  setRadioVal("browser", c.browser);
  // Radio: Discord
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
    const res = await fetch("/api/config/save", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(currentConfig),
    });
    if (!res.ok) {
      alert("Erreur lors de la sauvegarde de la configuration");
      return;
    }
    initialConfigStr = JSON.stringify(currentConfig);
    checkDirtyState();

    if (andApply) {
      runAction("switch");
    } else {
      alert("Configuration sauvegardée avec succès dans /etc/nixos/vars.nix !");
    }
  } catch (e) {
    alert("Erreur réseau: " + e);
  }
}

// 5. Streaming Terminal Runner
function runAction(action) {
  const modal = document.getElementById("terminal-modal");
  const output = document.getElementById("terminal-output");
  const statusDot = document.getElementById("term-status-icon");
  const statusText = document.getElementById("term-status-text");

  modal.classList.remove("hidden");
  output.innerHTML = "";
  statusDot.className = "status-dot running";
  statusText.textContent = "Exécution de la commande en cours...";

  if (eventSource) {
    eventSource.close();
  }

  eventSource = new EventSource(`/api/action/stream?action=${encodeURIComponent(action)}`);

  eventSource.onmessage = (event) => {
    appendTerminal(event.data);
  };

  eventSource.onerror = () => {
    eventSource.close();
    eventSource = null;
    statusDot.className = "status-dot success";
    statusText.textContent = "Terminé (flux fermé)";
    // Refresh generations if cleanup was executed
    loadGenerations();
  };
}

function appendTerminal(line) {
  const output = document.getElementById("terminal-output");
  const p = document.createElement("div");
  p.innerHTML = ansiToHtml(line);
  output.appendChild(p);

  if (autoScroll) {
    output.scrollTop = output.scrollHeight;
  }
}

function clearTerminal() {
  document.getElementById("terminal-output").innerHTML = "";
}

function closeTerminal() {
  if (eventSource) {
    eventSource.close();
    eventSource = null;
  }
  document.getElementById("terminal-modal").classList.add("hidden");
}

function toggleAutoScroll() {
  autoScroll = !autoScroll;
  const btn = document.getElementById("term-autoscroll-btn");
  btn.textContent = `Auto-scroll: ${autoScroll ? 'ON' : 'OFF'}`;
  btn.className = `term-chip ${autoScroll ? 'active' : ''}`;
}

// Convert common ANSI escape codes to colored HTML spans
function ansiToHtml(text) {
  return text
    .replace(/\x1b\[31m/g, '<span style="color: var(--red);">')
    .replace(/\x1b\[32m/g, '<span style="color: var(--green);">')
    .replace(/\x1b\[33m/g, '<span style="color: var(--yellow);">')
    .replace(/\x1b\[34m/g, '<span style="color: var(--blue);">')
    .replace(/\x1b\[35m/g, '<span style="color: var(--mauve);">')
    .replace(/\x1b\[36m/g, '<span style="color: var(--teal);">')
    .replace(/\x1b\[0m/g, '</span>');
}
