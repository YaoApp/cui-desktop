import { getProxyStatus, setPreferenceCookies } from "../lib/api";
import { navigate } from "../lib/router";
import { t, getLocaleForCUI, getThemeForCUI } from "../lib/i18n";

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

  // Store locale/theme in the proxy cookie jar (for server-side cookie injection),
  // then navigate to the bridge page which writes umi_locale into localStorage
  // on the proxy origin before redirecting to CUI. This guarantees CUI's umi
  // framework picks up the correct language on first load without a reload.
  const locale = getLocaleForCUI();
  const cuiLocale = locale === "zh-cn" ? "zh-CN" : "en-US";
  const theme = getThemeForCUI();
  try {
    await setPreferenceCookies(locale, theme);
  } catch { /* ignore */ }

  setTimeout(() => {
    const bridge = `http://127.0.0.1:${port}/__yao_bridge?locale=${encodeURIComponent(cuiLocale)}&theme=${encodeURIComponent(theme)}`;
    window.location.href = bridge;
  }, 300);
}

function escapeHtml(s: string): string {
  const d = document.createElement("div");
  d.textContent = s ?? "";
  return d.innerHTML;
}
