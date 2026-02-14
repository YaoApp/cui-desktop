import { getProxyStatus } from "../lib/api";
import { getActiveServer } from "../lib/store";
import { navigate } from "../lib/router";

const PROXY_BASE = "http://127.0.0.1:19840";
const CUI_PATH = "/__yao_admin_root/";

/** Render the main page (load CUI via proxy) */
export async function renderApp(): Promise<void> {
  const app = document.getElementById("app")!;

  // Check proxy status
  let proxyRunning = false;
  try {
    const status = await getProxyStatus();
    proxyRunning = status.running;
  } catch {
    // ignore
  }

  if (!proxyRunning) {
    // Proxy not running — redirect to login to start it
    const server = await getActiveServer();
    if (!server || !server.url) {
      navigate("/login");
      return;
    }
    navigate("/login");
    return;
  }

  // Show loading state
  app.innerHTML = `
    <div class="app-loading fade-in">
      <div class="spinner spinner-dark spinner-lg"></div>
      <p>Loading CUI...</p>
    </div>
  `;

  // Navigate the WebView to the proxy server serving CUI
  setTimeout(() => {
    window.location.href = `${PROXY_BASE}${CUI_PATH}`;
  }, 300);
}
