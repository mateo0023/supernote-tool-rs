use std::error::Error;
use std::path::PathBuf;

use serde::{Serialize, Deserialize};

use crate::ServerConfig;

use super::MyApp;

#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub server_config: ServerConfig,
    pub combine_pdfs: bool,
    /// The name to save the Merged PDF
    pub out_name: String,
    pub show_only_empty: bool,
}

impl AppConfig {
    pub fn from_path(p: PathBuf) -> Result<Self, Box<dyn Error>> {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(p)?.read_to_string(&mut text)?;
        let cache = back_compat_deserialize!(text.as_str(), ServerConfig, AppConfig);
        cache.ok_or("Failed to deserialize".into())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            server_config: ServerConfig::default(),
            out_name: "EXPORT_FILE".to_string(),
            show_only_empty: false,
            combine_pdfs: true,
        }
    }
}

impl From<&mut MyApp> for AppConfig {
    fn from(value: &mut MyApp) -> Self {
        AppConfig {
            server_config: value.server_config.clone(),
            combine_pdfs: value.combine_pdfs,
            out_name: value.out_name.clone(),
            show_only_empty: value.show_only_empty,
        }
    }
}

impl From<ServerConfig> for AppConfig {
    fn from(value: ServerConfig) -> Self {
        AppConfig {
            server_config: value,
            ..Default::default()
        }
    }
}