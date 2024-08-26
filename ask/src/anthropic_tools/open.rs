//! Open tool.

use super::RustTool;

/// Tool to open a file or URL in a graphical tool - for example a web browser or text editor - on the user's machine.
pub struct OpenTool;

/// Input to the open tool.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct OpenToolInput {
    /// Either the path to a file to open or a URL to open.
    path: String,
    /// A hint to tell the tool whether or not `path` is a source code file that the user might want to interactively edit.
    /// The will change the behavior of the tool, for example by opening the file in an editor instead of a viewer.
    is_source_code: bool,
}

impl RustTool for OpenTool {
    type Input = OpenToolInput;

    fn get_name(&self) -> String {
        "open".to_string()
    }

    fn get_description(&self) -> String {
        "Open a file or URL on the user's computer.".to_string()
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        let open_result = if input.is_source_code {
            if let Ok(editor) = std::env::var("EDITOR") {
                let arguments = shlex::split(&editor).unwrap_or_else(|| vec![editor]);
                let mut command = open::with_command(&input.path, &arguments[0]);
                for argument in arguments.iter().skip(1) {
                    command.arg(argument);
                }

                std::thread::spawn(move || {
                    let _ = command.spawn();
                });
                Ok(())
            } else {
                open::that(&input.path)
            }
        } else {
            open::that(&input.path)
        };

        Ok(if let Err(e) = open_result {
            format!("Could not open the path - the error was {e}")
        } else {
            "Sucessfully opened the path".to_string()
        })
    }
}
