//! Tool to make HTTP requests.

use anyhow::Context;

use super::RustTool;

/// Crates.io rate limit.
const CRATES_IO_RATE_LIMIT: std::time::Duration = std::time::Duration::from_secs(1);

/// Tool to get the latest version of software libraries.
pub struct SoftwareVersionsTool {
    last_cargo_request_time: tokio::sync::Mutex<Option<std::time::Instant>>,
}

#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
#[serde(tag = "type")]
pub enum Library {
    /// A cargo crate.
    #[serde(rename = "cargo")]
    Cargo { crate_name: String },
}

/// Input to the software versions tool.
#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
pub struct SoftwareVersionsToolInput {
    /// The libraries to get version information about.
    libraries: Vec<Library>,
}

async fn get_crate_version(crate_name: &str) -> anyhow::Result<String> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}");
    tracing::info!("Sending request to {url}");

    let client = reqwest::Client::new();
    let maybe_response = client
        .get(url)
        .header("Accept", "application/json")
        .header(
            "User-Agent",
            "bucket_ask (https://github.com/bucket-robotics/productivity)",
        )
        .send()
        .await
        .context("Failed to send the HTTP request")?
        .error_for_status();
    let response: serde_json::Value = match maybe_response {
        Ok(response) => response
            .json()
            .await
            .context("Failed to read the response body")?,
        Err(e) => {
            tracing::error!("Failed to get crate version: {e}");
            return Err(e.into());
        }
    };

    tracing::debug!("Response: {response}");
    Ok(response["crate"]["max_stable_version"]
        .as_str()
        .context("Did not find crate.max_version in the response")?
        .to_string())
}

impl SoftwareVersionsTool {
    /// Create a new software versions tool.
    pub fn new() -> Self {
        SoftwareVersionsTool {
            last_cargo_request_time: tokio::sync::Mutex::new(None),
        }
    }
}

impl RustTool for SoftwareVersionsTool {
    type Input = SoftwareVersionsToolInput;

    fn get_name(&self) -> String {
        "software_version".to_string()
    }

    fn get_description(&self) -> String {
        "Get the latest version of software libraries. For example if you query the version of library `foo` this tool and the latest version is `1.2.3` this tool will return `foo: 1.2.3`.".to_string()
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        let mut result = Vec::with_capacity(input.libraries.len());
        let progress = indicatif::ProgressBar::new(input.libraries.len() as u64).with_style(
            indicatif::ProgressStyle::with_template("{msg} {wide_bar} {pos}/{len}").unwrap(),
        );
        for library in input.libraries {
            let (library_name, maybe_version) = match library {
                Library::Cargo { crate_name } => {
                    progress.set_message(format!("Fetching version for {crate_name}"));
                    // Comply with crates.io's one per second rate limit.
                    if let Some(last_request_time) = *self.last_cargo_request_time.lock().await {
                        let elapsed = last_request_time.elapsed();
                        if elapsed < CRATES_IO_RATE_LIMIT {
                            tokio::time::sleep(CRATES_IO_RATE_LIMIT - elapsed).await;
                        }
                    }
                    let version = get_crate_version(&crate_name).await;
                    (crate_name, version)
                }
            };

            progress.inc(1);

            match maybe_version {
                Ok(version) => result.push(format!("{library_name}: {version}")),
                Err(e) => result.push(format!("Failed to get version for {library_name}: {e}")),
            }
        }

        progress.finish_and_clear();
        Ok(result.join("\n"))
    }
}
