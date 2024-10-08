use std::path::PathBuf;

use anyhow::Context;

/// The available LLM providers.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum LlmProvider {
    /// The Anthropic API.
    Anthropic {
        /// The API key to use.
        api_key: String,
    },
    /// The Ollama API.
    Ollama {
        /// The base URL for the API.
        url_base: Option<String>,
        /// The model to use.
        model: Option<String>,
    },
}

impl Default for LlmProvider {
    fn default() -> Self {
        Self::Anthropic {
            api_key: String::new(),
        }
    }
}

impl LlmProvider {
    /// Get the base URL for the provider.
    pub fn get_url_base(&self) -> &str {
        match self {
            Self::Anthropic { .. } => "https://api.anthropic.com",
            Self::Ollama { url_base, .. } => url_base
                .as_ref()
                .map_or("http://localhost:11434", String::as_str),
        }
    }
}

/// Configuration for productivity CLI tools.
#[derive(serde::Deserialize, serde::Serialize, Debug, Default)]
pub struct Config {
    #[serde(skip)]
    /// The path the config file was loaded from.
    pub config_file_path: Option<String>,
    #[serde(skip)]
    /// A directory to use for caching data.
    pub cache_location: PathBuf,
    /// The base URL for the `OrgOrg` API.
    pub orgorg_url_base: Option<String>,
    /// The API key to use for `OrgOrg`.
    pub orgorg_api_key: Option<String>,
    /// The LLM provider to use.
    pub llm_provider: LlmProvider,
    /// Extra system prompt content for the `ask` tool.
    pub ask_system_prompt: Option<String>,
}

impl Config {
    /// Get the config using the XDG directories structure.
    pub fn get_or_default() -> anyhow::Result<Self> {
        let Some(project_dirs) = directories::ProjectDirs::from("bot", "bucket", "productivity")
        else {
            anyhow::bail!("Could not find project directories");
        };
        let config_file = project_dirs.config_dir().join("config.json");
        let mut config = if config_file.exists() {
            Self::load(config_file).context("Loading config")?
        } else {
            // If no config file exists then create the file using the default config
            let config = Self::default();
            std::fs::create_dir_all(project_dirs.config_dir())
                .context("Creating config directory")?;
            config.save(config_file).context("Writing default config")?;
            config
        };
        config.cache_location = project_dirs.cache_dir().to_path_buf();
        std::fs::create_dir_all(&config.cache_location).context("Creating cache directory")?;
        Ok(config)
    }

    /// Load the configuration from the given path.
    pub fn load<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let file = std::fs::File::open(&path).context("Opening config file for reading")?;
        let reader = std::io::BufReader::new(file);
        let mut config: Config = serde_json::from_reader(reader).context("Reading config file")?;
        config.config_file_path = Some(path.as_ref().to_string_lossy().to_string());
        Ok(config)
    }

    /// Save the configuration to the given path.
    pub fn save<P: AsRef<std::path::Path>>(&self, path: P) -> anyhow::Result<()> {
        let file = std::fs::File::create(path).context("Opening config file for writing")?;
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self).context("Reading config file")?;
        Ok(())
    }

    /// Get the API key to use.
    #[must_use]
    pub fn get_orgorg_api_key(&self) -> Option<String> {
        self.orgorg_api_key
            .clone()
            .or_else(|| std::env::var("ORGORG_API_KEY").ok())
    }

    /// Get the base URL for the `OrgOrg` API.
    #[must_use]
    pub fn get_orgorg_url_base(&self) -> String {
        self.orgorg_url_base
            .clone()
            .unwrap_or_else(|| orgorg_client::DEFAULT_URL_BASE.to_string())
    }
}
