use std::{
    cell::{LazyCell, OnceCell},
    sync::{LazyLock, OnceLock},
};

use amanda_aicore::{
    init_opts::{Language, Persona},
    providers::Provider,
};
use amanda_lowiq::{languages::autoadapt, personas::amanda};
use amanda_shared::color_eyre::{Result, eyre::Context};
use amanda_shared::dirs;
use serde;
pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Settings {
    pub config_version: u32,
    pub persona: Persona,
    pub language: Language,
    pub provider: Option<Provider>,
}

impl Settings {
    fn build_path() -> std::path::PathBuf {
        let config_dir = dirs::config_dir().expect("Could not find config directory");
        let amanda_config_dir = config_dir.join("amanda");
        if !amanda_config_dir.exists() {
            std::fs::create_dir_all(&amanda_config_dir)
                .expect("Could not create Amanda config directory");
        }

        return amanda_config_dir;
    }

    pub fn build_file_path() -> std::path::PathBuf {
        let path = Settings::build_path().join("settings.toml");
        return path;
    }

    pub fn load_or_create() -> Result<Settings> {
        let path = Settings::build_file_path();
        if !path.exists() {
            let default_settings = Settings::default_settings();
            default_settings.save()?;
            return Ok(default_settings);
        }

        let contents =
            std::fs::read_to_string(&path).wrap_err("Failed to read config directory")?;
        let settings: Settings =
            toml::from_str(&contents).wrap_err("unable to parse the config")?;

        if settings.config_version != CONFIG_VERSION {
            // Handle config migration here if needed
            // For now, we just return default settings
            let default_settings = Settings::default_settings();
            default_settings.save()?;
            return Ok(default_settings);
        }

        Ok(settings)
    }

    pub fn save(&self) -> Result<()> {
        let path = Settings::build_file_path();
        let contents = toml::to_string_pretty(self).wrap_err("unable to serialize the config")?;

        std::fs::write(path, contents).wrap_err("Failed to save config directory")
    }

    pub fn default_settings() -> Settings {
        Settings {
            config_version: CONFIG_VERSION,
            persona: amanda(),
            language: autoadapt(),
            provider: None,
        }
    }
}
