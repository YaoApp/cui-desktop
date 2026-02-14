import { route, initRouter } from "./lib/router";
import { renderServers } from "./pages/servers";
import { renderSettings } from "./pages/settings";
import { renderApp } from "./pages/app";
import "./lib/i18n"; // Ensure applyTheme() runs on startup

// Routes
route("/", renderServers);
route("/servers", renderServers);
route("/settings", renderSettings);
route("/app", renderApp);

// Start
document.addEventListener("DOMContentLoaded", () => {
  initRouter();
});
