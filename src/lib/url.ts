/**
 * Extract origin (scheme + host + port) from user input,
 * discarding path / query / hash.
 *
 * Auto-scheme rules (when no protocol is specified):
 *   - IP address or localhost or explicit port → http://
 *   - Domain name without port → https://
 */
export function normalizeServerUrl(input: string): string {
  let s = input.trim();
  if (!s) return "";

  const hasScheme = /^https?:\/\//i.test(s);

  if (!hasScheme) {
    const isIP = /^(\d{1,3}\.){3}\d{1,3}/.test(s);
    const isLocalhost = /^localhost([\/:?#]|$)/i.test(s);
    const isIPv6 = s.startsWith("[");
    const hasPort = /^[^\/:]+:\d+/.test(s);
    const scheme = (isIP || isLocalhost || isIPv6 || hasPort) ? "http://" : "https://";
    s = scheme + s;
  }

  try {
    const url = new URL(s);
    return url.origin;
  } catch {
    return input.trim();
  }
}
