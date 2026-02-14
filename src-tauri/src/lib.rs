mod commands;
mod config;
mod proxy;

use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("cui_desktop_lib=info"))
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::check_server,
            commands::login_openapi,
            commands::login_legacy,
            commands::start_proxy,
            commands::get_proxy_status,
            commands::update_proxy_token,
            commands::clear_cookies,
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start Tauri application");
}
