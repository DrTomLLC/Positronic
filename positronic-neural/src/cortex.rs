// positronic-neural/src/cortex.rs
//
// Neural client that talks to Lemonade (or any OpenAI-compatible local LLM).
// Supports smart model selection: routes code tasks to Coder models and
// general tasks to lighter/faster models.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// The types of task we can route to different models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskType {
    /// Code generation, explanation, debugging — routed to Coder model.
    Code,
    /// General questions, chat, advice — routed to lighter/faster model.
    General,
    /// Error diagnosis, troubleshooting — routed to Coder model.
    Debug,
}

impl TaskType {
    /// Classify a prompt into a task type using keyword heuristics.
    pub fn classify(prompt: &str, command_hint: Option<&str>) -> TaskType {
        // If the bang command tells us the type, trust it.
        if let Some(cmd) = command_hint {
            match cmd {
                "fix" | "explain" | "run" | "wasm" => return TaskType::Code,
                "debug" => return TaskType::Debug,
                "ask" => return TaskType::General,
                _ => {} // fall through to heuristic
            }
        }

        let lower = prompt.to_lowercase();

        // Code indicators
        let code_keywords = [
            "function", "class", "struct", "impl", "fn ", "def ", "async ",
            "compile", "compiler", "syntax", "error e0", "cargo", "rustc",
            "npm", "pip", "import", "module", "crate", "package",
            "algorithm", "data structure", "binary", "hex", "regex",
            "git ", "commit", "branch", "merge", "rebase",
            "docker", "container", "yaml", "json", "toml",
            "api", "endpoint", "http", "tcp", "socket",
            "database", "sql", "query", "schema",
            "code", "program", "script", "write a ", "generate a ",
            "```", "fn(", "fn (", "pub fn", "let ", "const ", "mut ",
        ];

        let debug_keywords = [
            "error", "bug", "crash", "fail", "panic", "segfault",
            "traceback", "stack trace", "exception", "abort",
            "permission denied", "not found", "cannot", "unable to",
            "broken", "wrong", "unexpected", "diagnose", "troubleshoot",
        ];

        let code_score: usize = code_keywords.iter()
            .filter(|kw| lower.contains(*kw))
            .count();

        let debug_score: usize = debug_keywords.iter()
            .filter(|kw| lower.contains(*kw))
            .count();

        if debug_score >= 2 {
            TaskType::Debug
        } else if code_score >= 2 {
            TaskType::Code
        } else if debug_score >= 1 && code_score >= 1 {
            TaskType::Code // borderline → Coder handles it better
        } else {
            TaskType::General
        }
    }
}

/// System context injected into every neural prompt to improve response quality.
pub struct SystemContext {
    pub datetime: String,
    pub os: String,
    pub shell: String,
    pub cwd: String,
    pub recent_commands: Vec<String>,
}

impl SystemContext {
    /// Build system context from available environment info.
    pub fn gather(cwd: &str, recent_commands: Vec<String>) -> Self {
        let datetime = chrono::Local::now().format("%A, %B %d, %Y at %H:%M").to_string();

        let os = if cfg!(windows) {
            "Windows".to_string()
        } else if cfg!(target_os = "macos") {
            "macOS".to_string()
        } else {
            "Linux".to_string()
        };

        let shell = if cfg!(windows) {
            std::env::var("COMSPEC").unwrap_or_else(|_| "PowerShell".to_string())
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "bash".to_string())
        };

        SystemContext {
            datetime,
            os,
            shell,
            cwd: cwd.to_string(),
            recent_commands,
        }
    }

    /// Format as a system prompt prefix.
    pub fn to_system_prompt(&self) -> String {
        let mut parts = vec![
            format!("Current date/time: {}", self.datetime),
            format!("OS: {}, Shell: {}", self.os, self.shell),
            format!("Working directory: {}", self.cwd),
        ];
        if !self.recent_commands.is_empty() {
            let cmds = self.recent_commands.iter()
                .map(|c| format!("  $ {}", c))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("Recent commands:\n{}", cmds));
        }
        parts.join("\n")
    }
}

// ════════════════════════════════════════════════════════════════════
// Client
// ════════════════════════════════════════════════════════════════════

/// Client for the Lemonade / OpenAI-compatible local LLM server.
#[derive(Debug, Clone)]
pub struct NeuralClient {
    base_url: String,
    /// Default model name (used for display / fallback).
    default_model: String,
    client: reqwest::Client,
    /// Cached list of available models (refreshed on first use).
    cached_models: std::sync::Arc<tokio::sync::Mutex<Option<Vec<String>>>>,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
}

/// Stop sequences that prevent small models from hallucinating multi-turn dialogue.
const STOP_SEQUENCES: &[&str] = &[
    "User:",
    "\nUser:",
    "Human:",
    "\nHuman:",
    "\n\nUser",
    "\n\nHuman",
];

impl NeuralClient {
    /// Create a new client pointing at the Lemonade server.
    pub fn new(base_url: &str, default_model: &str) -> Self {
        NeuralClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            default_model: default_model.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            cached_models: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// List available models from the server. Caches after first call.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        {
            let cache = self.cached_models.lock().await;
            if let Some(ref models) = *cache {
                return Ok(models.clone());
            }
        }

        let url = format!("{}/models", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let body: ModelsResponse = resp.json().await?;
        let models: Vec<String> = body.data.iter().map(|m| m.id.clone()).collect();

        {
            let mut cache = self.cached_models.lock().await;
            *cache = Some(models.clone());
        }

        Ok(models)
    }

    /// Invalidate the model cache (call after model hot-swap).
    pub async fn refresh_models(&self) {
        let mut cache = self.cached_models.lock().await;
        *cache = None;
    }

    /// Select the best model for a task type from available models.
    pub async fn select_model(&self, task_type: TaskType) -> Result<String> {
        let models = self.list_models().await?;

        if models.is_empty() {
            return Err(anyhow!("No models available from Lemonade"));
        }

        // Single model? No choice.
        if models.len() == 1 {
            return Ok(models[0].clone());
        }

        let chosen = match task_type {
            TaskType::Code | TaskType::Debug => {
                // Prefer Coder models for code/debug tasks.
                models.iter()
                    .find(|m| {
                        let lower = m.to_lowercase();
                        lower.contains("coder") || lower.contains("code")
                            || lower.contains("starcoder") || lower.contains("deepseek")
                    })
                    .or_else(|| {
                        // Fallback: prefer the larger model (more params).
                        models.iter().max_by_key(|m| Self::estimate_model_size(m))
                    })
            }
            TaskType::General => {
                // Prefer faster/lighter models for general chat.
                models.iter()
                    .find(|m| {
                        let lower = m.to_lowercase();
                        lower.contains("olmo") || lower.contains("chat")
                            || lower.contains("llama") || lower.contains("mistral")
                            || lower.contains("phi")
                    })
                    .or_else(|| {
                        // Fallback: prefer the smaller model (faster).
                        models.iter().min_by_key(|m| Self::estimate_model_size(m))
                    })
            }
        };

        Ok(chosen.cloned().unwrap_or_else(|| models[0].clone()))
    }

    /// Rough heuristic to estimate model size from its name.
    /// Returns a sortable score (higher = larger).
    fn estimate_model_size(name: &str) -> u64 {
        let lower = name.to_lowercase();
        for part in lower.split(|c: char| !c.is_alphanumeric()) {
            if part.ends_with('b') {
                if let Ok(n) = part.trim_end_matches('b').parse::<u64>() {
                    return n;
                }
                if let Ok(n) = part.trim_end_matches('b').parse::<f64>() {
                    return (n * 10.0) as u64;
                }
            }
        }
        1 // unknown → assume small
    }

    /// Max tokens scaled by task type — keeps small models from rambling.
    fn max_tokens_for(task_type: TaskType) -> u32 {
        match task_type {
            TaskType::General => 256,
            TaskType::Code => 512,
            TaskType::Debug => 384,
        }
    }

    /// Send a prompt with automatic model selection.
    /// Injects system context and selects the best model for the task.
    pub async fn ask_smart(
        &self,
        prompt: &str,
        task_type: TaskType,
        context: Option<&SystemContext>,
    ) -> Result<String> {
        let model = self.select_model(task_type).await?;

        let system_msg = if let Some(ctx) = context {
            format!(
                "You are a helpful terminal assistant. Be concise and practical. \
                 Give exact commands when applicable. Answer the user's question \
                 directly, then stop. Do NOT simulate follow-up questions or \
                 generate fake User/Assistant dialogue.\n\n{}",
                ctx.to_system_prompt()
            )
        } else {
            "You are a helpful terminal assistant. Be concise and practical. \
             Give exact commands when applicable. Answer the user's question \
             directly, then stop. Do NOT simulate follow-up questions or \
             generate fake User/Assistant dialogue."
                .to_string()
        };

        let max_tokens = Self::max_tokens_for(task_type);
        self.send_chat_with_stops(&model, &system_msg, prompt, max_tokens).await
    }

    /// Original simple ask — uses first available model, no context injection.
    pub async fn ask(&self, prompt: &str) -> Result<String> {
        let models = self.list_models().await?;
        let model = models.first()
            .ok_or_else(|| anyhow!("No models available"))?;

        let system = "You are a helpful terminal assistant. Be concise and practical. \
                      Answer the question directly, then stop.";
        self.send_chat_with_stops(model, system, prompt, 256).await
    }

    /// Ask with a specific model name.
    pub async fn ask_with_model(&self, prompt: &str, model: &str) -> Result<String> {
        let system = "You are a helpful terminal assistant. Be concise and practical. \
                      Answer the question directly, then stop.";
        self.send_chat_with_stops(model, system, prompt, 256).await
    }

    /// Chat completion with stop sequences and post-processing.
    async fn send_chat_with_stops(
        &self,
        model: &str,
        system: &str,
        user: &str,
        max_tokens: u32,
    ) -> Result<String> {
        let url = format!("{}/chat/completions", self.base_url);

        let stop_seqs: Vec<String> = STOP_SEQUENCES.iter().map(|s| s.to_string()).collect();

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user.to_string(),
                },
            ],
            max_tokens,
            temperature: 0.3,
            stop: Some(stop_seqs),
        };

        let resp = self.client.post(&url).json(&request).send().await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Lemonade API error {}: {}", status, body));
        }

        let body: ChatResponse = resp.json().await?;
        let raw = body.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow!("Empty response from Lemonade"))?;

        // Post-process: strip any hallucinated multi-turn dialogue
        Ok(Self::truncate_hallucinated_turns(&raw))
    }

    /// Legacy send_chat — kept for backward compat, routes through new method.
    #[allow(dead_code)]
    async fn send_chat(&self, model: &str, system: &str, user: &str) -> Result<String> {
        self.send_chat_with_stops(model, system, user, 256).await
    }

    /// Strip hallucinated multi-turn conversation from the response.
    /// Small models (1B–3B) frequently generate fake "User:" / "Assistant:" turns.
    fn truncate_hallucinated_turns(response: &str) -> String {
        let markers = [
            "\nUser:",
            "\nHuman:",
            "\nAssistant:",
            "\n\nUser:",
            "\n\nHuman:",
            "\n\nAssistant:",
            "\n\n---\n",
        ];

        let mut end = response.len();
        for marker in &markers {
            if let Some(pos) = response.find(marker) {
                if pos > 0 && pos < end {
                    end = pos;
                }
            }
        }

        response[..end].trim().to_string()
    }
}

// ════════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_classification_code() {
        assert_eq!(
            TaskType::classify("Write a Rust function to sort a vector", None),
            TaskType::Code
        );
        assert_eq!(
            TaskType::classify("generate a Python script that reads JSON", None),
            TaskType::Code
        );
    }

    #[test]
    fn test_task_classification_debug() {
        assert_eq!(
            TaskType::classify("I got a permission denied error when running cargo build", None),
            TaskType::Debug
        );
    }

    #[test]
    fn test_task_classification_general() {
        assert_eq!(
            TaskType::classify("What is the capital of France?", None),
            TaskType::General
        );
        assert_eq!(
            TaskType::classify("Tell me a joke", None),
            TaskType::General
        );
    }

    #[test]
    fn test_command_hint_overrides() {
        assert_eq!(
            TaskType::classify("what is this", Some("explain")),
            TaskType::Code
        );
        assert_eq!(
            TaskType::classify("what is this", Some("debug")),
            TaskType::Debug
        );
        assert_eq!(
            TaskType::classify("write code for me", Some("ask")),
            TaskType::General
        );
    }

    #[test]
    fn test_model_size_estimation() {
        assert!(NeuralClient::estimate_model_size("llama-7B") > NeuralClient::estimate_model_size("phi-1B"));
        assert!(NeuralClient::estimate_model_size("deepseek-coder-33b") > NeuralClient::estimate_model_size("olmo-1b"));
    }

    #[test]
    fn test_truncate_hallucinated_turns() {
        let clean = "The answer is 42.";
        assert_eq!(NeuralClient::truncate_hallucinated_turns(clean), "The answer is 42.");

        let dirty = "The answer is 42.\n\nUser: What about 43?\n\nAssistant: That too.";
        assert_eq!(NeuralClient::truncate_hallucinated_turns(dirty), "The answer is 42.");

        let dirty2 = "Hello world.\nUser: follow up\nAssistant: more garbage";
        assert_eq!(NeuralClient::truncate_hallucinated_turns(dirty2), "Hello world.");

        let dirty3 = "Some response.\n\nHuman: fake question\n\nAssistant: fake answer";
        assert_eq!(NeuralClient::truncate_hallucinated_turns(dirty3), "Some response.");
    }

    #[test]
    fn test_max_tokens_scaling() {
        assert_eq!(NeuralClient::max_tokens_for(TaskType::General), 256);
        assert_eq!(NeuralClient::max_tokens_for(TaskType::Code), 512);
        assert_eq!(NeuralClient::max_tokens_for(TaskType::Debug), 384);
    }
}