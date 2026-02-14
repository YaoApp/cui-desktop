import { getProxyStatus, setPreferenceCookies } from "../lib/api";
import { navigate } from "../lib/router";
import { t, getLocaleForCUI } from "../lib/i18n";

const CUI_PATH = "/__yao_admin_root/";

/** Navigate the WebView to CUI served by the local proxy */
export async function renderApp(): Promise<void> {
  const app = document.getElementById("app")!;

  let port = 15099;
  let proxyRunning = false;
  try {
    const status = await getProxyStatus();
    proxyRunning = status.running;
    port = status.port;
  } catch { /* ignore */ }

  if (!proxyRunning) {
    navigate("/");
    return;
  }

  app.innerHTML = `
    <div class="app-loading fade-in">
      <div class="spinner spinner-dark spinner-lg"></div>
      <p>${escapeHtml(t("app.loading_cui"))}</p>
    </div>
  `;

  // Detect system locale and theme, store in proxy cookie jar.
  // The proxy will inject these as Set-Cookie when serving CUI pages,
  // so CUI JavaScript can read them immediately without a jarring switch.
  const locale = getLocaleForCUI();
  const theme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "";
  try {
    await setPreferenceCookies(locale, theme);
  } catch { /* ignore */ }

  setTimeout(() => {
    window.location.href = `http://127.0.0.1:${port}${CUI_PATH}`;
  }, 300);
}

function escapeHtml(s: string): string {
  const d = document.createElement("div");
  d.textContent = s ?? "";
  return d.innerHTML;
}
