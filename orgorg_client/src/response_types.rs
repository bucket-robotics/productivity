/// A `go/` link.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GoLink {
    /// The name of the link.
    pub name: String,
    /// The description of the link.
    pub description: String,
    /// The target URL of the link.
    pub url: String,
}

/// The response to a `v1/go/find` query.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GoFindResponse {
    /// The `go/` links that match the query.
    pub links: Vec<GoLink>,
}
