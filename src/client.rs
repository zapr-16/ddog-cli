use crate::config::Config;
use crate::error::DdError;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::time::Duration;

pub struct DdClient {
    http: reqwest::Client,
    base_url: String,
}

impl DdClient {
    pub fn new(config: &Config) -> Result<Self, DdError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "DD-API-KEY",
            HeaderValue::from_str(&config.api_key).map_err(|e| DdError::Api {
                status: 0,
                body: format!("Invalid API key header value: {e}"),
            })?,
        );
        headers.insert(
            "DD-APPLICATION-KEY",
            HeaderValue::from_str(&config.app_key).map_err(|e| DdError::Api {
                status: 0,
                body: format!("Invalid APP key header value: {e}"),
            })?,
        );
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(DdClient {
            http,
            base_url: config.base_url(),
        })
    }

    /// Create a client pointing at a custom base URL (for tests).
    pub fn with_base_url(base_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build test http client");
        DdClient {
            http,
            base_url: base_url.to_string(),
        }
    }

    pub async fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, DdError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.get(&url).query(query).send().await?;
        self.handle_response(resp).await
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, DdError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.http.post(&url).json(body).send().await?;
        self.handle_response(resp).await
    }

    async fn handle_response(&self, resp: reqwest::Response) -> Result<Value, DdError> {
        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(DdError::Api { status, body });
        }
        let body = resp.text().await?;
        if body.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&body).map_err(DdError::Json)
    }
}
