use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tracing::info;
use std::path::PathBuf;

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
    //    macOS: Contents/Resources/cui-dist/
    if let Ok(resource_dir) = app.path().resource_dir() {
        let prod_path = resource_dir.join("cui-dist");
        if prod_path.exists() {
            info!("CUI path (resource): {:?}", prod_path);
            return prod_path;
        }
    }

    // 2. Project root (dev mode)
    //    cargo tauri dev sets CWD to src-tauri/, parent is project root
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

    // 3. Fallback — return even if missing; proxy will show "CUI not built" page
    let fallback = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .map(|p| p.join("cui-dist"))
        .unwrap_or_else(|| PathBuf::from("cui-dist"));
    info!("CUI path (fallback): {:?}", fallback);
    fallback
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

/// OpenAPI login flow
#[tauri::command]
pub async fn login_openapi(
    server_url: String,
    username: String,
    password: String,
) -> Result<LoginResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let base = server_url.trim_end_matches('/');

    // Step 1: Get login entry configuration
    let entry_url = format!("{}/v1/user/entry", base);
    info!("Fetching login entry: {}", entry_url);

    let entry_resp = client.get(&entry_url).send().await
        .map_err(|e| format!("Failed to fetch login entry: {}", e))?;

    if !entry_resp.status().is_success() {
        return Err(format!("Failed to fetch login entry: HTTP {}", entry_resp.status()));
    }

    // Step 2: Verify username
    let verify_url = format!("{}/v1/user/entry/verify", base);
    info!("Verifying user: {}", verify_url);

    let verify_body = serde_json::json!({
        "username": username
    });

    let verify_resp = client.post(&verify_url)
        .json(&verify_body)
        .send()
        .await
        .map_err(|e| format!("User verification failed: {}", e))?;

    if !verify_resp.status().is_success() {
        let status = verify_resp.status();
        let body = verify_resp.text().await.unwrap_or_default();
        return Err(format!("User verification failed: HTTP {} - {}", status, body));
    }

    let verify_result: serde_json::Value = verify_resp.json().await
        .map_err(|e| format!("Failed to parse verify response: {}", e))?;

    let temp_token = verify_result.get("token")
        .or_else(|| verify_result.get("access_token"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Token not found in verify response".to_string())?;

    // Step 3: Login with password
    let login_url = format!("{}/v1/user/entry/login", base);
    info!("Logging in: {}", login_url);

    let login_body = serde_json::json!({
        "password": password
    });

    let login_resp = client.post(&login_url)
        .header("Authorization", format!("Bearer {}", temp_token))
        .json(&login_body)
        .send()
        .await
        .map_err(|e| format!("Login failed: {}", e))?;

    if !login_resp.status().is_success() {
        let status = login_resp.status();
        let body = login_resp.text().await.unwrap_or_default();
        return Err(format!("Login failed: HTTP {} - {}", status, body));
    }

    let login_result: serde_json::Value = login_resp.json().await
        .map_err(|e| format!("Failed to parse login response: {}", e))?;

    let token = login_result.get("token")
        .or_else(|| login_result.get("access_token"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Token not found in login response".to_string())?
        .to_string();

    Ok(LoginResult {
        success: true,
        message: "Login successful".to_string(),
        token,
        auth_mode: "openapi".to_string(),
    })
}

/// Legacy login flow
#[tauri::command]
pub async fn login_legacy(
    server_url: String,
    username: String,
    password: String,
) -> Result<LoginResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let base = server_url.trim_end_matches('/');
    let login_url = format!("{}/api/__yao/login/admin", base);
    info!("Legacy login: {}", login_url);

    let login_body = serde_json::json!({
        "email": username,
        "password": password
    });

    let login_resp = client.post(&login_url)
        .json(&login_body)
        .send()
        .await
        .map_err(|e| format!("Login failed: {}", e))?;

    if !login_resp.status().is_success() {
        let status = login_resp.status();
        let body = login_resp.text().await.unwrap_or_default();
        return Err(format!("Login failed: HTTP {} - {}", status, body));
    }

    let login_result: serde_json::Value = login_resp.json().await
        .map_err(|e| format!("Failed to parse login response: {}", e))?;

    let token = login_result.get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Token not found in login response".to_string())?
        .to_string();

    Ok(LoginResult {
        success: true,
        message: "Login successful (Legacy)".to_string(),
        token,
        auth_mode: "legacy".to_string(),
    })
}

/// Start the local proxy server
#[tauri::command]
pub async fn start_proxy(
    app: AppHandle,
    server_url: String,
    token: String,
    auth_mode: String,
) -> Result<u16, String> {
    let state = config::get_proxy_state();
    if state.running {
        // Proxy already running — just update config
        config::update_proxy_state(&server_url, &token, &auth_mode);
        info!("Proxy config updated");
        return Ok(state.port);
    }

    // Update config
    config::update_proxy_state(&server_url, &token, &auth_mode);

    // Set up cookie jar file path and load existing cookies
    if let Ok(app_data) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&app_data);
        let cookie_file = app_data.join("cookies.json");
        info!("Cookie file: {:?}", cookie_file);
        config::set_cookie_file(cookie_file);
        config::load_cookies();
    }

    // Resolve CUI build output path
    let cui_dist = get_cui_dist_path(&app);
    info!("CUI dist path: {:?}", cui_dist);

    // Start proxy
    let port = proxy::start_proxy_server(cui_dist).await?;
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
    config::update_proxy_state(&state.server_url, &token, &state.auth_mode);
    Ok(())
}

/// Clear all stored cookies
#[tauri::command]
pub async fn clear_cookies() -> Result<(), String> {
    config::clear_cookies();
    info!("Cookies cleared");
    Ok(())
}
