import { checkServer, startProxy } from "../lib/api";
import { saveServer, getActiveServer, getSettings, type ServerEntry } from "../lib/store";
import { navigate } from "../lib/router";

const DEFAULT_SERVER = "http://127.0.0.1:5099";

/** Render the login / connect page */
export async function renderLogin(): Promise<void> {
  const app = document.getElementById("app")!;

  // Check for existing server config → auto-connect
  let savedUrl = "";
  try {
    const active = await getActiveServer();
    if (active && active.url) {
      savedUrl = active.url;
      try {
        await startProxy(active.url, active.token || "", active.authMode || "openapi");
        navigate("/app");
        return;
      } catch {
        // Proxy failed to start (e.g. port in use), show page
      }
    }
  } catch {
    // Store may not be initialized
  }

  app.innerHTML = `
    <div class="login-container">
      <div class="login-card fade-in">
        <div class="login-header">
          <div class="logo">Yao CUI Desktop</div>
          <div class="subtitle">Connect to a remote Yao App Engine server</div>
        </div>

        <div id="login-alert"></div>

        <form id="connect-form" autocomplete="on">
          <div class="form-group">
            <label for="server-url">Server URL</label>
            <input
              type="text"
              id="server-url"
              placeholder="http://127.0.0.1:5099"
              autocomplete="url"
            />
            <div class="input-hint">Enter the full Yao server address. CUI will handle authentication.</div>
          </div>

          <button type="submit" class="btn btn-primary" id="connect-btn">
            Connect
          </button>
        </form>

        <div id="saved-servers"></div>

        <div style="text-align:center;margin-top:20px">
          <a href="#" id="goto-settings" style="font-size:12px;color:var(--color-text-muted);text-decoration:none">
            Manage server configurations
          </a>
        </div>
      </div>
    </div>
  `;

  // Pre-fill server URL
  const urlInput = document.getElementById("server-url") as HTMLInputElement;
  urlInput.value = savedUrl || DEFAULT_SERVER;

  // Render saved server list
  await renderSavedServers();

  // Bind events
  const form = document.getElementById("connect-form") as HTMLFormElement;
  const alertEl = document.getElementById("login-alert")!;
  const connectBtn = document.getElementById("connect-btn") as HTMLButtonElement;

  document.getElementById("goto-settings")!.addEventListener("click", (e) => {
    e.preventDefault();
    navigate("/settings");
  });

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    let serverUrl = urlInput.value.trim();
    if (!serverUrl) {
      serverUrl = DEFAULT_SERVER;
      urlInput.value = serverUrl;
    }
    await doConnect(serverUrl, alertEl, connectBtn);
  });
}

/** Connect to a server */
async function doConnect(
  serverUrl: string,
  alertEl: HTMLElement,
  connectBtn: HTMLButtonElement,
): Promise<void> {
  alertEl.innerHTML = "";
  connectBtn.disabled = true;
  connectBtn.innerHTML = '<span class="spinner"></span> Connecting...';

  try {
    // Step 1: Verify server
    showAlert(alertEl, "info", "Connecting to server...");
    let serverName = serverUrl.replace(/^https?:\/\//, "");

    try {
      const info = await checkServer(serverUrl);
      serverName = info.name ?? serverName;
      showAlert(alertEl, "info", `Server: ${serverName} v${info.version ?? "?"}`);
    } catch {
      // .well-known/yao not available — may be an older server
      showAlert(alertEl, "info", "Connecting...");
    }

    // Step 2: Save server config
    const entry: ServerEntry = {
      url: serverUrl,
      name: serverName,
      token: "",  // CUI handles authentication
      authMode: "openapi",
      tokenExpiresAt: 0,
      lastConnected: Date.now(),
    };
    await saveServer(entry);

    // Step 3: Start proxy (no token — CUI authenticates via cookies managed by proxy jar)
    showAlert(alertEl, "info", "Starting local proxy...");
    await startProxy(serverUrl, "", "openapi");

    showAlert(alertEl, "success", "Connected! Loading CUI...");

    // Step 4: Navigate to CUI
    setTimeout(() => navigate("/app"), 300);

  } catch (err: any) {
    const msg = typeof err === "string" ? err : (err?.message ?? String(err));
    showAlert(alertEl, "error", `Connection failed: ${msg}`);
  } finally {
    connectBtn.disabled = false;
    connectBtn.innerHTML = "Connect";
  }
}

/** Render recently connected servers for quick access */
async function renderSavedServers(): Promise<void> {
  const container = document.getElementById("saved-servers");
  if (!container) return;

  try {
    const settings = await getSettings();
    if (settings.servers.length === 0) return;

    container.innerHTML = `
      <div style="margin-top:20px;padding-top:16px;border-top:1px solid var(--color-border)">
        <div style="font-size:12px;color:var(--color-text-muted);margin-bottom:8px">Recent connections</div>
        ${settings.servers
          .sort((a, b) => b.lastConnected - a.lastConnected)
          .slice(0, 5)
          .map(s => `
            <button class="btn btn-secondary btn-sm saved-server-btn" data-url="${escapeAttr(s.url)}" style="width:100%;margin-bottom:6px;justify-content:flex-start;font-family:var(--font-mono);font-size:12px">
              ${escapeHtml(s.name || s.url)}
            </button>
          `).join("")}
      </div>
    `;

    container.querySelectorAll(".saved-server-btn").forEach(btn => {
      btn.addEventListener("click", () => {
        const url = (btn as HTMLElement).dataset.url!;
        (document.getElementById("server-url") as HTMLInputElement).value = url;
        const alertEl = document.getElementById("login-alert")!;
        const connectBtn = document.getElementById("connect-btn") as HTMLButtonElement;
        doConnect(url, alertEl, connectBtn);
      });
    });
  } catch {
    // ignore
  }
}

function showAlert(el: HTMLElement, type: "error" | "success" | "info", msg: string): void {
  el.innerHTML = `<div class="alert alert-${type}">${escapeHtml(msg)}</div>`;
}

function escapeHtml(str: string): string {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

function escapeAttr(str: string): string {
  return str.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
