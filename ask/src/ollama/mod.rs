//! Ollama-related code.

use anyhow::Context;
use data_types::{ChatResponse, ToolCall};

use crate::llm_client::{LlmResponse, ToolInvocation};

mod data_types;

/// A basic client for the Anthropic API.
pub struct OllamaClient {
    /// The base URL for the API.
    pub base_url: String,
    /// The model to use.
    pub model: String,
}

impl OllamaClient {
    /// Get the currently downloaded models.
    pub async fn get_tags(&self) -> anyhow::Result<data_types::TagsResponse> {
        let url = format!("{}/api/tags", self.base_url);
        let response = reqwest::get(&url)
            .await?
            .error_for_status()?
            .json::<data_types::TagsResponse>()
            .await?;
        Ok(response)
    }

    /// Pull a model.
    pub async fn pull(&self, name: &str) -> anyhow::Result<data_types::PullResponse> {
        let url = format!("{}/api/pull", self.base_url);
        let request = data_types::PullRequest {
            model: name.to_string(),
            insecure: false,
            stream: false,
        };
        let response = reqwest::Client::new()
            .post(&url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<data_types::PullResponse>()
            .await?;

        if &response.status != "success" {
            anyhow::bail!("Failed to pull model: {:?}", response);
        }
        Ok(response)
    }

    /// Pull a model if it's not already downloaded.
    pub async fn pull_if_needed(&self, name: &str) -> anyhow::Result<()> {
        let tags = self.get_tags().await?;
        if !tags.models.iter().any(|model| model.name == name) {
            println!("Downloading model {name}...");
            self.pull(name).await?;
            println!("Model {name} downloaded.");
        }
        Ok(())
    }
}

/// Convert an Ollama response to the internal response representation.
fn ollama_to_internal(response: &ChatResponse) -> anyhow::Result<LlmResponse> {
    let mut text_blocks = Vec::with_capacity(2);
    let mut tool_invocations = Vec::new();

    if !response.message.content.is_empty() {
        crate::response_parsing::parse_text(&response.message.content, &mut text_blocks)?;
    }

    if let Some(tool_calls) = &response.message.tool_calls {
        for ToolCall::Function { name, arguments } in tool_calls {
            tool_invocations.push(ToolInvocation {
                id: name.clone(),
                name: name.clone(),
                input: arguments.clone(),
            });
        }
    }

    Ok(LlmResponse {
        text: text_blocks,
        tool_invocations,
    })
}

impl crate::llm_client::LlmClient for OllamaClient {
    type Query = data_types::ChatRequest;

    /// Query the Anthropic API.
    async fn query(&self, mut query: Self::Query) -> anyhow::Result<(LlmResponse, Self::Query)> {
        self.pull_if_needed(&self.model).await?;

        if tracing::enabled!(tracing::Level::INFO) {
            if let Ok(serialized_query) =
                serde_json::to_string_pretty(&query.messages).context("Serializing query")
            {
                tracing::info!("Sending query to Ollama:\n{}", serialized_query);
            }
        }

        query.model = self.model.clone();

        let url = format!("{}/api/chat", self.base_url);
        let response = reqwest::Client::new()
            .post(&url)
            .json(&query)
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;

        if tracing::enabled!(tracing::Level::INFO) {
            if let Ok(serialized_response) =
                serde_json::to_string_pretty(&response).context("Serializing response")
            {
                tracing::info!("Received from Ollama:\n{}", serialized_response);
            }
        }

        query.messages.push(response.message.clone());

        Ok((ollama_to_internal(&response)?, query))
    }
}
