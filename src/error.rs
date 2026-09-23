use serde_json::{Map, Value};

/// Everything that can go wrong in this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The API answered with a non-2xx status.
    #[error("serikapay: {0}")]
    Api(ApiError),
    /// The request never got an answer (DNS, TLS, timeout, connection reset).
    #[error("serikapay: could not reach SerikaPay: {0}")]
    Connection(#[source] reqwest::Error),
    /// The response body wasn't what we expected.
    #[error("serikapay: could not decode response: {0}")]
    Decode(#[from] serde_json::Error),
    /// A webhook signature was missing or wrong.
    #[error("serikapay: invalid webhook signature")]
    InvalidSignature,
}

/// An error returned by the API.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// e.g. `insufficient_balance`
    pub error_type: String,
    pub message: String,
    pub status: u16,
    /// The raw `error` object.
    pub raw: Map<String, Value>,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}): {}", self.error_type, self.status, self.message)
    }
}

impl std::error::Error for ApiError {}
