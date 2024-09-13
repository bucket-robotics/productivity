use std::collections::HashMap;

/// Details about a model.
#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
pub struct ModelDetails {
    /// The format of the model.
    pub format: String,
    /// The family of the model.
    pub family: String,
    /// The families of the model.
    pub families: Option<Vec<String>>,
    /// The size of the parameters of the model.
    pub parameter_size: String,
    /// The quantization level of the model.
    pub quantization_level: String,
}

/// Information about a model.
#[derive(serde::Deserialize, Debug)]
#[allow(dead_code)]
pub struct Model {
    /// The name of the model.
    pub name: String,
    /// The last time the model was modified.
    pub modified_at: String,
    /// The size of the model.
    pub size: i64,
    /// The digest of the model.
    pub digest: String,
    /// Details about the model.
    pub details: ModelDetails,
}

/// The response from the `/api/tags` endpoint.
#[derive(serde::Deserialize, Debug)]
pub struct TagsResponse {
    pub models: Vec<Model>,
}

/// Request for the `/api/pull` endpoint.
#[derive(serde::Serialize, Debug)]
pub struct PullRequest {
    pub model: String,
    pub insecure: bool,
    pub stream: bool,
}

/// Request for the `/api/pull` endpoint.
#[derive(serde::Deserialize, Debug)]
pub struct PullResponse {
    pub status: String,
}

/// A tool call.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum ToolCall {
    #[serde(rename = "function")]
    Function {
        /// The name of the tool.
        name: String,
        /// The parameters to pass to the tool.
        arguments: serde_json::Value,
    },
}

/// A chat message.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatMessage {
    /// The role - "system", "user", or "assistant".
    pub role: String,
    /// The text content of the message.
    pub content: String,
    /// The images attached to the message.
    pub images: Option<Vec<String>>,
    /// The tools to invoke.
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// A tool definition.
#[derive(serde::Serialize, Clone, Debug)]
pub enum ToolDefinition {
    #[serde(rename = "function")]
    Function {
        /// The name of the function.
        name: String,
        /// A description of the function.
        description: String,
        /// The parameters of the function.
        parameters: serde_json::Value,
    },
}

/// A tool definition.
#[derive(serde::Serialize, Clone, Debug)]
pub struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    #[serde(flatten)]
    definition: ToolDefinition,
}

impl From<ToolDefinition> for OllamaTool {
    fn from(definition: ToolDefinition) -> Self {
        OllamaTool {
            tool_type: "function".to_string(),
            definition,
        }
    }
}

/// Request for the `/api/chat` endpoint.
#[derive(serde::Serialize, Clone, Debug)]
pub struct ChatRequest {
    /// The model name.
    pub model: String,
    /// The chat messages.
    pub messages: Vec<ChatMessage>,
    /// The tools to make available.
    pub tools: Vec<OllamaTool>,
    /// Whether or not to stream the response.
    pub stream: bool,
}

/// Request for the `/api/chat` endpoint.
#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[allow(dead_code)]
pub struct ChatResponse {
    /// The model name.
    pub model: String,
    /// The time the response was created.
    pub created_at: String,
    /// The chat messages.
    pub message: ChatMessage,
    /// Whether the conversation is done.
    pub done: bool,
    /// The reason the conversation is done.
    pub done_reason: String,
    /// The duration of the request.
    pub total_duration: i64,
    /// The time spent loading the model.
    pub load_duration: i64,
    pub prompt_eval_count: i64,
    pub prompt_eval_duration: i64,
    pub eval_count: i64,
    pub eval_duration: i64,
}

impl crate::llm_client::LlmQuery for ChatRequest {
    fn create_query(system_prompt: String) -> Self {
        let mut tools = vec![];
        for tool in crate::tools::rust_tools::get_rust_tools() {
            let definition = tool.get_definition();
            let mut properties = HashMap::with_capacity(1);
            for (key, value) in definition.input_schema["properties"].as_object().unwrap() {
                let json_type = &value["type"];
                let type_string = if let Some(array) = json_type.as_array() {
                    array
                        .iter()
                        .filter_map(|x| x.as_str())
                        .find(|&x| x != "null")
                        .unwrap()
                } else {
                    json_type.as_str().unwrap()
                };
                properties.insert(
                    key,
                    serde_json::json!({
                        "type": type_string,
                        "description": value["description"],
                        "enum": value["enum"],
                    }),
                );
            }

            let parameters = serde_json::json!({
                "type": definition.input_schema["type"],
                "required": definition.input_schema["required"],
                "properties": properties,
            });

            tools.push(OllamaTool::from(ToolDefinition::Function {
                name: definition.name,
                description: definition.description,
                parameters,
            }));
        }

        let mut messages = Vec::with_capacity(2);
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
            images: None,
            tool_calls: None,
        });

        ChatRequest {
            model: String::new(),
            messages,
            tools,
            stream: false,
        }
    }

    fn add_question(&mut self, question: String) {
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: question,
            images: None,
            tool_calls: None,
        });
    }

    fn add_tool_results(&mut self, tool_results: Vec<(String, String)>) {
        for (_, result) in tool_results {
            self.messages.push(ChatMessage {
                role: "tool".to_string(),
                content: result,
                images: None,
                tool_calls: None,
            });
        }
    }
}
