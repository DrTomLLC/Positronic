use crate::privacy::PrivacyGuard;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
};
use tracing::{info, warn};

/// The Cortex is the interface to the Neural Processing Unit (or local server).
pub struct NeuralClient {
    client: Client<OpenAIConfig>,
    model: String,
    base_url: String,
}

impl std::fmt::Debug for NeuralClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeuralClient")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl NeuralClient {
    /// Connect to a local Lemonade server.
    ///
    /// `base_url` should be the OpenAI-compatible API root:
    ///   - Lemonade:    "http://localhost:8000/api/v1"
    ///   - llama.cpp:   "http://localhost:8080/v1"
    ///   - Ollama:      "http://localhost:11434/v1"
    ///
    /// `model` can be a specific model name, or "auto" to query
    /// the server's /models endpoint and pick the first available.
    pub fn new(base_url: &str, model: &str) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key("sk-no-key-required");
        let client = Client::with_config(config);

        info!("Cortex targeting: {} (model: {})", base_url, model);

        Self {
            client,
            model: model.to_string(),
            base_url: base_url.to_string(),
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

        // Auto-detect model if set to "auto" or empty
        let model_name = if self.model.is_empty()
            || self.model.eq_ignore_ascii_case("auto")
        {
            match self.detect_model().await {
                Ok(m) => m,
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Could not auto-detect model: {}. \
                         Ensure Lemonade has a model loaded.",
                        e
                    ));
                }
            }
        } else {
            self.model.clone()
        };

        info!("Cortex using model: {}", model_name);

        let request = CreateChatCompletionRequestArgs::default()
            .model(&model_name)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(
                        "You are Positronic, an intelligent terminal assistant. Be concise.",
                    )
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(safe_prompt)
                    .build()?
                    .into(),
            ])
            .build()?;

        let response = self.client.chat().create(request).await?;

        let answer = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| "No response from model.".to_string());

        Ok(answer)
    }

    /// Query the server's /models endpoint to find the first available model.
    async fn detect_model(&self) -> anyhow::Result<String> {
        let url = format!("{}/models", self.base_url);
        let resp = reqwest::get(&url).await?;
        let body: serde_json::Value = resp.json().await?;

        // OpenAI-compatible: { "data": [ { "id": "model-name" }, ... ] }
        if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
            if let Some(first) = data.first() {
                if let Some(id) = first.get("id").and_then(|v| v.as_str()) {
                    info!("Auto-detected model: {}", id);
                    return Ok(id.to_string());
                }
            }
        }

        Err(anyhow::anyhow!(
            "No models found. Load a model in Lemonade first."
        ))
    }
}