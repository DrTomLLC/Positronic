use crate::privacy::PrivacyGuard;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{info, warn};

/// The Cortex is the interface to the Neural Processing Unit (or local server).
///
/// Uses raw HTTP via reqwest instead of async-openai to avoid
/// version breakage. Lemonade and other local servers all speak
/// the same simple OpenAI-compatible JSON protocol.
pub struct NeuralClient {
    http: Client,
    base_url: String,
    model: String,
}

impl std::fmt::Debug for NeuralClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeuralClient")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .finish()
    }
}

impl NeuralClient {
    /// Connect to a local LLM server.
    ///
    /// `base_url` should be the OpenAI-compatible API root:
    ///   - Lemonade:    "http://localhost:8000/api/v1"
    ///   - llama.cpp:   "http://localhost:8080/v1"
    ///   - Ollama:      "http://localhost:11434/v1"
    ///
    /// `model` can be a specific model name, or "auto" to query
    /// the server's /models endpoint and pick the first available.
    pub fn new(base_url: &str, model: &str) -> Self {
        info!("Cortex targeting: {} (model: {})", base_url, model);

        Self {
            http: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    /// Ask the AI a question, with PII scrubbing.
    pub async fn ask(&self, prompt: &str) -> anyhow::Result<String> {
        if prompt.trim().is_empty() {
            return Ok("Usage: !ai <your question here>".to_string());
        }

        let safe_prompt = PrivacyGuard::scrub(prompt);
        if safe_prompt != prompt {
            warn!("Original prompt contained PII — scrubbed before sending.");
        }

        let model_name = if self.model.is_empty()
            || self.model.eq_ignore_ascii_case("auto")
        {
            self.detect_model().await?
        } else {
            self.model.clone()
        };

        info!("Cortex using model: {}", model_name);

        let url = format!("{}/chat/completions", self.base_url);

        let body = json!({
            "model": model_name,
            "messages": [
                {
                    "role": "system",
                    "content": "You are Positronic, an intelligent terminal assistant. Be concise."
                },
                {
                    "role": "user",
                    "content": safe_prompt
                }
            ],
            "stream": false
        });

        let resp = self.http
            .post(&url)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "Server returned {}: {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        let parsed: Value = serde_json::from_str(&text)?;

        let answer = parsed["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No response from model.")
            .to_string();

        Ok(answer)
    }

    /// Query the server's /models endpoint to find the first available model.
    async fn detect_model(&self) -> anyhow::Result<String> {
        let url = format!("{}/models", self.base_url);
        let resp = self.http.get(&url).send().await?;
        let body: Value = resp.json().await?;

        if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
            if let Some(first) = data.first() {
                if let Some(id) = first.get("id").and_then(|v| v.as_str()) {
                    info!("Auto-detected model: {}", id);
                    return Ok(id.to_string());
                }
            }
        }

        Err(anyhow::anyhow!(
            "No models found at {}. Load a model in Lemonade first.",
            url
        ))
    }
}