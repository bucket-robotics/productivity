/// Details about a model.
#[derive(serde::Deserialize, Debug)]
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

/// A chat message.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ChatMessage {
    /// The role - either "user" or "assistant".
    role: String,
    content: String,
    images: Option<Vec<String>>,
}

/// Request for the `/api/chat` endpoint.
#[derive(serde::Serialize, Debug)]
pub struct ChatRequest {
    /// The model name.
    pub model: String,
    /// The chat messages.
    pub messages: Vec<ChatMessage>,
    /// The tools to make available.
    pub tools: bool,
    /// Whether or not to stream the response.
    pub stream: bool,
}

/// Request for the `/api/chat` endpoint.
#[derive(serde::Deserialize, Debug)]
pub struct ChatResponse {
    /// The model name.
    pub model: String,
    pub created_at: String,
    pub messages: Vec<String>,
    pub done: bool,
    pub total_duration: i64,
    pub load_duration: i64,
    pub prompt_eval_count: i64,
    pub prompt_eval_duration: i64,
    pub eval_count: i64,
    pub eval_duration: i64,
}
