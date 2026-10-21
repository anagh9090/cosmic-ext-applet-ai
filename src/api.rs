use futures::Stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::Settings;

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
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
    pub context: Option<Vec<u64>>,
}

#[derive(Debug)]
pub struct Bot {}

impl Bot {
    pub async fn new(
        model: String,
        prompt: String,
        _images: Vec<String>,
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
            .map(|(role, content)| ChatMessage { role, content })
            .collect();

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: prompt,
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
            context: None,
        };

        Ok((Self {}, futures::stream::once(async move { Ok(bot_response) })))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tags {
    pub data: Vec<Model>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub created: Option<u64>,
    pub owned_by: Option<String>,
}

#[derive(Debug)]
pub struct ListModels {
    pub result: anyhow::Result<Tags, reqwest::Error>,
}

impl ListModels {
    pub fn new() -> Self {
        let settings = Settings::load();
        let client = reqwest::blocking::Client::new();

        let result = client
            .get(format!("{}/models", settings.api_endpoint))
            .header("Authorization", format!("Bearer {}", settings.api_key))
            .header("Content-Type", "application/json")
            .send()
            .and_then(|response| response.json::<Tags>());

        Self { result }
    }
}
