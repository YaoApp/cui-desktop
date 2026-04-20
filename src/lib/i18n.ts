/**
 * i18n + theme module
 *
 * Priority: user override (localStorage) > system default
 * Persisted keys: "cui_lang" ("zh"|"en"), "cui_theme" ("light"|"dark")
 *
 * Cross-window sync: setTheme/setLang call the Rust `sync_preferences` command
 * which uses webview.eval() to inject JS into ALL windows — including CUI proxy
 * pages that don't have our SPA JS loaded.
 */

// ===== Translations =====

const zhCN: Record<string, string> = {
  "app.no_servers": "尚未添加服务器",
  "app.add_server": "+ 添加服务器",
  "app.server_url_placeholder": "服务器地址，如 https://app.example.com",
  "app.server_name_placeholder": "名称（可选）",
  "app.add": "添加",
  "app.cancel": "取消",
  "app.connect": "连接",
  "app.connecting": "连接中…",
  "app.starting_proxy": "正在启动代理…",
  "app.connected": "已连接！正在加载 CUI…",
  "app.connection_failed": "连接失败：",
  "app.enter_url": "请输入服务器地址",
  "app.default": "默认",
  "app.remove": "移除",
  "app.settings": "设置",
  "app.loading_cui": "正在加载 CUI…",
  "app.connected_status": "已连接",
  "app.resume": "恢复",

  "settings.title": "设置",
  "settings.back": "← 返回",
  "settings.proxy": "代理",
  "settings.status": "状态",
  "settings.running": "运行中",
  "settings.stopped": "已停止",
  "settings.local": "本地地址",
  "settings.remote": "远程地址",
  "settings.about": "关于",
  "settings.application": "应用",
  "settings.version": "版本",
  "settings.data": "数据",
  "settings.clear_cookies": "清除 Cookie",
  "settings.clear_all": "清除所有数据",
  "settings.confirm_cookies": "清除所有已存储的 Cookie？你可能需要重新登录。",
  "settings.cookies_cleared": "Cookie 已清除。",
  "settings.confirm_all": "清除所有数据，包括服务器配置和 Cookie？",

  "settings.appearance": "外观",
  "settings.language": "语言",
  "settings.theme": "主题",
  "settings.check_update": "检查更新",
  "settings.checking": "正在检查…",
  "settings.up_to_date": "已是最新版本",
  "settings.new_version": "发现新版本 v{version}",
  "settings.downloading": "正在下载更新…",
  "settings.restart_now": "立即重启",
  "settings.update_error": "检查更新失败",
  "settings.close": "关闭",
};

const enUS: Record<string, string> = {
  "app.no_servers": "No servers yet",
  "app.add_server": "+ Add server",
  "app.server_url_placeholder": "Server URL, e.g. https://app.example.com",
  "app.server_name_placeholder": "Name (optional)",
  "app.add": "Add",
  "app.cancel": "Cancel",
  "app.connect": "Connect",
  "app.connecting": "Connecting…",
  "app.starting_proxy": "Starting proxy…",
  "app.connected": "Connected! Loading CUI…",
  "app.connection_failed": "Connection failed: ",
  "app.enter_url": "Please enter a server URL.",
  "app.default": "Default",
  "app.remove": "Remove",
  "app.settings": "Settings",
  "app.loading_cui": "Loading CUI…",
  "app.connected_status": "Connected",
  "app.resume": "Resume",

  "settings.title": "Settings",
  "settings.back": "← Back",
  "settings.proxy": "Proxy",
  "settings.status": "Status",
  "settings.running": "Running",
  "settings.stopped": "Stopped",
  "settings.local": "Local",
  "settings.remote": "Remote",
  "settings.about": "About",
  "settings.application": "Application",
  "settings.version": "Version",
  "settings.data": "Data",
  "settings.clear_cookies": "Clear Cookies",
  "settings.clear_all": "Clear All Data",
  "settings.confirm_cookies": "Clear all stored cookies? You may need to log in again.",
  "settings.cookies_cleared": "Cookies cleared.",
  "settings.confirm_all": "Clear all data including server configurations and cookies?",

  "settings.appearance": "Appearance",
  "settings.language": "Language",
  "settings.theme": "Theme",
  "settings.check_update": "Check for Update",
  "settings.checking": "Checking…",
  "settings.up_to_date": "You're up to date",
  "settings.new_version": "New version v{version} available",
  "settings.downloading": "Downloading update…",
  "settings.restart_now": "Restart Now",
  "settings.update_error": "Update check failed",
  "settings.close": "Close",
};

const translations: Record<string, Record<string, string>> = {
  zh: zhCN,
  en: enUS,
};

// ===== Language =====

const LANG_KEY = "cui_lang";

/** Get the current language: user override > system */
export function getLang(): string {
  const saved = localStorage.getItem(LANG_KEY);
  if (saved === "zh" || saved === "en") return saved;
  const sys = navigator.language || "en";
  return sys.startsWith("zh") ? "zh" : "en";
}

/** Set language override, persist, sync tray menu, and broadcast to all windows */
export function setLang(lang: string): void {
  localStorage.setItem(LANG_KEY, lang);
  import("./api").then((api) => {
    api.setUiLanguage(lang).catch(() => {});
    api.syncPreferences(getTheme(), lang).catch(() => {});
  }).catch(() => {});
}

/** Get locale string for CUI cookie (e.g. "zh-cn", "en-us") */
export function getLocaleForCUI(): string {
  return getLang() === "zh" ? "zh-cn" : "en-us";
}

/** Translate a key */
export function t(key: string): string {
  const lang = getLang();
  return translations[lang]?.[key] ?? translations["en"]?.[key] ?? key;
}

// ===== Theme =====

const THEME_KEY = "cui_theme";

/** Get the current theme: user override > system */
export function getTheme(): string {
  const saved = localStorage.getItem(THEME_KEY);
  if (saved === "light" || saved === "dark") return saved;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** Set theme override, persist, and broadcast to all windows */
export function setTheme(theme: string): void {
  localStorage.setItem(THEME_KEY, theme);
  applyTheme();
  import("./api").then((api) => {
    api.syncPreferences(theme, getLang()).catch(() => {});
  }).catch(() => {});
}

/** Apply theme to document (adds/removes data-theme attribute + sync title bar) */
export function applyTheme(): void {
  const theme = getTheme();
  document.documentElement.setAttribute("data-theme", theme);
  // Sync with Tauri window theme so the title bar follows the UI theme
  import("../lib/api").then(({ setWindowTheme }) => {
    setWindowTheme(theme).catch(() => {});
  }).catch(() => {});
}

/** Get theme value for CUI cookie: "dark" or "" (empty = light) */
export function getThemeForCUI(): string {
  return getTheme() === "dark" ? "dark" : "";
}

// Apply on load
applyTheme();

// Sync initial language to Rust (tray menu)
import("./api").then((api) => api.setUiLanguage(getLang())).catch(() => {});

// Cross-window sync is handled by the Rust `sync_preferences` command.
// It injects JS via webview.eval() into ALL windows, updating localStorage,
// data-theme, and dispatching "cui:theme-sync" / "cui:lang-sync" events.
// SPA pages (servers.ts, settings.ts) listen for these CustomEvents to re-render.
