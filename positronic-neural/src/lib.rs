use async_trait::async_trait;
use anyhow::Result;

#[async_trait]
pub trait NeuralBackend {
    /// Takes a broken command and returns a fixed one.
    async fn fix_command(&self, broken_command: &str) -> Result<String>;
    
    /// Explains what a command does in plain English.
    async fn explain_command(&self, command: &str) -> Result<String>;
}

pub struct LemonadeClient {
    base_url: String,
    // Client state...
}

// Implementation stubs for later...
