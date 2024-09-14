use core::str;

use anyhow::Context;

use super::RustTool;
use crate::tools::ToolPrerequisites;

/// The kubernetes context.
#[derive(serde::Deserialize, schemars::JsonSchema)]
struct KubernetesContext {
    /// The name of the kubernetes context to use.
    kubernetes_context_name: String,
}

/// Input to the cloud context tool.
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ArgocdStatusInput {
    #[serde(flatten)]
    kubernetes_context: KubernetesContext,
}

/// A tool that collects status information from `ArgoCD`
pub struct ArgocdStatusTool;

impl RustTool for ArgocdStatusTool {
    type Input = ArgocdStatusInput;

    fn get_name(&self) -> String {
        "argocd_status".to_string()
    }

    fn get_description(&self) -> String {
        "Get the status of ArgoCD repositories in a given Kubernetes cluster. This returned value will be a JSON list containing the status of each GitOps repository ArgoCD manages in the cluster.".to_string()
    }

    fn get_prequisites(&self) -> ToolPrerequisites {
        ToolPrerequisites {
            binaries: vec![String::from("argocd")],
        }
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        let kubectl_output = std::process::Command::new("argocd")
            .arg("repo")
            .arg("list")
            .arg("--output=json")
            .arg(format!(
                "--kube-context={}",
                input.kubernetes_context.kubernetes_context_name
            ))
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .context("Failed to execute argocd repo list")?;

        Ok(if kubectl_output.status.success() {
            str::from_utf8(&kubectl_output.stdout)?.to_string()
        } else {
            str::from_utf8(&kubectl_output.stderr)?.to_string()
        })
    }
}
