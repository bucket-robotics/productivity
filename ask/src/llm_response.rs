/// A text output section.
pub struct TextOutput {
    pub bold: bool,
    pub italic: bool,
    pub color: Option<String>,
    pub text: String,
}

impl TextOutput {
    pub fn get_terminal_style(&self) -> console::Style {
        let mut style = console::Style::new();
        style = if self.bold { style.bold() } else { style };
        style = if self.italic { style.italic() } else { style };
        style = match self.color.as_deref() {
            Some("red") => style.red(),
            Some("yellow") => style.yellow(),
            Some("green") => style.green(),
            _ => style,
        };
        style
    }
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
    /// The raw responses from the LLM.
    pub raw_text: Vec<String>,
    /// A list of tool invocations.
    pub tool_invocations: Vec<ToolInvocation>,
}
