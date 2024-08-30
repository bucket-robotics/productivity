//! Ollama-related code.

mod data_types;

/// A basic client for the Anthropic API.
pub struct OllamaClient {
    /// The base URL for the API.
    pub base_url: String,
}

impl Default for OllamaClient {
    fn default() -> Self {
        OllamaClient {
            base_url: "http://localhost:11434".to_string(),
        }
    }
}

impl OllamaClient {
    /// Get the currently downloaded models.
    pub async fn get_tags(&self) -> anyhow::Result<data_types::TagsResponse> {
        let url = format!("{}/api/tags", self.base_url);
        let response = reqwest::get(&url)
            .await?
            .error_for_status()?
            .json::<data_types::TagsResponse>()
            .await?;
        Ok(response)
    }

    /// Pull a model.
    pub async fn pull(&self, name: &str) -> anyhow::Result<data_types::PullResponse> {
        let url = format!("{}/api/pull", self.base_url);
        let request = data_types::PullRequest {
            model: name.to_string(),
            insecure: false,
            stream: false,
        };
        let response = reqwest::Client::new()
            .post(&url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<data_types::PullResponse>()
            .await?;

        if &response.status != "success" {
            anyhow::bail!("Failed to pull model: {:?}", response);
        }
        Ok(response)
    }

    /// Pull a model if it's not already downloaded.
    pub async fn pull_if_needed(&self, name: &str) -> anyhow::Result<()> {
        let tags = self.get_tags().await?;
        if !tags.models.iter().any(|model| model.name == name) {
            println!("Downloading model {name}...");
            self.pull(name).await?;
            println!("Model {name} downloaded.");
        }
        Ok(())
    }
}
