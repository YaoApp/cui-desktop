import { route, initRouter } from "./lib/router";
import { renderLogin } from "./pages/login";
import { renderSettings } from "./pages/settings";
import { renderApp } from "./pages/app";

// Register routes
route("/", renderLogin);
route("/login", renderLogin);
route("/settings", renderSettings);
route("/app", renderApp);

// Start
document.addEventListener("DOMContentLoaded", () => {
  initRouter();
});
