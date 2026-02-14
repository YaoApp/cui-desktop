import { getSettings, removeServer, clearAll, type AppSettings } from "../lib/store";
import { getProxyStatus, clearCookies } from "../lib/api";
import { navigate } from "../lib/router";

/** Render the settings page */
export async function renderSettings(): Promise<void> {
  const app = document.getElementById("app")!;
  const settings = await getSettings();
  let proxyStatus = { running: false, port: 19840, server_url: "", token: "", auth_mode: "" };

  try {
    proxyStatus = await getProxyStatus();
  } catch {
    // Proxy may not be running
  }

  app.innerHTML = `
    <div class="settings-container fade-in">
      <div class="settings-header">
        <h1>Settings</h1>
        <button class="btn btn-secondary btn-sm" id="back-btn">&larr; Back</button>
      </div>

      <!-- Proxy Status -->
      <div class="settings-section">
        <h2>Proxy Status</h2>
        <div class="settings-item">
          <span class="label">Status</span>
          <span class="status-badge ${proxyStatus.running ? "online" : "offline"}">
            ${proxyStatus.running ? "Running" : "Stopped"}
          </span>
        </div>
        ${proxyStatus.running ? `
        <div class="settings-item">
          <span class="label">Local Address</span>
          <span class="value">http://127.0.0.1:${proxyStatus.port}</span>
        </div>
        <div class="settings-item">
          <span class="label">Remote Server</span>
          <span class="value">${escapeHtml(proxyStatus.server_url)}</span>
        </div>
        <div class="settings-item">
          <span class="label">Auth Mode</span>
          <span class="value">${proxyStatus.auth_mode}</span>
        </div>
        ` : ""}
      </div>

      <!-- Server List -->
      <div class="settings-section">
        <h2>Servers</h2>
        <div id="server-list">
          ${renderServerList(settings)}
        </div>
        <div class="settings-actions">
          <button class="btn btn-secondary btn-sm" id="add-server-btn">+ Add Server</button>
        </div>
      </div>

      <!-- About -->
      <div class="settings-section">
        <h2>About</h2>
        <div class="settings-item">
          <span class="label">Version</span>
          <span class="value">v0.1.0</span>
        </div>
        <div class="settings-item">
          <span class="label">Tauri</span>
          <span class="value">2.x</span>
        </div>
      </div>

      <!-- Data Management -->
      <div class="settings-section">
        <h2>Data Management</h2>
        <div class="settings-actions">
          <button class="btn btn-secondary btn-sm" id="clear-cookies-btn">Clear Cookies</button>
          <button class="btn btn-danger btn-sm" id="clear-all-btn">Clear All Data</button>
        </div>
      </div>
    </div>
  `;

  // Bind events
  document.getElementById("back-btn")!.addEventListener("click", () => {
    if (proxyStatus.running) {
      navigate("/app");
    } else {
      navigate("/login");
    }
  });

  document.getElementById("add-server-btn")!.addEventListener("click", () => {
    navigate("/login");
  });

  document.getElementById("clear-cookies-btn")!.addEventListener("click", async () => {
    if (confirm("Clear all stored cookies? You may need to log in again.")) {
      await clearCookies();
      alert("Cookies cleared.");
    }
  });

  document.getElementById("clear-all-btn")!.addEventListener("click", async () => {
    if (confirm("Clear all data? This will remove all server configurations and cookies.")) {
      await clearCookies();
      await clearAll();
      navigate("/login");
    }
  });

  // Remove server buttons
  document.querySelectorAll(".remove-server-btn").forEach((btn) => {
    btn.addEventListener("click", async (e) => {
      const url = (e.currentTarget as HTMLElement).dataset.url!;
      if (confirm(`Remove server ${url}?`)) {
        await removeServer(url);
        renderSettings();
      }
    });
  });
}

function renderServerList(settings: AppSettings): string {
  if (settings.servers.length === 0) {
    return `<div style="padding:16px 0;text-align:center;color:var(--color-text-muted);font-size:13px">
      No servers configured
    </div>`;
  }

  return settings.servers
    .map((s) => {
      const isActive = s.url === settings.activeServerUrl;
      const isExpired = s.tokenExpiresAt > 0 && s.tokenExpiresAt < Date.now();
      const expiresDate = s.tokenExpiresAt > 0
        ? new Date(s.tokenExpiresAt).toLocaleDateString("en-US")
        : "N/A";

      return `
        <div class="settings-item" style="flex-wrap:wrap;gap:8px">
          <div>
            <div style="font-weight:500;font-size:13px">
              ${isActive ? "&#9679; " : ""}${escapeHtml(s.name || s.url)}
            </div>
            <div style="font-size:12px;color:var(--color-text-muted);margin-top:2px">
              ${escapeHtml(s.url)} &middot; ${s.authMode}
              ${s.tokenExpiresAt > 0
                ? ` &middot; Token ${isExpired ? '<span style="color:var(--color-danger)">expired</span>' : `valid until ${expiresDate}`}`
                : ""}
            </div>
          </div>
          <button class="btn btn-secondary btn-sm remove-server-btn" data-url="${escapeHtml(s.url)}">Remove</button>
        </div>
      `;
    })
    .join("");
}

function escapeHtml(str: string): string {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}
