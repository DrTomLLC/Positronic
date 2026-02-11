use crate::privacy::PrivacyGuard;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
};
use tracing::{info, warn};

/// The Cortex is the interface to the Neural Processing Unit (or local server).
pub struct NeuralClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl std::fmt::Debug for NeuralClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NeuralClient")
            .field("model", &self.model)
            .finish()
    }
}

impl NeuralClient {
    /// Connect to a local Lemonade/Llama.cpp server.
    pub fn new(base_url: &str, model: &str) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key("sk-no-key-required");
        let client = Client::with_config(config);

        info!("Cortex connected to: {}", base_url);

        Self {
            client,
            model: model.to_string(),
        }
    }

    /// Ask the AI a question, with PII scrubbing.
    pub async fn ask(&self, prompt: &str) -> anyhow::Result<String> {
        let safe_prompt = PrivacyGuard::scrub(prompt);
        if safe_prompt != prompt {
            warn!("Original prompt contained PII. scrubbed.");
        }

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content("You are Positronic, an intelligent terminal assistant. Be concise.")
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
            .unwrap_or_else(|| "No response.".to_string());

        Ok(answer)
    }
}
