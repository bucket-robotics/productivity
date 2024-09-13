use anyhow::Context;

use super::RustTool;
use crate::tools::ToolPrerequisites;

/// Input to the Terraform plan command.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct TerraformPlanInput {
    /// The working directory to run the Terraform plan in.
    working_directory: Option<String>,
}

/// A tool that wraps the `terraform plan` command.
pub struct TerraformPlanTool;

impl RustTool for TerraformPlanTool {
    type Input = TerraformPlanInput;

    fn get_name(&self) -> String {
        "terraform_plan".to_string()
    }

    fn get_description(&self) -> String {
        "Run `terraform plan` in the specified directory and returns the output plan. This tool can be used to find out if cloud deployments are up to date.".to_string()
    }

    fn get_prequisites(&self) -> ToolPrerequisites {
        ToolPrerequisites {
            binaries: vec!["terraform".to_string()],
        }
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        let mut base_command = std::process::Command::new("terraform");
        let working_directory = crate::path_utils::expand_path(
            input.working_directory.clone().unwrap_or_default().as_str(),
        )?;
        let output = base_command
            .arg(format!("-chdir={working_directory}"))
            .arg("plan")
            .arg("-detailed-exitcode")
            .arg("-no-color")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .context("Failed to execute terraform plan")?;

        // 0 - Succeeded, diff is empty (no changes)
        // 1 - Errored
        // 2 - Succeeded, there is a diff
        let status = output.status.code().unwrap_or(1);
        Ok(match status {
            0 => "The infrastructure is up to date, terraform has no changes to deploy".to_string(),
            2 => format!("The infrastructure has changes, terraform has a plan to deploy:\n<stdout>\n{}\n</stdout>", String::from_utf8_lossy(&output.stdout)),
            _ => format!("An error occurred while running terraform plan:\n<stderr>\n{}\n</stderr>", String::from_utf8_lossy(&output.stderr)),
        })
    }
}
