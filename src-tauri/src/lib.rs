mod app_conf;
mod commands;
mod config;
mod proxy;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{info, debug};
use tracing_subscriber::EnvFilter;

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

            // Channel for navigation redirect requests
            let (tx, rx) = std::sync::mpsc::channel::<String>();

            // Create the main window manually so we can attach on_navigation
            let window = WebviewWindowBuilder::new(
                    app,
                    "main",
                    WebviewUrl::App("index.html".into()),
                )
                .title("Yao CUI Desktop")
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
        .invoke_handler(tauri::generate_handler![
            commands::get_app_conf,
            commands::check_server,
            commands::start_proxy,
            commands::get_proxy_status,
            commands::update_proxy_token,
            commands::clear_cookies,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start Tauri application");
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
