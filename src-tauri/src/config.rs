use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::{info, warn};

// ========== Global AppHandle ==========

/// Global Tauri AppHandle, set once during app setup.
/// Used by the proxy to call native window APIs (e.g. fullscreen).
static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

pub fn set_app_handle(handle: tauri::AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

pub fn get_app_handle() -> Option<&'static tauri::AppHandle> {
    APP_HANDLE.get()
}

/// Proxy runtime state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyState {
    pub running: bool,
    pub port: u16,
    pub server_url: String,
    pub token: String,
    pub auth_mode: String,
    /// Server admin root path from .well-known/yao (e.g. "/dashboard").
    /// Used to redirect /{dashboard}/* → /__yao_admin_root/* so that
    /// server-side redirects (login success_url etc.) land on local CUI.
    pub dashboard: String,
}

impl Default for ProxyState {
    fn default() -> Self {
        Self {
            running: false,
            port: 15099,
            server_url: String::new(),
            token: String::new(),
            auth_mode: String::from("openapi"),
            dashboard: String::new(),
        }
    }
}

/// Global proxy state
pub static PROXY_STATE: Lazy<RwLock<ProxyState>> = Lazy::new(|| {
    RwLock::new(ProxyState::default())
});

pub fn update_proxy_state(server_url: &str, token: &str, auth_mode: &str, dashboard: &str) {
    let mut state = PROXY_STATE.write();
    state.server_url = server_url.to_string();
    state.token = token.to_string();
    state.auth_mode = auth_mode.to_string();
    // Normalize: ensure leading slash, strip trailing slash
    let d = dashboard.trim().trim_end_matches('/');
    state.dashboard = if d.is_empty() {
        String::new()
    } else if d.starts_with('/') {
        d.to_string()
    } else {
        format!("/{}", d)
    };
}

pub fn set_proxy_running(running: bool) {
    let mut state = PROXY_STATE.write();
    state.running = running;
}

pub fn get_proxy_state() -> ProxyState {
    PROXY_STATE.read().clone()
}

// ========== Cookie Jar ==========

/// A single cookie entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntry {
    /// Original cookie name (including __Secure- prefix etc.)
    pub name: String,
    /// Cookie value
    pub value: String,
    /// Path scope
    pub path: String,
    /// Expiry time (Unix seconds), 0 = session cookie
    pub expires_at: u64,
    /// Whether the cookie is HttpOnly
    pub http_only: bool,
}

/// Cookie jar persistence file path
static COOKIE_FILE: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

/// Global cookie jar
pub static COOKIE_JAR: Lazy<RwLock<Vec<CookieEntry>>> = Lazy::new(|| RwLock::new(Vec::new()));

/// Set the cookie persistence file path
pub fn set_cookie_file(path: PathBuf) {
    *COOKIE_FILE.write() = Some(path);
}

/// Load cookies from file
pub fn load_cookies() {
    let path = COOKIE_FILE.read().clone();
    if let Some(path) = path {
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(data) => {
                    match serde_json::from_str::<Vec<CookieEntry>>(&data) {
                        Ok(cookies) => {
                            let count = cookies.len();
                            *COOKIE_JAR.write() = cookies;
                            info!("Loaded {} cookies from file", count);
                        }
                        Err(e) => warn!("Failed to parse cookie file: {}", e),
                    }
                }
                Err(e) => warn!("Failed to read cookie file: {}", e),
            }
        }
    }
    purge_expired();
}

/// Save cookies to file
fn save_cookies() {
    let path = COOKIE_FILE.read().clone();
    if let Some(path) = path {
        let jar = COOKIE_JAR.read();
        match serde_json::to_string_pretty(&*jar) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, data) {
                    warn!("Failed to write cookie file: {}", e);
                }
            }
            Err(e) => warn!("Failed to serialize cookies: {}", e),
        }
    }
}

/// Purge expired cookies
fn purge_expired() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut jar = COOKIE_JAR.write();
    jar.retain(|c| c.expires_at == 0 || c.expires_at > now);
}

/// Result of processing a Set-Cookie header
pub struct StoreCookieResult {
    /// Whether this cookie is "secure-only" (browser can't store it on HTTP)
    pub is_secure: bool,
    /// A sanitized Set-Cookie string for forwarding to the browser (None if secure-only)
    pub browser_cookie: Option<String>,
}

/// Parse a Set-Cookie header, store it in the jar, and return processing result.
///
/// "Secure" cookies (__Secure-*, __Host-*, or with Secure attribute) are stored
/// in the jar only. Non-secure cookies are stored in the jar AND a sanitized
/// version is returned for forwarding to the browser.
pub fn store_cookie(set_cookie: &str) -> StoreCookieResult {
    let parts: Vec<&str> = set_cookie.split(';').collect();
    if parts.is_empty() {
        return StoreCookieResult { is_secure: false, browser_cookie: None };
    }

    // Parse name=value
    let name_value = parts[0].trim();
    let (name, value) = match name_value.split_once('=') {
        Some((n, v)) => (n.trim().to_string(), v.trim().to_string()),
        None => return StoreCookieResult { is_secure: false, browser_cookie: None },
    };

    if name.is_empty() {
        return StoreCookieResult { is_secure: false, browser_cookie: None };
    }

    let mut path = "/".to_string();
    let mut expires_at: u64 = 0;
    let mut http_only = false;
    let mut has_secure_flag = false;
    let mut has_samesite_none = false;

    for part in &parts[1..] {
        let trimmed = part.trim();
        let lower = trimmed.to_lowercase();

        if lower.starts_with("path=") {
            path = trimmed[5..].trim().to_string();
        } else if lower.starts_with("max-age=") {
            if let Ok(secs) = trimmed[8..].trim().parse::<i64>() {
                if secs > 0 {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    expires_at = now + secs as u64;
                } else {
                    // max-age=0 means delete
                    remove_cookie(&name);
                    return StoreCookieResult { is_secure: false, browser_cookie: None };
                }
            }
        } else if lower == "httponly" {
            http_only = true;
        } else if lower == "secure" {
            has_secure_flag = true;
        } else if lower == "samesite=none" {
            has_samesite_none = true;
        }
    }

    // Determine if this cookie is "secure-only" (can't work on plain HTTP)
    let is_secure = has_secure_flag
        || name.starts_with("__Secure-")
        || name.starts_with("__Host-");

    let entry = CookieEntry {
        name: name.clone(),
        value: value.clone(),
        path: path.clone(),
        expires_at,
        http_only,
    };

    // Upsert into jar (always)
    let mut jar = COOKIE_JAR.write();
    if let Some(existing) = jar.iter_mut().find(|c| c.name == name) {
        *existing = entry;
    } else {
        jar.push(entry);
    }
    drop(jar);
    save_cookies();

    // Build sanitized Set-Cookie for browser (only if non-secure)
    let browser_cookie = if !is_secure {
        // Rebuild Set-Cookie: keep name=value, Path, Max-Age/Expires, HttpOnly
        // Remove: Domain, Secure, SameSite=None (requires Secure on HTTP)
        let mut parts_out = vec![format!("{}={}", name, value)];
        for part in &parts[1..] {
            let lower = part.trim().to_lowercase();
            // Skip attributes that don't work on HTTP localhost
            if lower == "secure"
                || lower.starts_with("domain=")
                || lower == "samesite=none"
            {
                continue;
            }
            parts_out.push(part.trim().to_string());
        }
        // If original had SameSite=None (which requires Secure), replace with Lax
        if has_samesite_none {
            parts_out.push("SameSite=Lax".to_string());
        }
        Some(parts_out.join("; "))
    } else {
        None
    };

    StoreCookieResult { is_secure, browser_cookie }
}

/// Remove a cookie by name
fn remove_cookie(name: &str) {
    let mut jar = COOKIE_JAR.write();
    jar.retain(|c| c.name != name);
    drop(jar);
    save_cookies();
}

/// Build a Cookie header value by merging jar cookies with browser cookies.
/// Jar cookies take precedence for names that exist in both.
///
/// `browser_cookie_header`: the raw Cookie header from the browser (may be empty)
/// `request_path`: used to filter jar cookies by path scope
pub fn get_merged_cookies(browser_cookie_header: &str, request_path: &str) -> String {
    purge_expired();

    // Parse browser cookies into a map
    let mut cookie_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    if !browser_cookie_header.is_empty() {
        for pair in browser_cookie_header.split(';') {
            let pair = pair.trim();
            if let Some((n, v)) = pair.split_once('=') {
                cookie_map.insert(n.trim().to_string(), v.trim().to_string());
            }
        }
    }

    // Merge jar cookies (jar wins on conflict, because it has secure cookies the browser can't store)
    let jar = COOKIE_JAR.read();
    for c in jar.iter() {
        if request_path.starts_with(&c.path) {
            cookie_map.insert(c.name.clone(), c.value.clone());
        }
    }

    cookie_map.into_iter()
        .map(|(n, v)| format!("{}={}", n, v))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Build a Cookie header value from jar only (legacy, kept for compatibility)
pub fn get_cookies_header(request_path: &str) -> String {
    purge_expired();
    let jar = COOKIE_JAR.read();
    jar.iter()
        .filter(|c| request_path.starts_with(&c.path))
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Clear all cookies
pub fn clear_cookies() {
    COOKIE_JAR.write().clear();
    save_cookies();
}

/// Get the number of stored cookies
pub fn cookie_count() -> usize {
    COOKIE_JAR.read().len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn reset_jar() {
        COOKIE_JAR.write().clear();
    }

    #[test]
    fn store_simple_cookie() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        let result = store_cookie("session=abc123; Path=/; HttpOnly");
        assert!(!result.is_secure);
        assert!(result.browser_cookie.is_some());
        let bc = result.browser_cookie.unwrap();
        assert!(bc.contains("session=abc123"));
        assert!(bc.contains("HttpOnly"));

        let jar = COOKIE_JAR.read();
        assert_eq!(jar.len(), 1);
        assert_eq!(jar[0].name, "session");
        assert_eq!(jar[0].value, "abc123");
        assert!(jar[0].http_only);
    }

    #[test]
    fn store_secure_cookie_not_forwarded() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        let result = store_cookie("__Secure-token=xyz; Path=/; Secure; HttpOnly");
        assert!(result.is_secure);
        assert!(result.browser_cookie.is_none());

        let jar = COOKIE_JAR.read();
        assert_eq!(jar.len(), 1);
        assert_eq!(jar[0].name, "__Secure-token");
    }

    #[test]
    fn store_cookie_with_secure_flag() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        let result = store_cookie("id=42; Path=/; Secure");
        assert!(result.is_secure);
        assert!(result.browser_cookie.is_none());
    }

    #[test]
    fn store_cookie_strips_domain_and_samesite_none() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        let result = store_cookie("tok=v; Path=/; Domain=example.com; SameSite=None; Secure");
        assert!(result.is_secure);

        reset_jar();
        let result = store_cookie("tok=v; Path=/; Domain=example.com; SameSite=None");
        assert!(!result.is_secure);
        let bc = result.browser_cookie.unwrap();
        assert!(!bc.contains("Domain="));
        assert!(!bc.contains("SameSite=None"));
        assert!(bc.contains("SameSite=Lax"));
    }

    #[test]
    fn store_cookie_upsert() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        store_cookie("key=old; Path=/");
        store_cookie("key=new; Path=/");
        let jar = COOKIE_JAR.read();
        assert_eq!(jar.len(), 1);
        assert_eq!(jar[0].value, "new");
    }

    #[test]
    fn store_cookie_max_age_zero_deletes() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        store_cookie("key=val; Path=/");
        assert_eq!(cookie_count(), 1);
        store_cookie("key=val; Path=/; Max-Age=0");
        assert_eq!(cookie_count(), 0);
    }

    #[test]
    fn store_cookie_empty_name_ignored() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        let result = store_cookie("=value; Path=/");
        assert!(!result.is_secure);
        assert!(result.browser_cookie.is_none());
        assert_eq!(cookie_count(), 0);
    }

    #[test]
    fn get_merged_cookies_browser_and_jar() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        store_cookie("jar_only=secret; Path=/");
        let merged = get_merged_cookies("browser_cookie=visible", "/api/test");
        assert!(merged.contains("jar_only=secret"));
        assert!(merged.contains("browser_cookie=visible"));
    }

    #[test]
    fn get_merged_cookies_jar_wins_conflict() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        store_cookie("token=from_jar; Path=/");
        let merged = get_merged_cookies("token=from_browser", "/");
        assert!(merged.contains("token=from_jar"));
        assert!(!merged.contains("from_browser"));
    }

    #[test]
    fn get_merged_cookies_path_filtering() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        store_cookie("a=1; Path=/api");
        store_cookie("b=2; Path=/web");
        let merged = get_merged_cookies("", "/api/data");
        assert!(merged.contains("a=1"));
        assert!(!merged.contains("b=2"));
    }

    #[test]
    fn get_merged_cookies_empty() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        let merged = get_merged_cookies("", "/");
        assert!(merged.is_empty());
    }

    #[test]
    fn clear_cookies_empties_jar() {
        let _lock = TEST_MUTEX.lock().unwrap();
        reset_jar();
        store_cookie("a=1; Path=/");
        store_cookie("b=2; Path=/");
        assert_eq!(cookie_count(), 2);
        clear_cookies();
        assert_eq!(cookie_count(), 0);
    }

    #[test]
    fn update_proxy_state_normalizes_dashboard() {
        let _lock = TEST_MUTEX.lock().unwrap();
        update_proxy_state("http://example.com", "tok", "openapi", "dashboard/");
        let s = get_proxy_state();
        assert_eq!(s.dashboard, "/dashboard");

        update_proxy_state("http://example.com", "tok", "openapi", "/admin/");
        let s = get_proxy_state();
        assert_eq!(s.dashboard, "/admin");

        update_proxy_state("http://example.com", "tok", "openapi", "");
        let s = get_proxy_state();
        assert_eq!(s.dashboard, "");
    }
}
