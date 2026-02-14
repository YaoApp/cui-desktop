import { getAppConf, getProxyStatus, clearCookies } from "../lib/api";
import { clearAll } from "../lib/store";
import { navigate } from "../lib/router";

/** Render the settings page */
export async function renderSettings(): Promise<void> {
  const app = document.getElementById("app")!;

  let appName = "Yao CUI Desktop";
  try {
    const c = await getAppConf();
    appName = c.name;
  } catch { /* ignore */ }

  let proxy = { running: false, port: 19840, server_url: "", token: "", auth_mode: "" };
  try {
    proxy = await getProxyStatus();
  } catch { /* ignore */ }

  app.innerHTML = `
    <div class="settings-container fade-in">
      <div class="settings-header">
        <h1>Settings</h1>
        <button class="btn-ghost btn-sm" id="back-btn">&larr; Back</button>
      </div>

      <div class="settings-section">
        <h2>Proxy</h2>
        <div class="settings-item">
          <span class="label">Status</span>
          <span class="status-badge ${proxy.running ? "online" : "offline"}">
            ${proxy.running ? "Running" : "Stopped"}
          </span>
        </div>
        ${proxy.running ? `
        <div class="settings-item">
          <span class="label">Local</span>
          <span class="value">http://127.0.0.1:${proxy.port}</span>
        </div>
        <div class="settings-item">
          <span class="label">Remote</span>
          <span class="value">${escapeHtml(proxy.server_url)}</span>
        </div>` : ""}
      </div>

      <div class="settings-section">
        <h2>About</h2>
        <div class="settings-item">
          <span class="label">Application</span>
          <span class="value">${escapeHtml(appName)}</span>
        </div>
        <div class="settings-item">
          <span class="label">Version</span>
          <span class="value">v0.1.0</span>
        </div>
      </div>

      <div class="settings-section">
        <h2>Data</h2>
        <div class="settings-actions">
          <button class="btn-ghost btn-sm" id="clear-cookies-btn">Clear Cookies</button>
          <button class="btn-danger-solid" id="clear-all-btn">Clear All Data</button>
        </div>
      </div>
    </div>
  `;

  document.getElementById("back-btn")!.addEventListener("click", () => {
    navigate(proxy.running ? "/app" : "/");
  });

  document.getElementById("clear-cookies-btn")!.addEventListener("click", async () => {
    if (confirm("Clear all stored cookies? You may need to log in again.")) {
      await clearCookies();
      alert("Cookies cleared.");
    }
  });

  document.getElementById("clear-all-btn")!.addEventListener("click", async () => {
    if (confirm("Clear all data including server configurations and cookies?")) {
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
