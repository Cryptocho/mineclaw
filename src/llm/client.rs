use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::config::LlmConfig;
use crate::error::{Error, Result};
use crate::models::MessageRole;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl From<(MessageRole, String)> for ChatMessage {
    fn from((role, content): (MessageRole, String)) -> Self {
        let role_str = match role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::ToolCall => "assistant",
            MessageRole::ToolResult => "tool",
        };
        Self {
            role: role_str.to_string(),
            content,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f64,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

pub struct OpenAiProvider {
    config: LlmConfig,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(config: LlmConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String> {
        info!(
            "LLM chat request: model={}, message_count={}",
            self.config.model,
            messages.len()
        );

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages,
            temperature: self.config.temperature,
        };

        let url = format!("{}/chat/completions", self.config.base_url);

        info!("Sending request to LLM API");
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(Error::Llm(format!(
                "LLM request failed: {} - {}",
                status, text
            )));
        }

        info!("LLM API response received, status: {}", response.status());

        let completion: ChatCompletionResponse = response.json().await?;

        info!("LLM response parsed, choices: {}", completion.choices.len());

        completion
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| Error::Llm("No response from LLM".into()))
    }
}

pub fn create_provider(config: LlmConfig) -> Arc<dyn LlmProvider> {
    match config.provider.to_lowercase().as_str() {
        "openai" => Arc::new(OpenAiProvider::new(config)),
        _ => Arc::new(OpenAiProvider::new(config)),
    }
}
