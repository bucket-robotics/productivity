use core::str;

use anyhow::Context;

use super::RustTool;
use crate::tools::ToolPrerequisites;

/// Input to the cloud context tool.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct CloudContextInput {}

/// A tool that provides general information about available cloud resources.
pub struct CloudContextTool;

impl RustTool for CloudContextTool {
    type Input = CloudContextInput;

    fn get_name(&self) -> String {
        "cloud_context".to_string()
    }

    fn get_description(&self) -> String {
        "Get information about available Kubernetes clusters and AWS profiles.".to_string()
    }

    fn get_prequisites(&self) -> ToolPrerequisites {
        ToolPrerequisites { binaries: vec![] }
    }

    async fn run(self: std::sync::Arc<Self>, _input: Self::Input) -> anyhow::Result<String> {
        let kubectl_output = std::process::Command::new("kubectl")
            .arg("config")
            .arg("view")
            .arg("--output=jsonpath={.contexts}")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .context("Failed to execute kubectl config view")?;
        let aws_output = std::process::Command::new("aws")
            .arg("configure")
            .arg("list-profiles")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .context("Failed to execute aws configure list-profiles")?;

        let mut output = serde_json::json!({});
        output["kubernetes"] = if kubectl_output.status.success() {
            serde_json::from_slice(&kubectl_output.stdout)?
        } else {
            serde_json::Value::Null
        };
        output["aws_profiles"] = if aws_output.status.success() {
            str::from_utf8(&aws_output.stdout)?
                .lines()
                .map(|line| serde_json::Value::String(line.to_string()))
                .collect::<Vec<_>>()
                .into()
        } else {
            serde_json::Value::Null
        };

        Ok(serde_json::to_string(&output)?)
    }
}
