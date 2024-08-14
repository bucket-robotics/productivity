mod response_types;

pub use response_types::{GoFindResponse, GoLink};

/// Errors that can occur using the API client.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// An error occurred while making a request.
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// An error occurred while making a request.
    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    /// The client is unauthorized.
    #[error("Unauthorized - is your OrgOrg API key correct?")]
    Unauthorized,
}

/// See <https://orgorg.us/api/> for the API documentation.
pub static DEFAULT_URL_BASE: &str = "https://orgorg.us/api/v1";

/// A client for the `OrgOrg` API.
pub struct Client {
    /// The base URL for the API.
    url_base: String,
    /// The API key to use.
    api_key: String,
    /// The HTTP client to use.
    client: reqwest::Client,
}

impl Client {
    /// Create a new client.
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self::new_with_url(api_key, DEFAULT_URL_BASE.to_string())
    }

    /// Create a new client with the given base URL.
    #[must_use]
    pub fn new_with_url(api_key: String, url_base: String) -> Self {
        Self {
            url_base,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Query `go/` links.
    ///
    /// # Arguments
    ///
    /// - `link`: The link to query without the `go/` prefix.
    pub async fn go_find(&self, link: &str) -> Result<GoFindResponse, Error> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/go/find", &self.url_base),
            &[("q", link)],
        )?;
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await?;

        if response.status() == 401 {
            return Err(Error::Unauthorized);
        }
        Ok(response.json::<GoFindResponse>().await?)
    }
}
