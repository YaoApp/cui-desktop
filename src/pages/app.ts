import { getProxyStatus } from "../lib/api";
import { navigate } from "../lib/router";

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
      <p>Loading CUI...</p>
    </div>
  `;

  setTimeout(() => {
    window.location.href = `http://127.0.0.1:${port}${CUI_PATH}`;
  }, 300);
}
