mod app_conf;
mod commands;
mod config;
mod proxy;

use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{
    Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    image::Image,
    WindowEvent,
};
use tauri::webview::{DownloadEvent, NewWindowResponse};
use tracing::{info, debug, warn};
use tracing_subscriber::EnvFilter;

/// Global counter for generating unique popup window labels
static POPUP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("cui_desktop_lib=info"))
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::default().build())
        .setup(|app| {
            // Load developer config.json at startup
            load_app_conf_from_resources(app.handle());

            // Store AppHandle globally so the proxy can call native APIs
            config::set_app_handle(app.handle().clone());

            // ── System Tray ──
            setup_tray(app)?;

            // Channel for navigation redirect requests (main window)
            let (tx, rx) = std::sync::mpsc::channel::<String>();

            // Clone AppHandle for use in closures
            let app_handle = app.handle().clone();
            let app_handle_dl = app.handle().clone();

            // Create the main window manually so we can attach on_navigation + on_new_window
            let window = WebviewWindowBuilder::new(
                    app,
                    "main",
                    WebviewUrl::App("index.html".into()),
                )
                .title("Yao Agents")
                .inner_size(1280.0, 860.0)
                .min_inner_size(900.0, 600.0)
                .center()
                .resizable(true)
                .decorations(true)
                .disable_drag_drop_handler()
                .on_navigation(move |url| {
                    let url_str = url.as_str();

                    // Always allow: tauri://, localhost
                    if url_str.starts_with("tauri://")
                        || url_str.starts_with("http://localhost")
                    {
                        return true;
                    }

                    // If navigating to the remote server, intercept and redirect through proxy
                    let state = config::get_proxy_state();
                    let local_base = format!("http://127.0.0.1:{}", state.port);

                    // Allow our own proxy
                    if url_str.starts_with(&local_base) {
                        return true;
                    }

                    if state.running && !state.server_url.is_empty() {
                        let remote = state.server_url.trim_end_matches('/');
                        if url_str.starts_with(remote) {
                            let proxy_url = url_str.replacen(remote, &local_base, 1);
                            info!("OAuth intercept: {} -> {}", url_str, proxy_url);
                            let _ = tx.send(proxy_url);
                            return false; // Block direct navigation to server
                        }
                    }

                    // Allow all other navigation (Google OAuth, GitHub, etc.)
                    debug!("External navigation: {}", url_str);
                    true
                })
                // Intercept window.open / target="_blank":
                // Return Deny immediately (avoids crash inside WebKit's createNewPage
                // callback), then spawn a new Tauri window asynchronously.
                // Authentication still works because the proxy manages cookies server-side.
                .on_new_window(move |url, _features| {
                    let url_str = url.to_string();
                    let handle = app_handle.clone();
                    info!("New window request: {}", url_str);

                    // Spawn outside the WebKit callback
                    std::thread::spawn(move || {
                        // Rewrite URL if it points to the remote server
                        let state = config::get_proxy_state();
                        let final_url = if state.running && !state.server_url.is_empty() {
                            let remote = state.server_url.trim_end_matches('/');
                            let local_base = format!("http://127.0.0.1:{}", state.port);
                            if url_str.starts_with(remote) {
                                url_str.replacen(remote, &local_base, 1)
                            } else {
                                url_str.clone()
                            }
                        } else {
                            url_str.clone()
                        };

                        // File download URL → download directly instead of popup
                        if is_file_download_url(&final_url) {
                            spawn_file_download(handle, final_url);
                            return;
                        }

                        let parsed = match url::Url::parse(&final_url) {
                            Ok(u) => u,
                            Err(e) => {
                                warn!("Failed to parse popup URL: {} — {}", final_url, e);
                                return;
                            }
                        };

                        let n = POPUP_COUNTER.fetch_add(1, Ordering::SeqCst);
                        let label = format!("popup_{}", n);
                        info!("Creating popup window: {} -> {}", label, final_url);
                        let handle_dl = handle.clone();
                        match WebviewWindowBuilder::new(
                            &handle,
                            &label,
                            WebviewUrl::External(parsed),
                        )
                        .title("Yao Agents")
                        .inner_size(1100.0, 780.0)
                        .min_inner_size(600.0, 400.0)
                        .center()
                        .resizable(true)
                        .disable_drag_drop_handler()
                        .on_document_title_changed(|wv, title| {
                            let _ = wv.set_title(&title);
                        })
                        .on_download(move |_wv, event| {
                            match event {
                                DownloadEvent::Requested { url, destination } => {
                                    if let Ok(dl_dir) = handle_dl.path().download_dir() {
                                        let fname = destination.file_name()
                                            .map(|f| f.to_string_lossy().to_string())
                                            .unwrap_or_else(|| "download".to_string());
                                        *destination = dl_dir.join(&fname);
                                    }
                                    info!("Popup download: {} -> {:?}", url.as_str(), destination);
                                }
                                DownloadEvent::Finished { url, path, success } => {
                                    info!("Popup download done: {} success={} path={:?}", url.as_str(), success, path);
                                }
                                _ => {}
                            }
                            true
                        })
                        .build()
                        {
                            Ok(_) => info!("Popup window created: {}", label),
                            Err(e) => warn!("Failed to create popup window: {}", e),
                        }
                    });

                    // Deny immediately so WebKit doesn't try to create a page
                    // (our async thread will handle it)
                    NewWindowResponse::Deny
                })
                // Handle file downloads: save to system Downloads folder
                .on_download(move |_webview, event| {
                    match event {
                        DownloadEvent::Requested { url, destination } => {
                            if let Ok(download_dir) = app_handle_dl.path().download_dir() {
                                let filename = destination.file_name()
                                    .map(|f| f.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "download".to_string());
                                *destination = download_dir.join(&filename);
                            }
                            info!("Download started: {} -> {:?}", url.as_str(), destination);
                        }
                        DownloadEvent::Finished { url, path, success } => {
                            if success {
                                info!("Download complete: {} -> {:?}", url.as_str(), path);
                            } else {
                                warn!("Download failed: {}", url.as_str());
                            }
                        }
                        _ => {}
                    }
                    true // allow all downloads
                })
                .build()?;

            // Background thread: process redirect requests
            let webview = window.clone();
            std::thread::spawn(move || {
                while let Ok(url) = rx.recv() {
                    if let Ok(parsed) = url::Url::parse(&url) {
                        info!("Redirecting to proxy: {}", parsed);
                        let _ = webview.navigate(parsed);
                    }
                }
            });

            Ok(())
        })
        // Intercept main window close: hide to tray instead of quitting.
        // Popup windows close normally.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    // Hide window instead of closing
                    let _ = window.hide();
                    api.prevent_close();
                    info!("Main window hidden to tray");
                }
                // Popup windows close normally (no prevent_close)
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_conf,
            commands::check_server,
            commands::start_proxy,
            commands::get_proxy_status,
            commands::update_proxy_token,
            commands::clear_cookies,
            commands::set_preference_cookies,
            commands::set_window_theme,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start Tauri application");
}

/// Set up the system tray icon and menu
fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    // Load the tray icon: monochrome template on macOS, colored on Windows/Linux
    let icon = load_tray_icon(app);

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos")) // macOS: monochrome template; others: colored
        .tooltip("Yao Agents")
        .menu(&menu)
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
                "quit" => {
                    info!("Quit from tray");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            // Left-click on tray icon → show and focus main window
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event {
                if let Some(win) = tray.app_handle().get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }
            }
        })
        .build(app)?;

    info!("System tray initialized");
    Ok(())
}

/// Load the tray icon PNG, trying multiple paths (bundled resources, dev icons/).
/// macOS: monochrome template icons; Windows/Linux: colored icons.
fn load_tray_icon(app: &tauri::App) -> Image<'static> {
    // Choose icon set based on platform:
    //   macOS: monochrome template (system auto-inverts for dark/light menu bar)
    //   Windows/Linux: colored icon for better visual identification
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["tray-icon@2x.png", "tray-icon.png"]
    } else {
        &["tray-icon-color@2x.png", "tray-icon-color.png", "tray-icon@2x.png", "tray-icon.png"]
    };

    // 1. Try from bundled resource directory
    if let Ok(resource_dir) = app.handle().path().resource_dir() {
        for name in candidates {
            let path = resource_dir.join("icons").join(name);
            if let Ok(img) = Image::from_path(&path) {
                info!("Tray icon loaded from: {:?}", path);
                return img;
            }
        }
    }

    // 2. Try from project icons/ directory (dev mode)
    for name in candidates {
        let path = std::path::PathBuf::from("icons").join(name);
        if let Ok(img) = Image::from_path(&path) {
            info!("Tray icon loaded from: {:?}", path);
            return img;
        }
    }

    // 3. Fallback: use embedded icon
    warn!("Tray icon not found, using fallback");
    if cfg!(target_os = "macos") {
        Image::from_bytes(include_bytes!("../icons/tray-icon.png")).unwrap()
    } else {
        Image::from_bytes(include_bytes!("../icons/tray-icon-color.png")).unwrap()
    }
}

// ========== File Download Helpers ==========

/// Check if a URL looks like a file download (Yao file API)
fn is_file_download_url(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        let path = parsed.path();
        // Yao file API: /v1/file/{namespace}/{hash}/content
        return path.starts_with("/v1/file/");
    }
    false
}

/// Spawn an async task to download a file from the proxy and save to Downloads folder.
fn spawn_file_download(handle: tauri::AppHandle, url: String) {
    info!("File download: {}", url);
    tauri::async_runtime::spawn(async move {
        let download_dir = match handle.path().download_dir() {
            Ok(d) => d,
            Err(e) => {
                warn!("Cannot resolve Downloads directory: {}", e);
                return;
            }
        };

        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .no_proxy()
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("Download client error: {}", e);
                return;
            }
        };

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("Download request failed: {} — {}", url, e);
                return;
            }
        };

        if !resp.status().is_success() {
            warn!("Download HTTP {}: {}", resp.status(), url);
            return;
        }

        // Extract filename from Content-Disposition header or URL
        let filename = extract_download_filename(&resp, &url);
        let dest = ensure_unique_path(download_dir.join(&filename));

        match resp.bytes().await {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&dest, &bytes) {
                    warn!("Failed to save file: {:?} — {}", dest, e);
                    return;
                }
                info!("Downloaded {} bytes → {:?}", bytes.len(), dest);

                // Reveal file in system file manager
                #[cfg(target_os = "macos")]
                { let _ = std::process::Command::new("open").arg("-R").arg(&dest).spawn(); }
                #[cfg(target_os = "windows")]
                { let _ = std::process::Command::new("explorer").arg("/select,").arg(&dest).spawn(); }
                #[cfg(target_os = "linux")]
                { let _ = std::process::Command::new("xdg-open").arg(&download_dir).spawn(); }
            }
            Err(e) => warn!("Failed to read response body: {} — {}", url, e),
        }
    });
}

/// Extract a filename from the response Content-Disposition header, falling back to the URL path.
fn extract_download_filename(resp: &reqwest::Response, url: &str) -> String {
    if let Some(cd) = resp.headers().get("content-disposition") {
        // Use from_utf8_lossy instead of to_str() — many servers send raw UTF-8
        // filenames (e.g. Chinese) which are not valid ASCII.
        // to_str() would fail silently and we'd lose the filename.
        let cd_str = String::from_utf8_lossy(cd.as_bytes());
        debug!("Content-Disposition: {}", cd_str);

        let cd_lower = cd_str.to_lowercase();

        // Try: filename*=UTF-8''encoded_name (RFC 5987)
        if let Some(pos) = cd_lower.find("filename*=") {
            let after = &cd_str[pos + 10..];
            // Strip charset prefix like "UTF-8''" or "utf-8''"
            let encoded = if let Some(idx) = after.find("''") {
                &after[idx + 2..]
            } else {
                after
            };
            let end = encoded.find(';').unwrap_or(encoded.len());
            let decoded = percent_decode(encoded[..end].trim());
            let decoded = decoded.trim().trim_matches('"');
            if !decoded.is_empty() {
                info!("Filename from Content-Disposition (RFC5987): {}", decoded);
                return sanitize_filename(decoded);
            }
        }

        // Try: filename="name" or filename=name
        if let Some(pos) = cd_lower.find("filename=") {
            let after = &cd_str[pos + 9..];
            let name = if after.starts_with('"') {
                after[1..].split('"').next().unwrap_or("")
            } else {
                let end = after.find(';').unwrap_or(after.len());
                after[..end].trim()
            };
            if !name.is_empty() {
                info!("Filename from Content-Disposition: {}", name);
                return sanitize_filename(name);
            }
        }
    }

    // Fallback: last meaningful path segment from URL
    if let Ok(parsed) = url::Url::parse(url) {
        let segments: Vec<&str> = parsed.path().split('/').filter(|s| !s.is_empty()).collect();
        for &seg in segments.iter().rev() {
            if seg != "content" && seg != "download" {
                info!("Filename from URL fallback: {}", seg);
                return sanitize_filename(seg);
            }
        }
    }
    "download".to_string()
}

/// Simple percent-decoding (for Content-Disposition filenames)
fn percent_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""),
                16,
            ) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Remove characters that are illegal in filenames
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}

/// If the path already exists, append (1), (2), … until unique
fn ensure_unique_path(path: std::path::PathBuf) -> std::path::PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = path.extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    for i in 1..1000 {
        let candidate = parent.join(format!("{} ({}){}", stem, i, ext));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

/// Load config.json from bundled resources or project root (dev mode)
fn load_app_conf_from_resources(app: &tauri::AppHandle) {
    if let Ok(resource_dir) = app.path().resource_dir() {
        let path = resource_dir.join("config.json");
        if path.exists() {
            info!("Loading config from resource dir: {:?}", path);
            app_conf::load_app_conf(&resource_dir);
            return;
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        if cwd.join("config.json").exists() {
            app_conf::load_app_conf(&cwd);
            return;
        }
        if let Some(parent) = cwd.parent() {
            if parent.join("config.json").exists() {
                app_conf::load_app_conf(&parent.to_path_buf());
                return;
            }
        }
    }

    info!("config.json not found, using defaults");
}
