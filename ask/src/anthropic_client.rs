use std::sync::Arc;

use anyhow::Context;

use crate::anthropic_tools::{Tool, ToolDefinition};

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
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct AnthropicStreamResponse {
    pub r#type: String,
    pub index: Option<i64>,
    pub content_block: Option<serde_json::Map<String, serde_json::Value>>,
    pub delta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl AnthropicClient {
    pub async fn query_anthropic(
        self: std::sync::Arc<Self>,
        query: AnthropicQuery,
        message_queue: tokio::sync::mpsc::Sender<AnthropicStreamResponse>,
    ) -> anyhow::Result<()> {
        if tracing::enabled!(tracing::Level::INFO) {
            if let Ok(serialized_query) =
                serde_json::to_string_pretty(&query).context("Serializing query")
            {
                tracing::debug!("Sending query to Anthropic API:\n{}", serialized_query);
            }
        }

        let client = reqwest::Client::new();
        let mut response = client
            .post(&format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.token)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&query)
            .send()
            .await
            .context("Querying Anthropic")?;

        assert!(query.stream, "We only support streaming queries right now");
        if !response.status().is_success() {
            let body: serde_json::Value = response.json().await.context("Reading response body")?;
            tracing::error!("Error querying API: {body:?}");
            anyhow::bail!("Failed to query Anthropic: {:?}", body);
        }

        let mut partial_data_buffer = String::new();
        while let Some(chunk) = response.chunk().await.context("Reading chunk")? {
            let chunk_str = String::from_utf8_lossy(&chunk);
            for line in chunk_str.lines() {
                let line = if partial_data_buffer.is_empty() {
                    line.to_string()
                } else {
                    let new_line = format!("{}{}", &partial_data_buffer, line);
                    partial_data_buffer.clear();
                    new_line
                };

                if let Some(data) = line.strip_prefix("data: ") {
                    let Ok(response) = serde_json::from_str::<AnthropicStreamResponse>(data)
                        .context("Parsing response")
                    else {
                        partial_data_buffer = line.to_string();
                        continue;
                    };
                    let should_break = response.r#type == "message_stop";
                    message_queue
                        .send(response)
                        .await
                        .context("Sending to message queue")?;

                    if should_break {
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

/// A content block processor.
///
/// Implementors of this trait are responsible for processing content block deltas as they stream in.
pub trait ContentBlockProcessor {
    /// Process a delta message.
    fn process_delta(
        &mut self,
        delta: serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<()>;

    /// Finalize the content block - e.g. flush the output.
    fn finalize(&mut self) -> anyhow::Result<()>;

    /// Get the content block to put back into the messages stream - e.g. a `tool_use`.
    fn get_original_content_block(&self) -> anyhow::Result<AnthropicContentBlock>;
    /// Get the content block to put back into the messages stream - e.g. a `tool_result`.
    async fn get_user_content_block(&self) -> Option<anyhow::Result<AnthropicContentBlock>>;
}

/// A text content block.
pub struct TextContentBlock {
    printer: super::printer::Printer<std::io::Stdout>,
    accumulated_text: String,
}

impl TextContentBlock {
    /// Construct a text content block.
    ///
    /// # Arguments
    /// `content_block` - The content block to construct from, for example:
    /// ```json
    /// {
    ///     "text": String(""),
    ///     "type": String("text"),
    /// },
    /// ```
    pub fn new(content_block: &serde_json::Map<String, serde_json::Value>) -> anyhow::Result<Self> {
        let mut printer = super::printer::Printer::new(std::io::stdout());
        let mut accumulated_text = String::with_capacity(64);
        if let Some(text) = content_block.get("text") {
            let text = text.as_str().context("Text field was not a string")?;
            accumulated_text.push_str(text);
            printer.print(text)?;
        }

        Ok(Self {
            printer,
            accumulated_text,
        })
    }
}

impl ContentBlockProcessor for TextContentBlock {
    fn process_delta(
        &mut self,
        delta: serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<()> {
        let delta_type = delta.get("type").context("No type in the delta")?;
        if delta_type.as_str().context("Type is not a string")? != "text_delta" {
            return Ok(());
        }

        let text = delta
            .get("text")
            .context("No text in the delta")?
            .as_str()
            .context("Text is not a string")?;

        self.accumulated_text.push_str(text);
        self.printer.print(text)?;
        Ok(())
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        self.printer.flush();
        Ok(())
    }

    fn get_original_content_block(&self) -> anyhow::Result<AnthropicContentBlock> {
        Ok(AnthropicContentBlock::Text {
            text: self.accumulated_text.clone(),
        })
    }

    async fn get_user_content_block(&self) -> Option<anyhow::Result<AnthropicContentBlock>> {
        None
    }
}

/// A tool use content block.
pub struct ToolUseContentBlock {
    /// ID of the tool invocation.
    id: String,
    /// Name of the tool being called.
    name: String,
    /// The implementation of the tool being called.
    tool: Arc<dyn Tool>,
    /// Accumulated JSON for the tool invocation.
    accumulated_json: String,
}

impl ToolUseContentBlock {
    pub fn new(
        tool: Arc<dyn Tool>,
        content_block: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<Self> {
        let Some(id) = content_block.get("id") else {
            anyhow::bail!("No ID in tool invocation");
        };
        let Some(name) = content_block.get("name") else {
            anyhow::bail!("No Name in tool invocation");
        };
        Ok(Self {
            name: name.as_str().context("Name is not a string")?.to_string(),
            id: id.as_str().context("ID is not a string")?.to_string(),
            tool,
            accumulated_json: String::with_capacity(64),
        })
    }

    fn get_input_value(&self) -> anyhow::Result<serde_json::Value> {
        serde_json::from_str(&self.accumulated_json).with_context(|| {
            format!(
                "Failed to parse accumulated JSON: {}",
                &self.accumulated_json
            )
        })
    }

    /// Invoke the underlying tool using the accumulated JSON.
    async fn invoke_tool(&self) -> anyhow::Result<String> {
        let prereq = self.tool.get_prequisites();
        for binary in &prereq.binaries {
            // Check if the binary is in the PATH.
            if which::which(binary).is_err() {
                tracing::warn!("Tried in call binary that is not installed: {}", &binary);
                return Err(anyhow::anyhow!(
                    "Binary {:?} not found in PATH - you can suggest that the user installs it or attempt to install it yourself using a tool",
                    binary
                ));
            }
        }
        tracing::info!(
            "Invoking tool: {:?}, Input is:\n{}",
            &self.name,
            &self.accumulated_json
        );
        let result = self
            .tool
            .clone()
            .run(self.get_input_value().context("Parsing tool input")?)
            .await;
        if let Err(e) = &result {
            eprintln!("Tool invocation failed: {e:?}");
        }

        result
    }
}

impl ContentBlockProcessor for ToolUseContentBlock {
    fn process_delta(
        &mut self,
        delta: serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<()> {
        let delta_type = delta.get("type").context("No type in the delta")?;
        if delta_type
            .as_str()
            .context("Type is not an input_json_delta")?
            != "input_json_delta"
        {
            return Ok(());
        }

        let partial_json = delta
            .get("partial_json")
            .context("No partial_json in the delta")?;

        self.accumulated_json.push_str(
            partial_json
                .as_str()
                .context("partial_json wasn't a string")?,
        );

        Ok(())
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn get_original_content_block(&self) -> anyhow::Result<AnthropicContentBlock> {
        Ok(AnthropicContentBlock::ToolUse {
            id: self.id.clone(),
            name: self.name.clone(),
            input: self.get_input_value()?,
        })
    }

    async fn get_user_content_block(&self) -> Option<anyhow::Result<AnthropicContentBlock>> {
        let content = match self
            .invoke_tool()
            .await
            .with_context(|| format!("Invoking {}", self.name))
        {
            Ok(content) => content,
            Err(e) => {
                tracing::error!("Error: {}", &e);
                format!("An error occured invoking the tool: {e}")
            }
        };

        Some(Ok(AnthropicContentBlock::ToolResult {
            tool_use_id: self.id.clone(),
            content,
        }))
    }
}

/// A content block that can be iteratively processed as data comes in.
pub enum ContentBlock {
    Text(TextContentBlock),
    ToolUse(ToolUseContentBlock),
}

impl ContentBlockProcessor for ContentBlock {
    fn process_delta(
        &mut self,
        delta: serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<()> {
        match self {
            ContentBlock::Text(block) => block.process_delta(delta),
            ContentBlock::ToolUse(block) => block.process_delta(delta),
        }
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        match self {
            ContentBlock::Text(block) => block.finalize(),
            ContentBlock::ToolUse(block) => block.finalize(),
        }
    }

    fn get_original_content_block(&self) -> anyhow::Result<AnthropicContentBlock> {
        match self {
            ContentBlock::Text(block) => block.get_original_content_block(),
            ContentBlock::ToolUse(block) => block.get_original_content_block(),
        }
    }

    async fn get_user_content_block(&self) -> Option<anyhow::Result<AnthropicContentBlock>> {
        match self {
            ContentBlock::Text(block) => block.get_user_content_block().await,
            ContentBlock::ToolUse(block) => block.get_user_content_block().await,
        }
    }
}
