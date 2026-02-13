use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

pub mod cortex;
pub mod privacy;
pub mod reflex;

/// The interface for any NPU backend.
#[async_trait]
pub trait NeuralBackend: Send + Sync {
    async fn fix_command(&self, broken_command: &str) -> Result<String>;
    async fn explain_command(&self, command: &str) -> Result<String>;
}

/// Direct HTTP client for Lemonade / any OpenAI-compatible server.
/// Uses reqwest instead of async-openai to avoid version breakage.
pub struct LemonadeClient {
    http: Client,
    base_url: String,
    model_name: String,
}

impl LemonadeClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model_name: model.to_string(),
        }
    }

    /// Send a chat completion request and return the content string.
    async fn chat(&self, messages: Vec<Value>) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);

        let body = json!({
            "model": self.model_name,
            "messages": messages,
            "stream": false
        });

        let resp = self.http
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Failed to contact NPU")?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "NPU returned {}: {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        let parsed: Value = serde_json::from_str(&text)?;

        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .context("NPU returned empty content")?
            .trim()
            .to_string();

        Ok(content)
    }
}

#[async_trait]
impl NeuralBackend for LemonadeClient {
    async fn fix_command(&self, broken_command: &str) -> Result<String> {
        let messages = vec![
            json!({
                "role": "system",
                "content": "You are a terminal expert. Fix the user's command. Output ONLY the fixed command."
            }),
            json!({
                "role": "user",
                "content": broken_command
            }),
        ];

        self.chat(messages).await
    }

    async fn explain_command(&self, command: &str) -> Result<String> {
        let messages = vec![
            json!({
                "role": "system",
                "content": "Explain this command briefly in one sentence."
            }),
            json!({
                "role": "user",
                "content": command
            }),
        ];

        self.chat(messages).await
    }
}