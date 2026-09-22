import { getAppConf, checkServer, startProxy, fetchCloudServers, type AppConf, type CloudServerInfo } from "../lib/api";
import { getSettings, saveServer, type ServerEntry } from "../lib/store";
import { navigate } from "../lib/router";
import { t, getLang, setLang, getTheme, setTheme } from "../lib/i18n";
import { normalizeServerUrl } from "../lib/url";

const DEFAULT_CONF: AppConf = {
  name: "Yao Agents",
  logo: "",
  port: 15099,
  theme: { primaryColor: "#3373fc" },
  updater: { active: false, endpoints: [], pubkey: "" },
  servers: [],
};

let _serversSyncCleanup: (() => void) | null = null;
let _autoReconnectDone = false;

// ---- Page state ----
let _currentTab = 0; // 0 = cloud, 1 = custom
let _cloudServers: CloudServerInfo[] = [];
let _cloudSelectedUrl = "";
let _cloudLoading = false;
let _cloudError: string | null = null;
let _customUrl = "";
let _dropdownOpen = false;
let _panelRendered = false;

/** Render the server selection page */
export async function renderServers(): Promise<void> {
  const app = document.getElementById("app")!;

  _serversSyncCleanup?.();
  const onTheme = () => renderServers();
  const onLang = () => {
    // Only reload if not already loading (toggle-lang handler may have started it)
    if (!_cloudLoading) {
      _cloudServers = [];
      _cloudError = null;
      loadCloudServers().then(() => renderServers());
    }
  };
  window.addEventListener("cui:theme-sync", onTheme);
  window.addEventListener("cui:lang-sync", onLang);
  _serversSyncCleanup = () => {
    window.removeEventListener("cui:theme-sync", onTheme);
    window.removeEventListener("cui:lang-sync", onLang);
  };

  let conf = DEFAULT_CONF;
  try {
    conf = await getAppConf();
  } catch {
    // Use defaults if Tauri bridge not ready
  }

  let settings = { servers: [] as ServerEntry[], activeServerUrl: "" };
  try {
    settings = await getSettings();
  } catch {
    // Store not initialized yet
  }

  // "Switch server" from tray menu passes ?switch=1 — skip auto-reconnect
  const urlParams = new URLSearchParams(window.location.search);
  if (urlParams.has("switch")) {
    _autoReconnectDone = true;
    window.history.replaceState({}, "", window.location.pathname);
  }

  // One-shot auto-reconnect: on first render, if there was an active server, reconnect
  if (!_autoReconnectDone && settings.activeServerUrl) {
    _autoReconnectDone = true;
    app.innerHTML = `
      <div class="page-servers">
        <div class="servers-panel fade-in" style="display:flex;flex-direction:column;align-items:center;justify-content:center;min-height:320px">
          <div class="spinner spinner-dark spinner-lg"></div>
          <p style="margin-top:16px;color:#6b7280;font-size:14px">${escapeHtml(t("app.connecting"))} ${escapeHtml(settings.activeServerUrl)}</p>
        </div>
      </div>
    `;
    try {
      let dashboard = "";
      let webproxyDomain = "";
      const info = await checkServer(settings.activeServerUrl);
      if (info.dashboard) dashboard = info.dashboard;
      if (info.webproxy?.domain) webproxyDomain = info.webproxy.domain;
      await startProxy(settings.activeServerUrl, "", "openapi", dashboard, webproxyDomain);
      navigate("/app");
      return;
    } catch {
      // Server unreachable, fall through to render server selection page
    }
  }

  // Apply theme
  const primary = conf.theme?.primaryColor || "#3373fc";
  document.documentElement.style.setProperty("--color-main", primary);

  // Load cloud servers on first render (non-blocking)
  if (_cloudServers.length === 0 && !_cloudLoading && !_cloudError) {
    loadCloudServers().then(() => {
      // After loading, determine tab for saved URL
      if (settings.activeServerUrl) {
        const isCloud = _cloudServers.some(s => s.url.replace(/\/$/, "") === settings.activeServerUrl.replace(/\/$/, ""));
        if (isCloud) {
          _currentTab = 0;
          _cloudSelectedUrl = settings.activeServerUrl;
        } else {
          _currentTab = 1;
          _customUrl = settings.activeServerUrl;
        }
      }
      renderServers();
    });
  }

  const logoSrc = conf.logo || "/icon.png";
  const langLabel = getLang() === "zh" ? "EN" : "中";
  const themeIcon = getTheme() === "dark"
    ? '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><path d="M12 1v2m0 18v2M4.22 4.22l1.42 1.42m12.72 12.72l1.42 1.42M1 12h2m18 0h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>'
    : '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>';

  const panelClass = _panelRendered ? "servers-panel" : "servers-panel fade-in";
  _panelRendered = true;

  app.innerHTML = `
    <div class="page-servers">
      <div class="quick-settings" id="quick-settings">
        <button class="quick-settings-btn" id="toggle-lang">${langLabel}</button>
        <button class="quick-settings-btn theme-btn" id="toggle-theme">${themeIcon}</button>
      </div>
      <div class="${panelClass}">
        <div class="servers-brand">
          <img src="${esc(logoSrc)}" alt="" class="brand-logo" />
          <h1 class="brand-name">${escapeHtml(conf.name)}</h1>
          <span class="server-subtitle">${escapeHtml(t("app.server_subtitle"))}</span>
        </div>

        <div id="alert-area"></div>

        <div class="capsule-tabs">
          <button class="capsule-tab${_currentTab === 0 ? " active" : ""}" data-tab="0">${escapeHtml(t("app.cloud_section"))}</button>
          <button class="capsule-tab${_currentTab === 1 ? " active" : ""}" data-tab="1">${escapeHtml(t("app.custom_section"))}</button>
        </div>

        <div id="tab-content">
          ${_currentTab === 0 ? renderCloudTab() : renderCustomTab()}
        </div>

        <div class="connect-section">
          <button class="btn-main btn-lg" id="connect-btn" ${isConnectEnabled() ? "" : "disabled"}>${escapeHtml(t("app.connect"))}</button>
        </div>

        ${_currentTab === 0 ? renderInviteHelper() : ""}

      </div>
    </div>
  `;

  bind(conf);
}

// ---- Tab renderers ----

function renderCloudTab(): string {
  if (_cloudLoading) {
    return `<div class="cloud-state"><div class="spinner"></div><span>${escapeHtml(t("app.cloud_loading"))}</span></div>`;
  }
  if (_cloudError) {
    return `
      <div class="cloud-error">
        <div class="cloud-error-text">${escapeHtml(t("app.cloud_load_failed"))}</div>
        <button class="btn-text" id="cloud-retry">${escapeHtml(t("app.cloud_retry"))}</button>
      </div>`;
  }
  if (_cloudServers.length === 0) {
    return `<div class="cloud-state"><span>${escapeHtml(t("app.cloud_empty"))}</span></div>`;
  }

  const selected = _cloudServers.find(s => s.url === _cloudSelectedUrl);
  const nameHtml = selected
    ? `<div class="cloud-picker-name">${escapeHtml(selected.name)}</div>
       <div class="cloud-picker-url">${escapeHtml(selected.url)}</div>`
    : `<div class="cloud-picker-name placeholder">${escapeHtml(t("app.cloud_select_hint"))}</div>`;

  let dropdownHtml = "";
  if (_dropdownOpen) {
    const items = _cloudServers.map(s => {
      const sel = s.url === _cloudSelectedUrl ? " selected" : "";
      const group = [s.region, s.url].filter(Boolean).join(" · ");
      return `<div class="cloud-dropdown-item${sel}" data-cloud-url="${esc(s.url)}">
        <div class="cloud-dropdown-name">${escapeHtml(s.name)}</div>
        <div class="cloud-dropdown-group">${escapeHtml(group)}</div>
      </div>`;
    }).join("");
    dropdownHtml = `<div class="cloud-dropdown">${items}</div>`;
  }

  return `
    <div class="cloud-picker-wrap">
      <div class="cloud-picker" id="cloud-picker">
        <div class="cloud-picker-info">${nameHtml}</div>
        <span class="picker-arrow${_dropdownOpen ? " open" : ""}">▾</span>
      </div>
      ${dropdownHtml}
    </div>`;
}

function renderCustomTab(): string {
  return `
    <div>
      <input type="text" class="custom-input" id="custom-url"
        placeholder="https://your-server.com"
        value="${esc(_customUrl)}" />
      <div class="custom-hint">${escapeHtml(t("app.custom_full_hint"))}</div>
    </div>`;
}

function renderInviteHelper(): string {
  const isZh = getLang() === "zh";
  const registerUrl = isZh ? "https://yaoagents.cn/servers" : "https://yaoagents.com/servers";
  return `
    <div class="invite-helper">
      <div>${escapeHtml(t("app.register_helper"))}</div>
      <a href="${esc(registerUrl)}" target="_blank">${escapeHtml(t("app.register_link"))}</a>
    </div>`;
}

function isConnectEnabled(): boolean {
  if (_currentTab === 0) return !!_cloudSelectedUrl;
  return !!_customUrl.trim();
}

function effectiveUrl(): string {
  if (_currentTab === 0) return _cloudSelectedUrl;
  return _customUrl.trim();
}

function effectiveLabel(): string {
  if (_currentTab === 0) {
    const s = _cloudServers.find(s => s.url === _cloudSelectedUrl);
    return s?.name || _cloudSelectedUrl;
  }
  return _customUrl.trim().replace(/^https?:\/\//, "");
}

// ---- Cloud server loading ----

async function loadCloudServers(): Promise<void> {
  if (_cloudLoading) return;
  _cloudLoading = true;
  _cloudError = null;

  const lang = getLang();
  const cloudBase = lang === "zh" ? "https://yaoagents.cn" : "https://yaoagents.com";
  const locale = lang === "zh" ? "zh-cn" : "en-US";

  try {
    const list = await fetchCloudServers(cloudBase, locale);
    _cloudServers = list;
    _cloudLoading = false;
    if (!_cloudSelectedUrl && list.length > 0) {
      _cloudSelectedUrl = list[0].url;
    }
  } catch (e: any) {
    _cloudLoading = false;
    _cloudError = typeof e === "string" ? e : e?.message ?? "Unknown error";
  }
}

// ---- Event binding ----

function bind(_conf: AppConf) {
  const alertArea = document.getElementById("alert-area")!;

  // Tab switching
  document.querySelectorAll(".capsule-tab").forEach(btn => {
    btn.addEventListener("click", () => {
      const tab = parseInt((btn as HTMLElement).dataset.tab || "0", 10);
      if (tab !== _currentTab) {
        _currentTab = tab;
        _dropdownOpen = false;
        renderServers();
      }
    });
  });

  // Cloud picker toggle
  document.getElementById("cloud-picker")?.addEventListener("click", () => {
    _dropdownOpen = !_dropdownOpen;
    renderServers();
  });

  // Cloud dropdown item selection
  document.querySelectorAll(".cloud-dropdown-item").forEach(item => {
    item.addEventListener("click", (e) => {
      e.stopPropagation();
      _cloudSelectedUrl = (item as HTMLElement).dataset.cloudUrl || "";
      _dropdownOpen = false;
      renderServers();
    });
  });

  // Close dropdown on outside click (delayed to avoid consuming the opening click)
  if (_dropdownOpen) {
    requestAnimationFrame(() => {
      const handler = (e: MouseEvent) => {
        const wrap = document.querySelector(".cloud-picker-wrap");
        if (wrap && !wrap.contains(e.target as Node)) {
          _dropdownOpen = false;
          renderServers();
        }
        document.removeEventListener("click", handler, true);
      };
      document.addEventListener("click", handler, true);
    });
  }

  // Cloud retry
  document.getElementById("cloud-retry")?.addEventListener("click", () => {
    loadCloudServers().then(() => renderServers());
  });

  // Custom URL input
  const customInput = document.getElementById("custom-url") as HTMLInputElement | null;
  if (customInput) {
    customInput.addEventListener("input", () => {
      _customUrl = customInput.value;
      const connectBtn = document.getElementById("connect-btn") as HTMLButtonElement | null;
      if (connectBtn) connectBtn.disabled = !isConnectEnabled();
    });
    customInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && isConnectEnabled()) {
        const normalized = normalizeServerUrl(effectiveUrl()) || effectiveUrl();
        doConnect(normalized, effectiveLabel(), alertArea);
      }
    });
  }

  // Connect button
  document.getElementById("connect-btn")?.addEventListener("click", () => {
    if (!isConnectEnabled()) return;
    if (_currentTab === 0) {
      doConnect(effectiveUrl(), effectiveLabel(), alertArea, true);
    } else {
      doConnect(normalizeServerUrl(effectiveUrl()) || effectiveUrl(), effectiveLabel(), alertArea);
    }
  });


  // Theme toggle
  document.getElementById("toggle-theme")?.addEventListener("click", () => {
    setTheme(getTheme() === "dark" ? "light" : "dark");
    renderServers();
  });

  // Language toggle — reset cloud cache, set lang, then reload directly.
  // setLang also dispatches cui:lang-sync asynchronously via Rust; we guard
  // against double-loading with the _cloudLoading flag in loadCloudServers().
  document.getElementById("toggle-lang")?.addEventListener("click", () => {
    const newLang = getLang() === "zh" ? "en" : "zh";
    _cloudServers = [];
    _cloudLoading = false;
    _cloudError = null;
    setLang(newLang);
    loadCloudServers().then(() => renderServers());
  });
}

async function doConnect(rawUrl: string, label: string, alertArea: HTMLElement, skipNormalize = false) {
  const url = skipNormalize ? rawUrl : (normalizeServerUrl(rawUrl) || rawUrl);
  const connectBtn = document.getElementById("connect-btn") as HTMLButtonElement | null;
  if (connectBtn) connectBtn.disabled = true;
  showAlert(alertArea, "info", t("app.connecting"));

  try {
    let name = label || url.replace(/^https?:\/\//, "");
    let dashboard = "";
    let webproxyDomain = "";

    const info = await checkServer(url);
    if (info.name) name = info.name;
    if (info.dashboard) dashboard = info.dashboard;
    if (info.webproxy?.domain) webproxyDomain = info.webproxy.domain;

    await saveServer({ url, label: name, lastConnected: Date.now() });

    showAlert(alertArea, "info", t("app.starting_proxy"));
    await startProxy(url, "", "openapi", dashboard, webproxyDomain);

    showAlert(alertArea, "success", t("app.connected"));
    setTimeout(() => navigate("/app"), 300);
  } catch (err: any) {
    showAlert(alertArea, "error", `${t("app.connection_failed")}${typeof err === "string" ? err : err?.message ?? String(err)}`);
  } finally {
    if (connectBtn) connectBtn.disabled = !isConnectEnabled();
  }
}

// ---- Helpers ----

function showAlert(el: HTMLElement, type: "error" | "success" | "info", msg: string) {
  el.innerHTML = `<div class="alert alert-${type}">${escapeHtml(msg)}</div>`;
}

function escapeHtml(s: string): string { const d = document.createElement("div"); d.textContent = s ?? ""; return d.innerHTML; }
function esc(s: string): string { return (s ?? "").replace(/&/g,"&amp;").replace(/"/g,"&quot;").replace(/</g,"&lt;").replace(/>/g,"&gt;"); }
