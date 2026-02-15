use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::info;
use std::path::PathBuf;

use crate::app_conf::AppConf;
use crate::config::{self, ProxyState};
use crate::proxy;

/// Login result returned to the frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResult {
    pub success: bool,
    pub message: String,
    pub token: String,
    pub auth_mode: String,
}

/// Server metadata from .well-known/yao
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WellKnownInfo {
    pub name: Option<String>,
    pub version: Option<String>,
    pub openapi: Option<String>,
    pub dashboard: Option<String>,
    pub issuer_url: Option<String>,
}

/// Resolve the CUI build output directory
fn get_cui_dist_path(app: &AppHandle) -> PathBuf {
    // 1. Tauri resource directory (bundled app, highest priority)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let prod_path = resource_dir.join("cui-dist");
        if prod_path.exists() {
            info!("CUI path (resource): {:?}", prod_path);
            return prod_path;
        }
    }

    // 2. Project root (dev mode)
    if let Ok(cwd) = std::env::current_dir() {
        let here = cwd.join("cui-dist");
        if here.exists() {
            info!("CUI path (cwd): {:?}", here);
            return here;
        }
        if let Some(parent) = cwd.parent() {
            let dev_path = parent.join("cui-dist");
            if dev_path.exists() {
                info!("CUI path (cwd parent): {:?}", dev_path);
                return dev_path;
            }
        }
    }

    // 3. Fallback
    let fallback = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .map(|p| p.join("cui-dist"))
        .unwrap_or_else(|| PathBuf::from("cui-dist"));
    info!("CUI path (fallback): {:?}", fallback);
    fallback
}

/// Get the developer app config (loaded at startup)
#[tauri::command]
pub async fn get_app_conf() -> AppConf {
    crate::app_conf::get_app_conf()
}

/// Check remote server availability via .well-known/yao
#[tauri::command]
pub async fn check_server(server_url: String) -> Result<WellKnownInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/.well-known/yao", server_url.trim_end_matches('/'));
    info!("Checking server: {}", url);

    let resp = client.get(&url).send().await
        .map_err(|e| format!("Cannot connect to server: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }

    let info: WellKnownInfo = resp.json().await
        .map_err(|e| format!("Failed to parse server response: {}", e))?;

    Ok(info)
}

/// Start the local proxy server
#[tauri::command]
pub async fn start_proxy(
    app: AppHandle,
    server_url: String,
    token: String,
    auth_mode: String,
    dashboard: String,
) -> Result<u16, String> {
    let state = config::get_proxy_state();
    if state.running {
        config::update_proxy_state(&server_url, &token, &auth_mode, &dashboard);
        info!("Proxy config updated (dashboard={})", dashboard);
        return Ok(state.port);
    }

    config::update_proxy_state(&server_url, &token, &auth_mode, &dashboard);

    // Set up cookie jar
    if let Ok(app_data) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&app_data);
        let cookie_file = app_data.join("cookies.json");
        info!("Cookie file: {:?}", cookie_file);
        config::set_cookie_file(cookie_file);
        config::load_cookies();
    }

    let cui_dist = get_cui_dist_path(&app);
    info!("CUI dist path: {:?}", cui_dist);

    // Use port from developer config
    let conf = crate::app_conf::get_app_conf();
    let port = proxy::start_proxy_server(cui_dist, conf.port).await?;
    Ok(port)
}

/// Get current proxy status
#[tauri::command]
pub async fn get_proxy_status() -> ProxyState {
    config::get_proxy_state()
}

/// Update the proxy auth token
#[tauri::command]
pub async fn update_proxy_token(token: String) -> Result<(), String> {
    let state = config::get_proxy_state();
    config::update_proxy_state(&state.server_url, &token, &state.auth_mode, &state.dashboard);
    Ok(())
}

/// Clear all stored cookies
#[tauri::command]
pub async fn clear_cookies() -> Result<(), String> {
    config::clear_cookies();
    info!("Cookies cleared");
    Ok(())
}

/// Set the window theme (title bar color) for all windows.
/// Accepts "dark" or "light".
#[tauri::command]
pub async fn set_window_theme(app: AppHandle, theme: String) -> Result<(), String> {
    let t = match theme.as_str() {
        "dark" => Some(tauri::Theme::Dark),
        "light" => Some(tauri::Theme::Light),
        _ => None,
    };
    for window in app.webview_windows().values() {
        let _ = window.set_theme(t);
    }
    info!("Window theme set to: {}", theme);
    Ok(())
}

/// Set user preference cookies (__locale, __theme) in the cookie jar.
/// These are sent to the server and injected into browser on CUI page load.
#[tauri::command]
pub async fn set_preference_cookies(locale: String, theme: String) -> Result<(), String> {
    if !locale.is_empty() {
        config::store_cookie(&format!("__locale={}; Path=/; Max-Age=31536000", locale));
    }
    if !theme.is_empty() {
        config::store_cookie(&format!("__theme={}; Path=/; Max-Age=31536000", theme));
    } else {
        // Clear __theme cookie (empty = default/light)
        config::store_cookie("__theme=; Path=/; Max-Age=0");
    }
    info!("Preference cookies set: locale={}, theme={}", locale, theme);
    Ok(())
}
