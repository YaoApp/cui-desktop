import { load } from "@tauri-apps/plugin-store";

/** A user-saved server entry */
export interface ServerEntry {
  url: string;
  label: string;
  lastConnected: number; // Unix timestamp ms
}

export interface AppSettings {
  servers: ServerEntry[];
  activeServerUrl: string;
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
  };
}

/** Save settings */
export async function saveSettings(settings: AppSettings): Promise<void> {
  const store = await getStore();
  await store.set("settings", settings);
  await store.save();
}

/** Save or update a server entry */
export async function saveServer(entry: ServerEntry): Promise<void> {
  const settings = await getSettings();
  const idx = settings.servers.findIndex((s) => s.url === entry.url);
  if (idx >= 0) {
    settings.servers[idx] = entry;
  } else {
    settings.servers.push(entry);
  }
  settings.activeServerUrl = entry.url;
  await saveSettings(settings);
}

/** Get the active server URL */
export async function getActiveServerUrl(): Promise<string> {
  const settings = await getSettings();
  return settings.activeServerUrl;
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
