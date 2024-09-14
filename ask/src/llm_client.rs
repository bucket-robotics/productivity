/// A text output section.
pub enum TextOutput {
    Text(String),
    Bold(String),
    Italic(String),
    InlineCode(String),
    CodeBlock { language: String, content: String },
    Newline,
}

/// A tool invocation.
pub struct ToolInvocation {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// LLM response structure.
pub struct LlmResponse {
    /// The LLMs text response.
    pub text: Vec<TextOutput>,
    /// A list of tool invocations.
    pub tool_invocations: Vec<ToolInvocation>,
}

pub trait LlmQuery: serde::Serialize + Clone {
    /// Create a query.
    fn create_query(system_prompt: String) -> Self;
    /// Add a question to the query.
    fn add_question(&mut self, question: String);
    /// Add tool use results to a query.
    fn add_tool_results(&mut self, tool_results: Vec<(String, String)>);
}

/// A client for an LLM.
pub trait LlmClient {
    type Query: LlmQuery;

    /// Send a query to the LLM.
    async fn query(&self, query: Self::Query) -> anyhow::Result<(LlmResponse, Self::Query)>;
}
