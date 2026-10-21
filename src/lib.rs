mod api;
mod chat;
mod localize;
mod models;
mod stream;
mod window;

use cosmic::widget;
use ron::de::from_reader;
use ron::ser::{to_string_pretty, PrettyConfig};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::PathBuf;
use window::Window;

pub fn run() -> cosmic::iced::Result {
    localize::localize();
    cosmic::applet::run::<Window>(())
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    avatar: PathBuf,
    keep_context: bool,
    api_endpoint: String,
    api_key: String,
    model: String,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            avatar: PathBuf::new(),
            keep_context: true,
            api_endpoint: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o-mini".to_string(),
        }
    }

    pub fn get_avatar_handle(&self) -> widget::image::Handle {
        if self.avatar.exists() {
            widget::image::Handle::from_path(&self.avatar)
        } else {
            let image: &[u8] = include_bytes!("../data/icons/avatar.png");
            widget::image::Handle::from_bytes(image)
        }
    }

    pub fn set_avatar(&mut self, path: PathBuf) -> &mut Self {
        self.avatar = path;
        self
    }

    pub fn change_context(&mut self, context: bool) -> &mut Self {
        self.keep_context = context;
        self
    }

    pub fn set_model(&mut self, model: String) -> &mut Self {
        self.model = model;
        self
    }

    pub fn set_api_endpoint(&mut self, endpoint: String) -> &mut Self {
        self.api_endpoint = endpoint;
        self
    }

    pub fn set_api_key(&mut self, key: String) -> &mut Self {
        self.api_key = key;
        self
    }

    pub fn load() -> Settings {
        let data_path = dirs::config_dir()
            .map(|d| d.join("cosmic-ext-applet-ai").join("settings.ron"))
            .unwrap_or_default();

        let settings = Settings::new();

        if let Some(path) = data_path.to_str().filter(|_| data_path.exists()) {
            if let Ok(opened) = File::open(&data_path) {
                let reader = BufReader::new(opened);
                if let Ok(parsed) = from_reader(reader) {
                    return parsed;
                }
            }
        }
        settings
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let Some(config_dir) = dirs::config_dir() else {
            anyhow::bail!("could not determine the user configuration directory");
        };
        let data_path = config_dir.join("cosmic-ext-applet-ai");

        fs::create_dir_all(&data_path)?;

        let pretty = PrettyConfig::default();
        let ron_string = to_string_pretty(self, pretty)?;

        let mut file = File::create(data_path.join("settings.ron"))?;
        file.write_all(ron_string.as_bytes())?;

        Ok(())
    }
}
