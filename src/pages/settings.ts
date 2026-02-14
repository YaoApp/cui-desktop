import { getAppConf, getProxyStatus, clearCookies } from "../lib/api";
import { clearAll } from "../lib/store";
import { navigate } from "../lib/router";
import { t } from "../lib/i18n";

/** Render the settings page */
export async function renderSettings(): Promise<void> {
  const app = document.getElementById("app")!;

  let appName = "Yao Desktop";
  try {
    const c = await getAppConf();
    appName = c.name;
  } catch { /* ignore */ }

  let proxy = { running: false, port: 15099, server_url: "", token: "", auth_mode: "" };
  try {
    proxy = await getProxyStatus();
  } catch { /* ignore */ }

  app.innerHTML = `
    <div class="settings-container fade-in">
      <div class="settings-header">
        <h1>${escapeHtml(t("settings.title"))}</h1>
        <button class="btn-ghost btn-sm" id="back-btn">${escapeHtml(t("settings.back"))}</button>
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
        <h2>${escapeHtml(t("settings.about"))}</h2>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.application"))}</span>
          <span class="value">${escapeHtml(appName)}</span>
        </div>
        <div class="settings-item">
          <span class="label">${escapeHtml(t("settings.version"))}</span>
          <span class="value">v0.1.0</span>
        </div>
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

  document.getElementById("back-btn")!.addEventListener("click", () => {
    navigate(proxy.running ? "/app" : "/");
  });

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
      navigate("/");
    }
  });
}

function escapeHtml(s: string): string {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}
