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
use tauri::webview::NewWindowResponse;
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

            // Clone AppHandle for use in on_new_window closure
            let app_handle = app.handle().clone();

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

                    // Spawn window creation outside the WebKit callback
                    std::thread::spawn(move || {
                        let n = POPUP_COUNTER.fetch_add(1, Ordering::SeqCst);
                        let label = format!("popup_{}", n);

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

                        let parsed = match url::Url::parse(&final_url) {
                            Ok(u) => u,
                            Err(e) => {
                                warn!("Failed to parse popup URL: {} — {}", final_url, e);
                                return;
                            }
                        };

                        info!("Creating popup window: {} -> {}", label, final_url);
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
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start Tauri application");
}

/// Set up the system tray icon and menu
fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    // Load the tray-specific template icon (monochrome silhouette).
    // macOS will auto-invert for dark/light mode when icon_as_template(true).
    let icon = load_tray_icon(app);

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true) // macOS: system handles dark/light mode
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

/// Load the tray icon PNG, trying multiple paths (bundled resources, dev icons/)
fn load_tray_icon(app: &tauri::App) -> Image<'static> {
    // Retina icon first (44x44 fallback to 88x88)
    let candidates = ["tray-icon@2x.png", "tray-icon.png"];

    // 1. Try from bundled resource directory
    if let Ok(resource_dir) = app.handle().path().resource_dir() {
        for name in &candidates {
            let path = resource_dir.join("icons").join(name);
            if let Ok(img) = Image::from_path(&path) {
                info!("Tray icon loaded from: {:?}", path);
                return img;
            }
        }
    }

    // 2. Try from project icons/ directory (dev mode)
    for name in &candidates {
        let path = std::path::PathBuf::from("icons").join(name);
        if let Ok(img) = Image::from_path(&path) {
            info!("Tray icon loaded from: {:?}", path);
            return img;
        }
    }

    // 3. Fallback: generate a simple 1x1 transparent pixel icon
    warn!("Tray icon not found, using fallback");
    Image::from_bytes(include_bytes!("../icons/tray-icon.png")).unwrap()
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
