use base64::prelude::*;
use chrono::Local;
use ron::{
    from_str,
    ser::{to_string_pretty, PrettyConfig},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Text {
    User(MessageContent),
    Bot(MessageContent),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageContent {
    Text(String),
    Image(ImageAttachment),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ImageAttachment {
    Svg(Image),
    Raster(Image),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Image {
    pub base64: String,
    #[serde(default = "default_mime")]
    pub mime_type: String,
    #[serde(skip)]
    pub data: bytes::Bytes,
}

fn default_mime() -> String {
    "image/jpeg".to_string()
}

/// Detect the MIME type of an image from its magic bytes, falling back to the
/// file extension and finally to `image/jpeg`.
fn detect_mime(path: &str, data: &[u8]) -> String {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        "image/png".to_string()
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg".to_string()
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "image/gif".to_string()
    } else {
        match std::path::Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") => "image/png".to_string(),
            Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
            Some("gif") => "image/gif".to_string(),
            _ => default_mime(),
        }
    }
}

impl Image {
    pub fn new(path: &str) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mime_type = detect_mime(path, &buffer);
        let data = bytes::Bytes::from(buffer);

        Ok(Self {
            base64: BASE64_STANDARD.encode(&data),
            mime_type,
            data,
        })
    }

    /// The MIME type of this image, e.g. `image/png`.
    pub fn mime(&self) -> String {
        self.mime_type.clone()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Conversation {
    pub messages: Vec<Text>,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn push(&mut self, message: Text) -> &mut Self {
        self.messages.push(message);
        self
    }

    pub fn save_to_file(&self) -> anyhow::Result<()> {
        let Some(data_dir) = dirs::data_dir() else {
            anyhow::bail!("could not determine the user data directory");
        };
        let data_path = data_dir.join("cosmic-ext-applet-ai/chat");

        fs::create_dir_all(&data_path)?;

        let now = Local::now();
        let formatted = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let filename = format!("{}.ron", formatted);

        let pretty = PrettyConfig::default();
        let ron_string = to_string_pretty(self, pretty)?;

        let mut file = File::create(data_path.join(filename))?;
        file.write_all(ron_string.as_bytes())?;

        Ok(())
    }

    pub fn remove(&self, filename: String) -> anyhow::Result<()> {
        let Some(data_dir) = dirs::data_dir() else {
            anyhow::bail!("could not determine user data dir");
        };
        let data_path = data_dir
            .join("cosmic-ext-applet-ai/chat")
            .join(format!("{}.ron", &filename));

        fs::remove_file(data_path)?;

        Ok(())
    }
}

pub fn read_conversation_files() -> anyhow::Result<Vec<String>> {
    let Some(data_dir) = dirs::data_dir() else {
        anyhow::bail!("could not determine user data dir");
    };
    let data_path = data_dir.join("cosmic-ext-applet-ai/chat");

    let mut conversations = Vec::new();

    if let Ok(entries) = fs::read_dir(&data_path) {
        for entry in entries.flatten() {
            if let Some(stem) = entry.path().file_stem() {
                if let Some(s) = stem.to_str() {
                    conversations.push(s.to_owned());
                }
            }
        }
    }

    Ok(conversations)
}

pub fn load_conversation(filename: String) -> anyhow::Result<Conversation> {
    let Some(data_dir) = dirs::data_dir() else {
        anyhow::bail!("could not determine user data dir");
    };
    let data_path = data_dir
        .join("cosmic-ext-applet-ai/chat")
        .join(format!("{}.ron", filename));

    let contents = fs::read_to_string(&data_path)?;
    Ok(from_str(&contents)?)
}
