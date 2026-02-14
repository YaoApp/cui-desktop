import { invoke } from "@tauri-apps/api/core";

export interface WellKnownInfo {
  name: string | null;
  version: string | null;
  openapi: string | null;
  dashboard: string | null;
  issuer_url: string | null;
}

export interface LoginResult {
  success: boolean;
  message: string;
  token: string;
  auth_mode: string;
}

export interface ProxyStatus {
  running: boolean;
  port: number;
  server_url: string;
  token: string;
  auth_mode: string;
}

/** Check remote server availability */
export async function checkServer(serverUrl: string): Promise<WellKnownInfo> {
  return invoke<WellKnownInfo>("check_server", { serverUrl });
}

/** OpenAPI login */
export async function loginOpenapi(
  serverUrl: string,
  username: string,
  password: string
): Promise<LoginResult> {
  return invoke<LoginResult>("login_openapi", { serverUrl, username, password });
}

/** Legacy login */
export async function loginLegacy(
  serverUrl: string,
  username: string,
  password: string
): Promise<LoginResult> {
  return invoke<LoginResult>("login_legacy", { serverUrl, username, password });
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
