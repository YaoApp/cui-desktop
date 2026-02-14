use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use tracing::{info, warn};

/// Proxy runtime state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyState {
    pub running: bool,
    pub port: u16,
    pub server_url: String,
    pub token: String,
    pub auth_mode: String,
}

impl Default for ProxyState {
    fn default() -> Self {
        Self {
            running: false,
            port: 19840,
            server_url: String::new(),
            token: String::new(),
            auth_mode: String::from("openapi"),
        }
    }
}

/// Global proxy state
pub static PROXY_STATE: Lazy<RwLock<ProxyState>> = Lazy::new(|| {
    RwLock::new(ProxyState::default())
});

pub fn update_proxy_state(server_url: &str, token: &str, auth_mode: &str) {
    let mut state = PROXY_STATE.write();
    state.server_url = server_url.to_string();
    state.token = token.to_string();
    state.auth_mode = auth_mode.to_string();
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
static COOKIE_JAR: Lazy<RwLock<Vec<CookieEntry>>> = Lazy::new(|| RwLock::new(Vec::new()));

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

/// Parse a Set-Cookie header and store it in the jar
pub fn store_cookie(set_cookie: &str) {
    let parts: Vec<&str> = set_cookie.split(';').collect();
    if parts.is_empty() {
        return;
    }

    // Parse name=value
    let name_value = parts[0].trim();
    let (name, value) = match name_value.split_once('=') {
        Some((n, v)) => (n.trim().to_string(), v.trim().to_string()),
        None => return,
    };

    if name.is_empty() {
        return;
    }

    let mut path = "/".to_string();
    let mut expires_at: u64 = 0;
    let mut http_only = false;

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
                    return;
                }
            }
        } else if lower == "httponly" {
            http_only = true;
        }
    }

    let entry = CookieEntry {
        name: name.clone(),
        value,
        path,
        expires_at,
        http_only,
    };

    // Upsert
    let mut jar = COOKIE_JAR.write();
    if let Some(existing) = jar.iter_mut().find(|c| c.name == name) {
        *existing = entry;
    } else {
        jar.push(entry);
    }
    drop(jar);

    save_cookies();
}

/// Remove a cookie by name
fn remove_cookie(name: &str) {
    let mut jar = COOKIE_JAR.write();
    jar.retain(|c| c.name != name);
    drop(jar);
    save_cookies();
}

/// Build a Cookie header value for a given request path
/// e.g. "__Secure-access_token=xxx; __Secure-csrf_token=yyy"
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
