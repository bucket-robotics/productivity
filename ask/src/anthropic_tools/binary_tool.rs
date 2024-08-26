//! Tools that allow direct access to binaries to scripts.

use anyhow::Context;

use super::RustTool;
use crate::anthropic_tools::ToolPrerequisites;

/// Input to the binary or script.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct BinaryToolInput {
    /// The arguments to pass to the binary or script.
    arguments: Vec<String>,
    /// The working directory to run the binary or script in.
    working_directory: Option<String>,
}

/// A binary tool that allows direct access to a binary.
pub struct BinaryTool {
    binary: String,
    description: String,
    send_output: bool,
    print_to_console: bool,
}

static USER_CAN_SEE_OUTPUT: &str = "The user will see the output of the command.";
static USER_CANT_SEE_OUTPUT: &str = "The user will not see the output of the command so if relevant you may want to give them snippets or a summary of the output if an error occurs.";

impl BinaryTool {
    /// Create a binary tool which sends stdout and stderr to the LLM.
    pub fn new_with_output(binary: &str, description: &str, print_to_console: bool) -> Self {
        let base_description = [
            description.to_string(),
            if print_to_console { USER_CAN_SEE_OUTPUT } else { USER_CANT_SEE_OUTPUT }.to_string(),
            format!("You will receive the return code, stdout, and stderr from `{binary}`."),
            format!("The stdout and stderr from {binary} will be returned sent back enclosed in XML tags - the `<stdout>` tag will have stdout and the `<stderr>` tag will have stderr."),
        ];

        Self {
            binary: binary.to_string(),
            description: base_description.join("\n"),
            send_output: true,
            print_to_console,
        }
    }

    /// Create a binary tool which only sends the return code to the LLM.
    pub fn new_without_output(binary: &str, description: &str, print_to_console: bool) -> Self {
        let base_description = [
            description.to_string(),
            if print_to_console {
                USER_CAN_SEE_OUTPUT
            } else {
                USER_CANT_SEE_OUTPUT
            }
            .to_string(),
            format!("You will receive the return code `{binary}`, but not the stdout or stderr."),
        ];
        Self {
            binary: binary.to_string(),
            description: base_description.join("\n"),
            send_output: false,
            print_to_console,
        }
    }
}

impl RustTool for BinaryTool {
    type Input = BinaryToolInput;

    fn get_name(&self) -> String {
        self.binary.clone()
    }

    fn get_description(&self) -> String {
        self.description.clone()
    }

    fn get_prequisites(&self) -> ToolPrerequisites {
        ToolPrerequisites {
            binaries: vec![self.binary.clone()],
        }
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        tracing::info!(
            "Running: {} with the arguments: {:?}",
            &self.binary,
            &input.arguments
        );

        let mut base_command = std::process::Command::new(&self.binary);
        if self.send_output {
            base_command
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
        } else if !self.print_to_console {
            base_command
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
        } else {
            base_command
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit());
        };

        let output = base_command
            .args(input.arguments)
            .current_dir(input.working_directory.unwrap_or_default())
            .output()
            .context("Failed to execute process")?;

        let mut result = vec![format!(
            "The return code was {code}",
            code = output.status.code().unwrap_or(-1)
        )];
        if self.send_output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            result.push(format!("<stdout>\n{}</stdout>", &stdout));
            result.push(format!("<stderr>\n{}</stderr>", &stderr,));
            if self.print_to_console {
                println!("{stdout}");
                eprintln!("{stderr}");
            }
        }

        Ok(result.join("\n"))
    }
}
