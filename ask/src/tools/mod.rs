//! Framework for Anthropic tools.

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use anyhow::Context;

pub mod rust_tools;

mod binary_tool;
mod filesystem;
mod http_request;
mod open;
mod package_manager;
mod software_versions;
mod terraform;

/// A tool definition.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ToolDefinition {
    /// The name of the tool. Must match the regex `^[a-zA-Z0-9_-]{1,64}$`.
    pub name: String,
    /// A detailed plaintext description of what the tool does, when it should be used, and how it behaves.
    pub description: String,
    /// A JSON Schema object defining the expected parameters for the tool.
    ///
    /// See <https://json-schema.org/> for more information.
    pub input_schema: serde_json::Value,
}

/// The prerequisites required to run a tool.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct ToolPrerequisites {
    /// The binaries that must be available on the system for the tool to run.
    pub binaries: Vec<String>,
}

/// A tool that the LLMs can run.
///
/// For tools implemented inside this binary in Rust use the `RustTool` trait.
pub trait Tool {
    /// Get the definition of the tool.
    fn get_definition(&self) -> ToolDefinition;

    /// Get the prerequisites required to run the tool.
    fn get_prequisites(&self) -> ToolPrerequisites {
        ToolPrerequisites { binaries: vec![] }
    }

    /// Run the tool.
    fn run(
        self: Arc<Self>,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>>>>;
}

/// A tool implemented in Rust.
pub trait RustTool {
    /// The input to the tool.
    type Input;

    /// The name of the tool. Must match the regex `^[a-zA-Z0-9_-]{1,64}$`.
    fn get_name(&self) -> String;

    /// A detailed plaintext description of what the tool does, when it should be used, and how it behaves.
    fn get_description(&self) -> String;

    /// Get the prerequisites required to run the tool.
    fn get_prequisites(&self) -> ToolPrerequisites {
        ToolPrerequisites { binaries: vec![] }
    }

    /// Run the tool.
    async fn run(self: Arc<Self>, input: Self::Input) -> anyhow::Result<String>;
}

// Blanket implementation for Rust tools.
impl<T: RustTool + 'static> Tool for T
where
    <Self as RustTool>::Input: for<'a> serde::Deserialize<'a> + schemars::JsonSchema,
{
    fn get_definition(&self) -> ToolDefinition {
        let schema = schemars::schema_for!(<Self as RustTool>::Input);
        ToolDefinition {
            name: self.get_name(),
            description: self.get_description(),
            input_schema: serde_json::to_value(&schema).unwrap(),
        }
    }

    fn get_prequisites(&self) -> ToolPrerequisites {
        RustTool::get_prequisites(self)
    }

    fn run(
        self: Arc<Self>,
        input: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>>>> {
        Box::pin(async {
            let tool_name = self.get_name();
            let input: <Self as RustTool>::Input =
                serde_json::from_value(input).with_context(|| {
                    format!("Converting the input JSON to {tool_name}'s input struct")
                })?;

            self.run(input)
                .await
                .with_context(|| format!("Running the {tool_name} tool"))
        })
    }
}

/// Get an appropriate cache directory for a tool.
fn get_cache_dir_for_tool(tool: &dyn Tool) -> anyhow::Result<PathBuf> {
    let config = productivity_config::Config::get_or_default()?;
    let tool_name = tool.get_definition().name;
    let cache_dir = config.cache_location.join(&tool_name);
    std::fs::create_dir_all(&cache_dir).with_context(|| {
        format!(
            "Creating cache directory {} for {}",
            cache_dir.display(),
            &tool_name
        )
    })?;
    Ok(cache_dir)
}
