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
        let server_changed = state.server_url != server_url;
        if server_changed {
            config::clear_cookies();
            info!("Server changed from {} to {}, cookies cleared", state.server_url, server_url);
        }
        let effective_token = if token.is_empty() && !server_changed { &state.token } else { &token };
        let effective_auth = if auth_mode.is_empty() && !server_changed { &state.auth_mode } else { &auth_mode };
        config::update_proxy_state(&server_url, effective_token, effective_auth, &dashboard);
        info!("Proxy config updated (server={}, dashboard={})", server_url, dashboard);
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

/// Set UI language and rebuild tray menu with localized labels
#[tauri::command]
pub fn set_ui_language(app: AppHandle, lang: String) {
    config::save_ui_lang(&lang);
    crate::rebuild_tray(&app);
}

/// Sync theme/lang preferences to all windows (including CUI proxy pages).
/// Called from the settings window after changing theme or language.
/// For our own SPA windows this triggers Tauri events; for CUI pages
/// we inject JS directly via webview.eval() since the CUI page doesn't
/// have our event listeners.
#[tauri::command]
pub fn sync_preferences(app: AppHandle, theme: String, lang: String) {
    let umi_locale = match lang.as_str() {
        "zh" => "zh-CN",
        _ => "en-US",
    };
    let locale_cookie = match lang.as_str() {
        "zh" => "zh-cn",
        _ => "en-us",
    };
    let theme_val = if theme == "dark" { "dark" } else { "" };

    // Update cookie jar so future proxy requests carry the new prefs
    config::store_cookie(&format!("__locale={}; Path=/; Max-Age=31536000", locale_cookie));
    if theme_val.is_empty() {
        config::store_cookie("__theme=; Path=/; Max-Age=0");
    } else {
        config::store_cookie(&format!("__theme={}; Path=/; Max-Age=31536000", theme_val));
    }

    // Inject JS into every window to update localStorage + reload CUI pages
    let js = format!(
        r#"(function(){{
  try {{
    localStorage.setItem("umi_locale","{umi_locale}");
    localStorage.setItem("cui_lang","{lang}");
    localStorage.setItem("cui_theme","{theme}");
    if ("{theme_val}") {{
      localStorage.setItem("__theme","{theme_val}");
      localStorage.setItem("xgen:xgen_theme",JSON.stringify({{type:"String",value:"{theme_val}"}}));
    }} else {{
      localStorage.removeItem("__theme");
      localStorage.removeItem("xgen:xgen_theme");
    }}
    document.documentElement.setAttribute("data-theme","{theme}");
    // Notify SPA pages to re-render
    window.dispatchEvent(new CustomEvent("cui:theme-sync"));
    window.dispatchEvent(new CustomEvent("cui:lang-sync"));
    // If this is a CUI proxy page (not our SPA), reload to apply umi changes
    if (window.location.hostname === "127.0.0.1") {{
      window.location.reload();
    }}
  }} catch(e) {{}}
}})()"#,
        umi_locale = umi_locale,
        lang = lang,
        theme = theme,
        theme_val = theme_val,
    );

    for (label, webview) in app.webview_windows() {
        info!("sync_preferences: injecting into window '{}'", label);
        let _ = webview.eval(&js);
    }

    // Also sync the window chrome / title bar theme
    let t = match theme.as_str() {
        "dark" => Some(tauri::Theme::Dark),
        "light" => Some(tauri::Theme::Light),
        _ => None,
    };
    for window in app.webview_windows().values() {
        let _ = window.set_theme(t);
    }
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
