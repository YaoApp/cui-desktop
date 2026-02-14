/** Simple i18n module — detects system language and provides translations */

const zhCN: Record<string, string> = {
  // Server selection page
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

  // Settings page
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
};

const translations: Record<string, Record<string, string>> = {
  zh: zhCN,
  en: enUS,
};

/** Detect system language, returns "zh" or "en" */
export function detectLang(): string {
  const lang = navigator.language || "en";
  if (lang.startsWith("zh")) return "zh";
  return "en";
}

/** Get locale string for CUI cookie (e.g. "zh-cn", "en-us") */
export function getLocaleForCUI(): string {
  const lang = navigator.language || "en-US";
  if (lang.startsWith("zh")) return "zh-cn";
  return "en-us";
}

/** Translate a key */
export function t(key: string): string {
  const lang = detectLang();
  return translations[lang]?.[key] ?? translations["en"]?.[key] ?? key;
}
