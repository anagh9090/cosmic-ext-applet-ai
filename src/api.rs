use futures::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::Settings;

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: Content,
}

/// The content of a single chat message. Plain text for most turns, a list of
/// parts when the turn carries attachments.
#[derive(Serialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BotResponse {
    pub response: String,
}

#[derive(Debug)]
pub struct Bot {}

impl Bot {
    pub async fn new(
        model: String,
        prompt: String,
        images: Vec<(String, String)>,
        history: Vec<(String, String)>,
        _keep_alive: String,
    ) -> anyhow::Result<(
        Self,
        impl Stream<Item = anyhow::Result<BotResponse, reqwest::Error>>,
    )> {
        let settings = Settings::load();
        let client = Client::new();

        // The model is taken from the settings so a change made in the settings
        // page takes effect immediately, falling back to the requested one.
        let model = if settings.model.is_empty() {
            model
        } else {
            settings.model.clone()
        };

        let mut messages: Vec<ChatMessage> = history
            .into_iter()
            .map(|(role, content)| ChatMessage {
                role,
                content: Content::Text(content),
            })
            .collect();

        // The current turn carries the prompt plus any attached images, which
        // are sent as inline data URLs.
        let content = if images.is_empty() {
            Content::Text(prompt)
        } else {
            let mut parts = vec![ContentPart::Text { text: prompt }];

            for (mime, base64) in images {
                parts.push(ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: format!("data:{mime};base64,{base64}"),
                    },
                });
            }

            Content::Parts(parts)
        };

        messages.push(ChatMessage {
            role: "user".to_string(),
            content,
        });

        let request = ChatCompletionRequest {
            model,
            messages,
            stream: false,
        };

        let response = client
            .post(format!("{}/chat/completions", settings.api_endpoint))
            .header("Authorization", format!("Bearer {}", settings.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, body);
        }

        let text: String = response.text().await?;
        let response_json: serde_json::Value = serde_json::from_str(&text)?;

        let content = response_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        let bot_response = BotResponse {
            response: content,
        };

        Ok((Self {}, futures::stream::once(async move { Ok(bot_response) })))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub created: Option<u64>,
    pub owned_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<Model>,
}

/// Lists the models exposed by the configured endpoint. Not every
/// OpenAI-compatible server implements `/models`, so failures are reported to
/// the caller instead of being fatal.
pub fn list_models(endpoint: &str, api_key: &str) -> anyhow::Result<Vec<String>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;

    let response = client
        .get(format!("{endpoint}/models"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send()?;

    if !response.status().is_success() {
        let status = response.status();
        anyhow::bail!("API error {status}");
    }

    let text: String = response.text()?;
    let models: ModelsResponse = serde_json::from_str(&text)?;

    let mut ids: Vec<String> = models.data.into_iter().map(|model| model.id).collect();
    ids.sort();

    Ok(ids)
}
