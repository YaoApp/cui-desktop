import { getAppConf, getProxyStatus, clearCookies } from "../lib/api";
import { clearAll } from "../lib/store";
import { t, getLang, setLang, getTheme, setTheme } from "../lib/i18n";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

let _settingsSyncCleanup: (() => void) | null = null;

/** Render the settings page (used in standalone settings window) */
export async function renderSettings(): Promise<void> {
  const app = document.getElementById("app")!;

  _settingsSyncCleanup?.();
  const onTheme = () => renderSettings();
  const onLang = () => renderSettings();
  window.addEventListener("cui:theme-sync", onTheme);
  window.addEventListener("cui:lang-sync", onLang);
  _settingsSyncCleanup = () => {
    window.removeEventListener("cui:theme-sync", onTheme);
    window.removeEventListener("cui:lang-sync", onLang);
  };

  let appName = "Yao Agents";
  try {
    const c = await getAppConf();
    appName = c.name;
  } catch { /* ignore */ }

  let appVersion = "0.0.0";
  try {
    appVersion = await getVersion();
  } catch { /* ignore */ }

  let proxy = { running: false, port: 15099, server_url: "", token: "", auth_mode: "" };
  try {
    proxy = await getProxyStatus();
  } catch { /* ignore */ }

  const currentLang = getLang();
  const currentTheme = getTheme();

  app.innerHTML = `
    <div class="settings-container fade-in">
      <div class="settings-header">
        <h1>${escapeHtml(t("settings.title"))}</h1>
        <button class="btn-ghost btn-sm" id="close-btn">${escapeHtml(t("settings.close"))}</button>
      </div>

      <div class="settings-section">
        <h2>${escapeHtml(t("settings.appearance"))}</h2>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.language"))}</span>
          <select id="lang-select" class="settings-select">
            <option value="zh" ${currentLang === "zh" ? "selected" : ""}>中文</option>
            <option value="en" ${currentLang === "en" ? "selected" : ""}>English</option>
          </select>
        </div>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.theme"))}</span>
          <select id="theme-select" class="settings-select">
            <option value="light" ${currentTheme === "light" ? "selected" : ""}>Light</option>
            <option value="dark" ${currentTheme === "dark" ? "selected" : ""}>Dark</option>
          </select>
        </div>
      </div>

      <div class="settings-section">
        <h2>${escapeHtml(t("settings.about"))}</h2>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.application"))}</span>
          <span class="value">${escapeHtml(appName)}</span>
        </div>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.version"))}</span>
          <span class="value">v${escapeHtml(appVersion)}</span>
        </div>
        <div class="settings-item">
          <span class="label"></span>
          <button class="btn-ghost btn-sm" id="check-update-btn">${escapeHtml(t("settings.check_update"))}</button>
          <span id="update-status" class="value" style="margin-left:8px"></span>
        </div>
      </div>

      <div class="settings-section">
        <h2>${escapeHtml(t("settings.proxy"))}</h2>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.status"))}</span>
          <span class="status-badge ${proxy.running ? "online" : "offline"}">
            ${proxy.running ? escapeHtml(t("settings.running")) : escapeHtml(t("settings.stopped"))}
          </span>
        </div>
        ${proxy.running ? `
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.local"))}</span>
          <span class="value">http://127.0.0.1:${proxy.port}</span>
        </div>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.remote"))}</span>
          <span class="value">${escapeHtml(proxy.server_url)}</span>
        </div>` : ""}
      </div>

      <div class="settings-section">
        <h2>${escapeHtml(t("settings.data"))}</h2>
        <div class="settings-actions">
          <button class="btn-ghost btn-sm" id="clear-cookies-btn">${escapeHtml(t("settings.clear_cookies"))}</button>
          <button class="btn-danger-solid" id="clear-all-btn">${escapeHtml(t("settings.clear_all"))}</button>
        </div>
      </div>
    </div>
  `;

  // Close button — close the independent settings window
  document.getElementById("close-btn")!.addEventListener("click", async () => {
    try {
      await getCurrentWindow().close();
    } catch {
      window.close();
    }
  });

  // Language switch
  document.getElementById("lang-select")!.addEventListener("change", (e) => {
    const lang = (e.target as HTMLSelectElement).value;
    setLang(lang);
    renderSettings();
  });

  // Theme switch
  document.getElementById("theme-select")!.addEventListener("change", (e) => {
    const theme = (e.target as HTMLSelectElement).value;
    setTheme(theme);
    renderSettings();
  });

  // Check for update
  document.getElementById("check-update-btn")!.addEventListener("click", async () => {
    const statusEl = document.getElementById("update-status")!;
    const btn = document.getElementById("check-update-btn") as HTMLButtonElement;
    btn.disabled = true;
    statusEl.textContent = t("settings.checking");

    try {
      const update = await check();
      if (update) {
        statusEl.textContent = t("settings.new_version").replace("{version}", update.version);
        btn.textContent = t("settings.downloading");
        await update.downloadAndInstall();
        btn.textContent = t("settings.restart_now");
        btn.disabled = false;
        btn.onclick = async () => { await relaunch(); };
      } else {
        statusEl.textContent = t("settings.up_to_date");
        btn.disabled = false;
      }
    } catch (err) {
      statusEl.textContent = `${t("settings.update_error")}: ${err}`;
      btn.disabled = false;
    }
  });

  // Data actions
  document.getElementById("clear-cookies-btn")!.addEventListener("click", async () => {
    if (confirm(t("settings.confirm_cookies"))) {
      await clearCookies();
      alert(t("settings.cookies_cleared"));
    }
  });

  document.getElementById("clear-all-btn")!.addEventListener("click", async () => {
    if (confirm(t("settings.confirm_all"))) {
      await clearCookies();
      await clearAll();
      try {
        await getCurrentWindow().close();
      } catch {
        window.close();
      }
    }
  });
}

function escapeHtml(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
