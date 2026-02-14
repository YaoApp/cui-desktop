import { getAppConf, checkServer, startProxy, type AppConf } from "../lib/api";
import { getSettings, saveServer, removeServer, type ServerEntry } from "../lib/store";
import { navigate } from "../lib/router";
import { t } from "../lib/i18n";

const DEFAULT_CONF: AppConf = {
  name: "Yao CUI Desktop",
  logo: "",
  port: 15099,
  theme: { primaryColor: "#3373fc" },
  updater: { active: false, endpoints: [], pubkey: "" },
  servers: [],
};

/** Render the server selection page */
export async function renderServers(): Promise<void> {
  const app = document.getElementById("app")!;

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

  // Merge config presets + user servers
  const allServers = mergeServers(settings.servers, conf.servers);

  // Apply theme
  const primary = conf.theme?.primaryColor || "#3373fc";
  document.documentElement.style.setProperty("--color-main", primary);

  // Logo: use config.logo if set, otherwise default Yao icon
  const logoSrc = conf.logo || "/icon.png";

  app.innerHTML = `
    <div class="page-servers">
      <div class="servers-panel fade-in">
        <div class="servers-brand">
          <img src="${esc(logoSrc)}" alt="" class="brand-logo" />
          <h1 class="brand-name">${escapeHtml(conf.name)}</h1>
        </div>

        <div id="alert-area"></div>

        <div class="server-list" id="server-list">
          ${allServers.length > 0
            ? allServers.map(s => serverItem(s, settings.activeServerUrl)).join("")
            : `<div class="server-list-empty">${escapeHtml(t("app.no_servers"))}</div>`
          }
        </div>

        <div class="server-add-section">
          <div id="add-toggle">
            <button class="btn-text" id="show-add-btn">${escapeHtml(t("app.add_server"))}</button>
          </div>
          <div id="add-form" style="display:none">
            <div class="add-form-fields">
              <input type="text" id="add-url" placeholder="${esc(t("app.server_url_placeholder"))}" />
              <input type="text" id="add-label" placeholder="${esc(t("app.server_name_placeholder"))}" />
            </div>
            <div class="add-form-actions">
              <button class="btn-main btn-sm" id="add-ok">${escapeHtml(t("app.add"))}</button>
              <button class="btn-ghost btn-sm" id="add-cancel">${escapeHtml(t("app.cancel"))}</button>
            </div>
          </div>
        </div>

        <div class="servers-bottom">
          <a href="#" id="goto-settings">${escapeHtml(t("app.settings"))}</a>
        </div>
      </div>
    </div>
  `;

  bind(conf);
}

function serverItem(s: MergedServer, activeUrl: string): string {
  const active = s.url === activeUrl ? " active" : "";
  const badge = s.source === "config" ? `<span class="badge-preset">${escapeHtml(t("app.default"))}</span>` : "";

  return `
    <div class="server-item${active}">
      <div class="server-info">
        <div class="server-name">${escapeHtml(s.label || s.url)} ${badge}</div>
        <div class="server-url">${escapeHtml(s.url)}</div>
      </div>
      <div class="server-actions">
        <button class="btn-main btn-sm connect-btn" data-url="${esc(s.url)}" data-label="${esc(s.label)}">${escapeHtml(t("app.connect"))}</button>
        ${s.source === "user" ? `<button class="btn-icon remove-btn" data-url="${esc(s.url)}" title="${esc(t("app.remove"))}">&times;</button>` : ""}
      </div>
    </div>
  `;
}

function bind(conf: AppConf) {
  const alertArea = document.getElementById("alert-area")!;

  // Connect
  document.querySelectorAll(".connect-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      const el = btn as HTMLElement;
      doConnect(el.dataset.url!, el.dataset.label || "", alertArea);
    });
  });

  // Remove
  document.querySelectorAll(".remove-btn").forEach(btn => {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      await removeServer((btn as HTMLElement).dataset.url!);
      renderServers();
    });
  });

  // Add toggle
  document.getElementById("show-add-btn")!.addEventListener("click", () => {
    document.getElementById("add-toggle")!.style.display = "none";
    document.getElementById("add-form")!.style.display = "block";
    (document.getElementById("add-url") as HTMLInputElement).focus();
  });

  document.getElementById("add-cancel")!.addEventListener("click", () => {
    document.getElementById("add-toggle")!.style.display = "block";
    document.getElementById("add-form")!.style.display = "none";
  });

  document.getElementById("add-ok")!.addEventListener("click", async () => {
    const url = (document.getElementById("add-url") as HTMLInputElement).value.trim();
    if (!url) { showAlert(alertArea, "error", t("app.enter_url")); return; }
    const label = (document.getElementById("add-label") as HTMLInputElement).value.trim() || url.replace(/^https?:\/\//, "");
    await saveServer({ url, label, lastConnected: 0 });
    renderServers();
  });

  // Settings
  document.getElementById("goto-settings")!.addEventListener("click", (e) => {
    e.preventDefault();
    navigate("/settings");
  });
}

async function doConnect(url: string, label: string, alertArea: HTMLElement) {
  document.querySelectorAll(".connect-btn").forEach(b => (b as HTMLButtonElement).disabled = true);
  showAlert(alertArea, "info", t("app.connecting"));

  try {
    let name = label || url.replace(/^https?:\/\//, "");
    try {
      const info = await checkServer(url);
      if (info.name) name = info.name;
    } catch { /* older server */ }

    await saveServer({ url, label: name, lastConnected: Date.now() });

    showAlert(alertArea, "info", t("app.starting_proxy"));
    await startProxy(url, "", "openapi");

    showAlert(alertArea, "success", t("app.connected"));
    setTimeout(() => navigate("/app"), 300);
  } catch (err: any) {
    showAlert(alertArea, "error", `${t("app.connection_failed")}${typeof err === "string" ? err : err?.message ?? String(err)}`);
  } finally {
    document.querySelectorAll(".connect-btn").forEach(b => (b as HTMLButtonElement).disabled = false);
  }
}

// ---- helpers ----

interface MergedServer { url: string; label: string; lastConnected: number; source: "user" | "config" }

function mergeServers(user: ServerEntry[], preset: AppConf["servers"]): MergedServer[] {
  const map = new Map<string, MergedServer>();
  for (const s of preset) map.set(s.url, { url: s.url, label: s.label, lastConnected: 0, source: "config" });
  for (const s of user)   map.set(s.url, { url: s.url, label: s.label, lastConnected: s.lastConnected, source: "user" });
  return Array.from(map.values()).sort((a, b) => (b.lastConnected || 0) - (a.lastConnected || 0));
}

function showAlert(el: HTMLElement, type: "error" | "success" | "info", msg: string) {
  el.innerHTML = `<div class="alert alert-${type}">${escapeHtml(msg)}</div>`;
}

function escapeHtml(s: string): string { const d = document.createElement("div"); d.textContent = s ?? ""; return d.innerHTML; }
function esc(s: string): string { return (s ?? "").replace(/&/g,"&amp;").replace(/"/g,"&quot;").replace(/</g,"&lt;").replace(/>/g,"&gt;"); }
