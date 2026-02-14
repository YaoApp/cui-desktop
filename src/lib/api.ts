import { invoke } from "@tauri-apps/api/core";

export interface AppConf {
  name: string;
  logo: string;
  port: number;
  theme: {
    primaryColor: string;
  };
  updater: {
    active: boolean;
    endpoints: string[];
    pubkey: string;
  };
  servers: Array<{
    url: string;
    label: string;
  }>;
}

export interface WellKnownInfo {
  name: string | null;
  version: string | null;
  openapi: string | null;
  dashboard: string | null;
  issuer_url: string | null;
}

export interface ProxyStatus {
  running: boolean;
  port: number;
  server_url: string;
  token: string;
  auth_mode: string;
}

/** Get developer app config (config.json) */
export async function getAppConf(): Promise<AppConf> {
  return invoke<AppConf>("get_app_conf");
}

/** Check remote server availability */
export async function checkServer(serverUrl: string): Promise<WellKnownInfo> {
  return invoke<WellKnownInfo>("check_server", { serverUrl });
}

/** Start the local proxy server */
export async function startProxy(
  serverUrl: string,
  token: string,
  authMode: string
): Promise<number> {
  return invoke<number>("start_proxy", { serverUrl, token, authMode });
}

/** Get current proxy status */
export async function getProxyStatus(): Promise<ProxyStatus> {
  return invoke<ProxyStatus>("get_proxy_status");
}

/** Update the proxy auth token */
export async function updateProxyToken(token: string): Promise<void> {
  return invoke<void>("update_proxy_token", { token });
}

/** Clear all stored cookies */
export async function clearCookies(): Promise<void> {
  return invoke<void>("clear_cookies");
}
