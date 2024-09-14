use core::str;

use anyhow::Context;

use crate::llm_client::{LlmResponse, ToolInvocation};
use crate::tools::ToolDefinition;

/// A query for the Anthropic messages API.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct AnthropicQuery {
    /// The model to use.
    ///
    /// See <https://docs.anthropic.com/en/docs/about-claude/models>
    pub model: String,
    pub max_tokens: i64,
    pub temperature: f32,
    pub top_p: f32,
    pub stop_sequences: Option<Vec<String>>,
    pub stream: bool,
    pub system: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub messages: Vec<AnthropicMessage>,
}

impl Default for AnthropicQuery {
    fn default() -> Self {
        AnthropicQuery {
            model: "claude-3-5-sonnet-20240620".to_string(),
            max_tokens: 2048,
            temperature: 0.2,
            top_p: 0.1,
            stop_sequences: None,
            stream: true,
            system: None,
            tools: vec![],
            messages: Vec::new(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Either plain text or a vector of content blocks.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TextOrContentVector {
    /// Plain text.
    Text(String),
    /// A vector of content blocks.
    Content(Vec<AnthropicContentBlock>),
}

/// An Anthropic message.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct AnthropicMessage {
    /// This is either "user" or "system".
    pub role: String,
    /// This is the content of the message.
    pub content: TextOrContentVector,
}

/// A basic client for the Anthropic API.
pub struct AnthropicClient {
    pub base_url: String,
    pub token: String,
}

/// The response the Anthropic API returns from a stream.
#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
pub struct AnthropicStreamResponse {
    pub r#type: String,
    pub index: Option<i64>,
    pub content_block: Option<serde_json::Map<String, serde_json::Value>>,
    pub delta: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The response the Anthropic API returns.
#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
pub struct AnthropicResponse {
    /// The content blocks.
    pub content: Vec<AnthropicContentBlock>,
    /// The ID of the message.
    pub id: String,
    /// The model used to generate the response.
    pub model: String,
    /// The role of the responder.
    pub role: String,
    /// The reason the response stopped.
    pub stop_reason: String,
    /// The stop sequence.
    pub stop_sequence: Option<String>,
    /// The type of the response.
    pub r#type: String,
    /// Usage statistics.
    pub usage: serde_json::Map<String, serde_json::Value>,
}

/// Convert an Anthropic response to the internal response representation.
fn anthropic_to_internal(response: AnthropicResponse) -> anyhow::Result<LlmResponse> {
    let mut text_blocks = Vec::with_capacity(2);
    let mut tool_invocations = Vec::new();

    for content_block in response.content {
        match content_block {
            AnthropicContentBlock::Text { text } => {
                crate::response_parsing::parse_text(&text, &mut text_blocks);
            }
            AnthropicContentBlock::ToolUse { id, name, input } => {
                tool_invocations.push(ToolInvocation { id, name, input });
            }
            AnthropicContentBlock::ToolResult { .. } => {
                anyhow::bail!("Unsupported content block: {:?}", content_block);
            }
        }
    }

    Ok(LlmResponse {
        text: text_blocks,
        tool_invocations,
    })
}

impl crate::llm_client::LlmQuery for AnthropicQuery {
    fn create_query(system_prompt: String) -> Self {
        let mut tool_map = std::collections::HashMap::<String, _>::new();
        let mut tool_definitions = vec![];
        for tool in crate::tools::rust_tools::get_rust_tools() {
            let definition = tool.get_definition();
            tool_map.insert(definition.name.to_string(), tool);
            tool_definitions.push(definition);
        }

        AnthropicQuery {
            messages: Vec::with_capacity(1),
            system: Some(system_prompt),
            tools: tool_definitions,
            stream: false,
            ..Default::default()
        }
    }

    fn add_question(&mut self, question: String) {
        self.messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: TextOrContentVector::Text(question),
        });
    }

    fn add_tool_results(&mut self, tool_results: Vec<(String, String)>) {
        let mut user_content = Vec::with_capacity(tool_results.len());
        for (invocation_id, tool_response) in tool_results {
            user_content.push(AnthropicContentBlock::ToolResult {
                tool_use_id: invocation_id,
                content: tool_response,
            });
        }

        self.messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: TextOrContentVector::Content(user_content),
        });
    }
}

impl crate::llm_client::LlmClient for AnthropicClient {
    type Query = AnthropicQuery;

    /// Query the Anthropic API.
    async fn query(&self, mut query: Self::Query) -> anyhow::Result<(LlmResponse, Self::Query)> {
        if tracing::enabled!(tracing::Level::DEBUG) {
            if let Ok(serialized_query) =
                serde_json::to_string_pretty(&query).context("Serializing query")
            {
                tracing::debug!("Sending query to Anthropic API:\n{}", serialized_query);
            }
        }

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.token)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&query)
            .send()
            .await
            .context("Querying Anthropic")?;

        assert!(
            !query.stream,
            "We don't support streaming queries right now"
        );

        if !response.status().is_success() {
            let body: serde_json::Value = response.json().await.context("Reading response body")?;
            tracing::error!("Error querying API: {body:?}");
            anyhow::bail!("Failed to query Anthropic: {:?}", body);
        }

        let response: AnthropicResponse =
            response.json().await.context("Deserializing response")?;

        query.messages.push(AnthropicMessage {
            role: "assistant".to_string(),
            content: TextOrContentVector::Content(response.content.clone()),
        });

        Ok((anthropic_to_internal(response)?, query))
    }
}
