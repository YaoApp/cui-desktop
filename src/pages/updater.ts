import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { t, getTheme } from "../lib/i18n";
import { getAppConf } from "../lib/api";

/** Render the updater page (standalone updater window) */
export async function renderUpdater(): Promise<void> {
  const app = document.getElementById("app")!;

  let appVersion = "0.0.0";
  let appName = "Yao Agents";
  try {
    appVersion = await getVersion();
  } catch { /* ignore */ }
  try {
    const conf = await getAppConf();
    appName = conf.name || appName;
  } catch { /* ignore */ }

  const theme = getTheme();
  document.documentElement.setAttribute("data-theme", theme);

  app.innerHTML = `
    <div class="updater-container fade-in">
      <div class="updater-header">
        <img src="/icon.png" alt="" class="updater-icon" />
        <div class="updater-header-text">
          <h1 class="updater-title">${esc(appName)}</h1>
          <p class="updater-subtitle">${esc(t("updater.title"))}</p>
        </div>
      </div>

      <div class="updater-info">
        <div class="updater-row">
          <span class="updater-label">${esc(t("updater.current_version"))}</span>
          <span class="updater-value">v${esc(appVersion)}</span>
        </div>
        <div class="updater-row" id="new-version-row" style="display:none">
          <span class="updater-label">${esc(t("updater.new_version"))}</span>
          <span class="updater-value" id="new-version-text"></span>
        </div>
      </div>

      <div id="release-notes-section" style="display:none">
        <div class="updater-notes-label">${esc(t("updater.release_notes"))}</div>
        <div class="updater-notes" id="release-notes"></div>
      </div>

      <div id="progress-section" style="display:none">
        <div class="updater-progress-text" id="progress-text"></div>
        <div class="updater-progress-bar">
          <div class="updater-progress-fill" id="progress-fill"></div>
        </div>
      </div>

      <div id="status-text" class="updater-status">${esc(t("updater.checking"))}</div>

      <div class="updater-actions">
        <button class="btn-main" id="action-btn" style="display:none"></button>
        <button class="btn-ghost btn-sm" id="close-btn">${esc(t("updater.close"))}</button>
      </div>
    </div>
  `;

  const statusEl = document.getElementById("status-text")!;
  const actionBtn = document.getElementById("action-btn") as HTMLButtonElement;
  const closeBtn = document.getElementById("close-btn") as HTMLButtonElement;

  closeBtn.addEventListener("click", async () => {
    try { await getCurrentWindow().close(); } catch { window.close(); }
  });

  try {
    const update = await check();
    if (update) {
      statusEl.style.display = "none";

      // Show new version
      document.getElementById("new-version-row")!.style.display = "";
      document.getElementById("new-version-text")!.textContent = `v${update.version}`;

      // Show release notes if available
      if (update.body) {
        document.getElementById("release-notes-section")!.style.display = "";
        document.getElementById("release-notes")!.textContent = update.body;
      }

      // Show download button
      actionBtn.style.display = "";
      actionBtn.textContent = t("updater.download_install");
      actionBtn.addEventListener("click", async () => {
        actionBtn.disabled = true;
        actionBtn.textContent = t("updater.downloading");

        const progressSection = document.getElementById("progress-section")!;
        const progressText = document.getElementById("progress-text")!;
        const progressFill = document.getElementById("progress-fill")!;
        progressSection.style.display = "";

        let downloaded = 0;
        let total = 0;

        try {
          await update.downloadAndInstall((event) => {
            if (event.event === "Started") {
              total = event.data.contentLength ?? 0;
              progressText.textContent = total > 0 ? `0 / ${formatBytes(total)}` : t("updater.downloading");
            } else if (event.event === "Progress") {
              downloaded += event.data.chunkLength;
              if (total > 0) {
                const pct = Math.min(100, Math.round(downloaded * 100 / total));
                progressFill.style.width = `${pct}%`;
                progressText.textContent = `${formatBytes(downloaded)} / ${formatBytes(total)} · ${pct}%`;
              } else {
                progressText.textContent = formatBytes(downloaded);
              }
            } else if (event.event === "Finished") {
              progressFill.style.width = "100%";
              progressText.textContent = "";
              actionBtn.textContent = t("updater.restart_now");
              actionBtn.disabled = false;
              actionBtn.onclick = async () => {
                await relaunch();
              };
            }
          });
        } catch (err: any) {
          progressSection.style.display = "none";
          statusEl.style.display = "";
          statusEl.textContent = `${t("updater.check_failed")}: ${err?.message ?? err}`;
          actionBtn.textContent = t("updater.download_install");
          actionBtn.disabled = false;
        }
      });
    } else {
      statusEl.textContent = t("updater.up_to_date");
    }
  } catch (err: any) {
    statusEl.textContent = `${t("updater.check_failed")}: ${err?.message ?? err}`;
  }
}

function formatBytes(b: number): string {
  if (b < 1024) return b + " B";
  if (b < 1048576) return (b / 1024).toFixed(1) + " KB";
  return (b / 1048576).toFixed(1) + " MB";
}

function esc(s: string): string {
  return (s ?? "").replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
