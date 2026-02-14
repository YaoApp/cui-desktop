type RouteHandler = () => void | Promise<void>;

interface Route {
  path: string;
  handler: RouteHandler;
}

const routes: Route[] = [];

/** Register a route */
export function route(path: string, handler: RouteHandler): void {
  routes.push({ path, handler });
}

/** Navigate to a path */
export function navigate(path: string): void {
  window.history.pushState({}, "", path);
  resolveRoute();
}

/** Resolve the current route */
export function resolveRoute(): void {
  const path = window.location.pathname;
  for (const r of routes) {
    if (r.path === path) {
      r.handler();
      return;
    }
  }
  // Default route
  if (routes.length > 0) {
    navigate(routes[0].path);
  }
}

/** Initialize the router */
export function initRouter(): void {
  window.addEventListener("popstate", () => {
    resolveRoute();
  });
  resolveRoute();
}
