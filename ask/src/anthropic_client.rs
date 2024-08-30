use core::str;

use anyhow::Context;
use quick_xml::events::Event;

use crate::llm_response::{LlmResponse, TextOutput, ToolInvocation};
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
pub struct AnthropicStreamResponse {
    pub r#type: String,
    pub index: Option<i64>,
    pub content_block: Option<serde_json::Map<String, serde_json::Value>>,
    pub delta: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The response the Anthropic API returns.
#[derive(serde::Deserialize, Debug)]
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TagType {
    Thought,
    Text,
    Followup,
}

struct TextAccumulator<'a> {
    output: &'a mut Vec<TextOutput>,
    text: String,
    tag_stack: Vec<TagType>,
    bold: bool,
    italic: bool,
    color: Option<String>,
    did_skip: bool,
}

impl TextAccumulator<'_> {
    /// Wrap an output vector in a text accumulator.
    fn new(output: &mut Vec<TextOutput>) -> TextAccumulator {
        TextAccumulator {
            output,
            text: String::with_capacity(8),
            bold: false,
            tag_stack: Vec::with_capacity(2),
            italic: false,
            color: None,
            did_skip: false,
        }
    }

    /// Push a block.
    fn push(&mut self) {
        let should_skip = self.tag_stack.iter().any(|x| match x {
            TagType::Thought | TagType::Followup => true,
            _ => false,
        });
        if should_skip {
            self.text = String::with_capacity(8);
            self.did_skip = true;
            return;
        }

        if !self.text.is_empty() {
            let mut output = TextOutput {
                bold: self.bold,
                italic: self.italic,
                color: self.color.clone(),
                text: String::with_capacity(8),
            };
            std::mem::swap(&mut self.text, &mut output.text);
            self.output.push(output);
        }
    }

    fn push_tag(&mut self, tag: TagType) {
        self.push();
        self.tag_stack.push(tag);
    }

    fn pop_tag_of_type(&mut self, tag: TagType) -> anyhow::Result<()> {
        self.push();

        if let Some(last) = self.tag_stack.pop() {
            if last != tag {
                anyhow::bail!("Expected tag {:?}, got {:?}", tag, last);
            }
        } else {
            anyhow::bail!("Expected tag {:?} to be in the stack, got nothing", tag);
        }
        Ok(())
    }

    fn set_bold(&mut self, state: bool) {
        if self.bold == state {
            return;
        }
        self.push();
        self.bold = state;
    }

    fn set_italic(&mut self, state: bool) {
        if self.italic == state {
            return;
        }
        self.push();
        self.italic = state;
    }

    fn set_color(&mut self, color: Option<String>) {
        if self.color == color {
            return;
        }
        self.push();
        self.color = color;
    }

    fn push_text(&mut self, text: &str) {
        self.text.push_str(if self.did_skip {
            self.did_skip = false;
            text.trim_start()
        } else {
            text
        });
    }
}

fn parse_text(text: &str, output: &mut Vec<TextOutput>) -> anyhow::Result<()> {
    let mut reader = quick_xml::reader::Reader::from_str(text);
    let mut writer = TextAccumulator::new(output);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => panic!("Error at position {}: {:?}", reader.error_position(), e),
            // exits the loop when reaching end of file
            Ok(Event::Eof) => {
                // Make sure the content ends in a newline.
                writer.push_text("\n");
                break;
            }

            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"thought" => {
                    writer.push_tag(TagType::Thought);
                }
                b"followup" => {
                    writer.push_tag(TagType::Followup);
                }
                b"text" => {
                    writer.push_tag(TagType::Text);
                }
                b"bold" => {
                    writer.set_bold(true);
                }
                b"italic" => {
                    writer.set_italic(true);
                }
                b"red" => {
                    writer.set_color(Some("red".to_string()));
                }
                b"green" => {
                    writer.set_color(Some("green".to_string()));
                }
                b"yellow" => {
                    writer.set_color(Some("yellow".to_string()));
                }
                _ => writer.push_text(&format!("<{}>", std::str::from_utf8(&e).unwrap())),
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"thought" => {
                    writer.pop_tag_of_type(TagType::Thought)?;
                }
                b"followup" => {
                    writer.pop_tag_of_type(TagType::Followup)?;
                }
                b"text" => {
                    writer.pop_tag_of_type(TagType::Text)?;
                }
                b"bold" => {
                    writer.set_bold(false);
                }
                b"italic" => {
                    writer.set_italic(false);
                }
                b"red" | b"green" | b"yellow" => {
                    writer.set_color(None);
                }
                _ => writer.push_text(&format!("</{}>", std::str::from_utf8(&e).unwrap())),
            },
            Ok(e) => writer.push_text(std::str::from_utf8(&e).unwrap()),
        }

        buf.clear();
    }

    // Make sure any remaining content is flushed
    writer.push();

    Ok(())
}

/// Convert an Anthropic response to the internal response representation.
fn anthropic_to_internal(response: AnthropicResponse) -> anyhow::Result<LlmResponse> {
    let mut text_blocks = Vec::with_capacity(2);
    let mut raw_text = Vec::with_capacity(2);
    let mut tool_invocations = Vec::new();

    for content_block in response.content {
        match content_block {
            AnthropicContentBlock::Text { text } => {
                raw_text.push(text.clone());
                parse_text(&text, &mut text_blocks)?;
            }
            AnthropicContentBlock::ToolUse { id, name, input } => {
                tool_invocations.push(ToolInvocation { id, name, input });
            }
            _ => {
                anyhow::bail!("Unsupported content block: {:?}", content_block);
            }
        }
    }

    Ok(LlmResponse {
        text: text_blocks,
        raw_text,
        tool_invocations,
    })
}

impl AnthropicClient {
    /// Query the Anthropic API.
    pub async fn query_anthropic(
        self: std::sync::Arc<Self>,
        mut query: AnthropicQuery,
    ) -> anyhow::Result<(LlmResponse, AnthropicQuery)> {
        if tracing::enabled!(tracing::Level::INFO) {
            if let Ok(serialized_query) =
                serde_json::to_string_pretty(&query).context("Serializing query")
            {
                tracing::debug!("Sending query to Anthropic API:\n{}", serialized_query);
            }
        }

        let client = reqwest::Client::new();
        let response = client
            .post(&format!("{}/v1/messages", self.base_url))
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

        let internal_response = anthropic_to_internal(response)?;
        Ok((internal_response, query))
    }
}
