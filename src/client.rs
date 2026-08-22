use std::time::Duration;

use reqwest::header::{ACCEPT, HeaderValue};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use url::Url;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct ApiEndpoint(Url);

impl ApiEndpoint {
    /// Parses an API base URL and enforces its transport policy.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed URLs, embedded credentials, fragments,
    /// unsupported schemes, or non-loopback cleartext endpoints.
    pub fn parse(value: &str) -> Result<Self, ClientError> {
        let mut url = Url::parse(value).map_err(|_| ClientError::InvalidBaseUrl)?;
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ClientError::EmbeddedCredentials);
        }
        if url.fragment().is_some() {
            return Err(ClientError::UrlFragment);
        }

        let loopback = matches!(url.host_str(), Some("127.0.0.1" | "::1" | "localhost"));
        let allowed = url.scheme() == "https" || (loopback && url.scheme() == "http");
        if !allowed {
            return Err(ClientError::InsecureBaseUrl);
        }

        url.set_query(None);
        if !url.path().ends_with('/') {
            let path = format!("{}/", url.path());
            url.set_path(&path);
        }

        Ok(Self(url))
    }

    fn resolve(&self, path: &str) -> Result<Url, ClientError> {
        if !path.starts_with('/') || path.starts_with("//") {
            return Err(ClientError::InvalidRequestPath);
        }

        let mut url = self.0.clone();
        url.set_path(path);
        url.set_query(None);
        url.set_fragment(None);
        Ok(url)
    }

    #[must_use]
    pub fn display_origin(&self) -> String {
        self.0.origin().ascii_serialization()
    }
}

pub struct ApiClient {
    endpoint: ApiEndpoint,
    client: Client,
}

impl ApiClient {
    /// Builds an HTTP client that rejects redirects and applies bounded timeouts.
    ///
    /// # Errors
    ///
    /// Returns an error if the TLS or HTTP client cannot be constructed.
    pub fn new(endpoint: ApiEndpoint) -> Result<Self, ClientError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("act-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ClientError::Build)?;

        Ok(Self { endpoint, client })
    }

    /// Executes a bounded JSON GET against the configured API origin.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid paths, network failures, oversized
    /// responses, or malformed JSON response bodies.
    pub async fn get(
        &self,
        path: &str,
        access_token: Option<&str>,
    ) -> Result<ApiResponse, ClientError> {
        let url = self.endpoint.resolve(path)?;
        let mut request = self
            .client
            .get(url)
            .header(ACCEPT, HeaderValue::from_static("application/json"));

        if let Some(access_token) = access_token {
            let access_token = access_token.trim();
            if access_token.is_empty() {
                return Err(ClientError::EmptyAccessToken);
            }
            request = request.bearer_auth(access_token);
        }

        let mut response = request.send().await.map_err(ClientError::Request)?;
        let status = response.status();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(ClientError::Request)? {
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(ClientError::ResponseTooLarge);
            }
            body.extend_from_slice(&chunk);
        }

        let body = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).map_err(ClientError::InvalidJson)?
        };

        Ok(ApiResponse {
            ok: status.is_success(),
            status: status.as_u16(),
            body,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ApiResponse {
    pub ok: bool,
    pub status: u16,
    pub body: Value,
}

impl ApiResponse {
    #[must_use]
    pub fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("API base URL is invalid")]
    InvalidBaseUrl,
    #[error("API base URL must use HTTPS outside loopback development")]
    InsecureBaseUrl,
    #[error("credentials must not be embedded in the API URL")]
    EmbeddedCredentials,
    #[error("URL fragments are not supported")]
    UrlFragment,
    #[error("request path must begin with one slash and remain on the configured origin")]
    InvalidRequestPath,
    #[error("ACT_ACCESS_TOKEN is empty")]
    EmptyAccessToken,
    #[error("could not construct the HTTP client: {0}")]
    Build(reqwest::Error),
    #[error("API request failed: {0}")]
    Request(reqwest::Error),
    #[error("API response exceeded the 1 MiB safety limit")]
    ResponseTooLarge,
    #[error("API returned malformed JSON: {0}")]
    InvalidJson(serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_https_and_loopback_http() {
        assert!(ApiEndpoint::parse("https://api.anticaptrad.example").is_ok());
        assert!(ApiEndpoint::parse("http://127.0.0.1:8080").is_ok());
    }

    #[test]
    fn rejects_remote_http_and_embedded_credentials() {
        assert!(matches!(
            ApiEndpoint::parse("http://api.anticaptrad.example"),
            Err(ClientError::InsecureBaseUrl)
        ));
        assert!(matches!(
            ApiEndpoint::parse("https://user:secret@api.example"),
            Err(ClientError::EmbeddedCredentials)
        ));
    }

    #[test]
    fn request_paths_cannot_switch_origins() {
        let endpoint = ApiEndpoint::parse("https://api.example/v1").expect("valid endpoint");
        assert!(endpoint.resolve("/health").is_ok());
        assert!(matches!(
            endpoint.resolve("//attacker.example/path"),
            Err(ClientError::InvalidRequestPath)
        ));
        assert!(matches!(
            endpoint.resolve("https://attacker.example/path"),
            Err(ClientError::InvalidRequestPath)
        ));
    }
}
