import { load } from "@tauri-apps/plugin-store";

export interface ServerEntry {
  url: string;
  name: string;
  token: string;
  authMode: "openapi" | "legacy";
  tokenExpiresAt: number; // Unix timestamp ms
  lastConnected: number;
}

export interface AppSettings {
  servers: ServerEntry[];
  activeServerUrl: string;
  rememberMe: boolean;
}

const STORE_NAME = "cui-desktop-store.json";
let storeInstance: Awaited<ReturnType<typeof load>> | null = null;

async function getStore() {
  if (!storeInstance) {
    storeInstance = await load(STORE_NAME, {
      defaults: {},
      autoSave: true,
    });
  }
  return storeInstance;
}

/** Get all settings */
export async function getSettings(): Promise<AppSettings> {
  const store = await getStore();
  const settings = await store.get<AppSettings>("settings");
  return settings ?? {
    servers: [],
    activeServerUrl: "",
    rememberMe: true,
  };
}

/** Save settings */
export async function saveSettings(settings: AppSettings): Promise<void> {
  const store = await getStore();
  await store.set("settings", settings);
  await store.save();
}

/** Save a server entry (called after successful login) */
export async function saveServer(server: ServerEntry): Promise<void> {
  const settings = await getSettings();
  const idx = settings.servers.findIndex((s) => s.url === server.url);
  if (idx >= 0) {
    settings.servers[idx] = server;
  } else {
    settings.servers.push(server);
  }
  settings.activeServerUrl = server.url;
  await saveSettings(settings);
}

/** Get the active server entry */
export async function getActiveServer(): Promise<ServerEntry | null> {
  const settings = await getSettings();
  if (!settings.activeServerUrl) return null;
  return settings.servers.find((s) => s.url === settings.activeServerUrl) ?? null;
}

/** Remove a server entry */
export async function removeServer(url: string): Promise<void> {
  const settings = await getSettings();
  settings.servers = settings.servers.filter((s) => s.url !== url);
  if (settings.activeServerUrl === url) {
    settings.activeServerUrl = settings.servers[0]?.url ?? "";
  }
  await saveSettings(settings);
}

/** Clear all stored data */
export async function clearAll(): Promise<void> {
  const store = await getStore();
  await store.clear();
  await store.save();
}
