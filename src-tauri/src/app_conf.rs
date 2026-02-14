use once_cell::sync::Lazy;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

/// Developer-facing application config (config.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConf {
    /// Application display name
    #[serde(default = "default_name")]
    pub name: String,

    /// Logo image path (relative to resources, empty = text-only)
    #[serde(default)]
    pub logo: String,

    /// Local proxy port (register http://127.0.0.1:PORT as OAuth redirect_uri)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Theme overrides
    #[serde(default)]
    pub theme: ThemeConf,

    /// Auto-updater settings
    #[serde(default)]
    pub updater: UpdaterConf,

    /// Default server list (pre-configured by developer)
    #[serde(default)]
    pub servers: Vec<ServerPreset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConf {
    #[serde(default = "default_primary_color", rename = "primaryColor")]
    pub primary_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterConf {
    #[serde(default)]
    pub active: bool,

    #[serde(default)]
    pub endpoints: Vec<String>,

    #[serde(default)]
    pub pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerPreset {
    /// Server URL
    pub url: String,

    /// Display label
    #[serde(default)]
    pub label: String,
}

// Defaults
fn default_name() -> String { "Yao Desktop".to_string() }
fn default_port() -> u16 { 15099 }
fn default_primary_color() -> String { "#3b82f6".to_string() }

impl Default for AppConf {
    fn default() -> Self {
        Self {
            name: default_name(),
            logo: String::new(),
            port: default_port(),
            theme: ThemeConf::default(),
            updater: UpdaterConf::default(),
            servers: vec![],
        }
    }
}

impl Default for ThemeConf {
    fn default() -> Self {
        Self {
            primary_color: default_primary_color(),
        }
    }
}

impl Default for UpdaterConf {
    fn default() -> Self {
        Self {
            active: false,
            endpoints: vec![],
            pubkey: String::new(),
        }
    }
}

/// Global app config (loaded once at startup)
static APP_CONF: Lazy<RwLock<AppConf>> = Lazy::new(|| RwLock::new(AppConf::default()));

/// Load config.json from the given path
pub fn load_app_conf(resource_dir: &PathBuf) {
    let config_path = resource_dir.join("config.json");
    if !config_path.exists() {
        info!("config.json not found at {:?}, using defaults", config_path);
        return;
    }

    match std::fs::read_to_string(&config_path) {
        Ok(data) => {
            match serde_json::from_str::<AppConf>(&data) {
                Ok(conf) => {
                    info!("Loaded config.json: name={}, servers={}", conf.name, conf.servers.len());
                    *APP_CONF.write() = conf;
                }
                Err(e) => warn!("Failed to parse config.json: {}", e),
            }
        }
        Err(e) => warn!("Failed to read config.json: {}", e),
    }
}

/// Get the current app config
pub fn get_app_conf() -> AppConf {
    APP_CONF.read().clone()
}
