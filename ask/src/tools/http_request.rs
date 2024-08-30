//! Tool to make HTTP requests.

use anyhow::Context;

use super::RustTool;

/// Tool to send HTTP GET requests.
pub struct HttpGetTool;

/// Input to the HTTP request tool.
#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
pub struct HttpGetRequestToolInput {
    /// The URL to send the GET request to.
    url: String,
}

impl RustTool for HttpGetTool {
    type Input = HttpGetRequestToolInput;

    fn get_name(&self) -> String {
        "http_get_request".to_string()
    }

    fn get_description(&self) -> String {
        "Send an HTTP GET request to the provided URL. This tool will return the response code and the body.".to_string()
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        tracing::info!("Sending request to {}", &input.url);

        let response = reqwest::get(&input.url)
            .await
            .context("Failed to send the HTTP request")?;

        Ok(format!(
            "Response code was {} - the response body is below:\n {}",
            response.status().as_str(),
            response
                .text()
                .await
                .context("Failed to read the response body")?
        ))
    }
}
