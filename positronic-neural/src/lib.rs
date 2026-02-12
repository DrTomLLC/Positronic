use anyhow::{Context, Result};
use async_trait::async_trait;
pub mod cortex;
pub mod privacy;
pub mod reflex;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
};

/// The interface for any NPU backend.
#[async_trait]
pub trait NeuralBackend: Send + Sync {
    async fn fix_command(&self, broken_command: &str) -> Result<String>;
    async fn explain_command(&self, command: &str) -> Result<String>;
}

pub struct LemonadeClient {
    client: Client<OpenAIConfig>,
    model_name: String,
}

impl LemonadeClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url)
            .with_api_key("dummy-key");

        Self {
            client: Client::with_config(config),
            model_name: model.to_string(),
        }
    }
}

#[async_trait]
impl NeuralBackend for LemonadeClient {
    async fn fix_command(&self, broken_command: &str) -> Result<String> {
        // Construct messages using specific builder types for System/User
        let messages = vec![
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content("You are a terminal expert. Fix the user's command. Output ONLY the fixed command.")
                    .build()?
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(broken_command)
                    .build()?
            ),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model_name)
            .messages(messages)
            .build()?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .context("Failed to contact NPU")?;

        let fixed = response
            .choices
            .first()
            .context("NPU returned no choices")?
            .message
            .content
            .clone()
            .context("NPU returned empty content")?;

        Ok(fixed.trim().to_string())
    }

    async fn explain_command(&self, command: &str) -> Result<String> {
        let messages = vec![
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content("Explain this command briefly in one sentence.")
                    .build()?,
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(command)
                    .build()?,
            ),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model_name)
            .messages(messages)
            .build()?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .context("Failed to contact NPU")?;

        let explanation = response
            .choices
            .first()
            .context("NPU returned no choices")?
            .message
            .content
            .clone()
            .unwrap_or_default();

        Ok(explanation)
    }
}
