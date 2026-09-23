use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};

use crate::{ApiError, Balance, CreatePayment, Error, List, Payment, Transaction};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_BASE_URL: &str = "https://wallet-api.serika.dev";

/// SerikaPay API client. Cheap to clone; share one per process.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    max_retries: u32,
}

/// Configures a [`Client`].
#[derive(Debug)]
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
    max_retries: u32,
}

impl ClientBuilder {
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Per-attempt timeout. Default 30 seconds.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Retries for network errors, 429 and 5xx. Default 2.
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    pub fn build(self) -> Client {
        let http = reqwest::Client::builder()
            .timeout(self.timeout)
            .user_agent(format!("serikapay-rust/{VERSION}"))
            .build()
            .expect("failed to build HTTP client");
        Client {
            http,
            api_key: self.api_key,
            base_url: self.base_url,
            max_retries: self.max_retries,
        }
    }
}

impl Client {
    /// A client with default settings, authenticated with a secret key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder(api_key).build()
    }

    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 2,
        }
    }

    pub fn payments(&self) -> Payments<'_> {
        Payments { client: self }
    }

    pub fn balance(&self) -> Balances<'_> {
        Balances { client: self }
    }

    pub fn transactions(&self) -> Transactions<'_> {
        Transactions { client: self }
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, Option<String>)],
        body: Option<Value>,
        idempotency_key: Option<&str>,
    ) -> Result<T, Error> {
        let url = format!("{}{}", self.base_url, path);
        let query: Vec<(&str, String)> = query
            .iter()
            .filter_map(|(k, v)| v.clone().map(|v| (*k, v)))
            .collect();
        let retryable = method == Method::GET || idempotency_key.is_some();

        let mut attempt = 0;
        loop {
            let mut req = self
                .http
                .request(method.clone(), &url)
                .bearer_auth(&self.api_key)
                .header("Accept", "application/json");
            if !query.is_empty() {
                req = req.query(&query);
            }
            if let Some(b) = &body {
                req = req.json(b);
            }
            if let Some(k) = idempotency_key {
                req = req.header("X-Idempotency-Key", k);
            }

            let res = match req.send().await {
                Ok(res) => res,
                Err(e) => {
                    if retryable && attempt < self.max_retries {
                        backoff(attempt).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(Error::Connection(e));
                }
            };

            let status = res.status();
            if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                && retryable
                && attempt < self.max_retries
            {
                backoff(attempt).await;
                attempt += 1;
                continue;
            }
            let bytes = res.bytes().await.map_err(Error::Connection)?;
            if !status.is_success() {
                let raw: Map<String, Value> = serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_object()).cloned())
                    .unwrap_or_default();
                return Err(Error::Api(ApiError {
                    error_type: raw
                        .get("type")
                        .and_then(Value::as_str)
                        .unwrap_or("api_error")
                        .to_string(),
                    message: raw
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("HTTP {}", status.as_u16())),
                    status: status.as_u16(),
                    raw,
                }));
            }
            return Ok(serde_json::from_slice(&bytes)?);
        }
    }
}

async fn backoff(attempt: u32) {
    let base = (500u64 << attempt.min(4)).min(5_000);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let ms = base / 2 + nanos % (base / 2 + 1);
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

fn seg(id: &str) -> String {
    id.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Create, read and refund payments.
pub struct Payments<'a> {
    client: &'a Client,
}

impl Payments<'_> {
    /// Charge your wallet. Returns the completed payment.
    pub async fn create(&self, params: CreatePayment) -> Result<Payment, Error> {
        let key = params
            .idempotency_key
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let body = serde_json::to_value(&params)?;
        self.client
            .request(Method::POST, "/v1/payments", &[], Some(body), Some(&key))
            .await
    }

    pub async fn retrieve(&self, id: &str) -> Result<Payment, Error> {
        self.client
            .request(
                Method::GET,
                &format!("/v1/payments/{}", seg(id)),
                &[],
                None,
                None,
            )
            .await
    }

    /// Up to `limit` (1–100, default 10) recent payments.
    pub async fn list(&self, limit: Option<u32>) -> Result<List<Payment>, Error> {
        self.client
            .request(
                Method::GET,
                "/v1/payments",
                &[("limit", limit.map(|l| l.to_string()))],
                None,
                None,
            )
            .await
    }

    /// Refund all of a completed payment (`None`), or `amount` of it.
    pub async fn refund(&self, id: &str, amount: Option<f64>) -> Result<Payment, Error> {
        let body = match amount {
            Some(a) => json!({ "amount": a }),
            None => json!({}),
        };
        self.client
            .request(
                Method::POST,
                &format!("/v1/payments/{}/refund", seg(id)),
                &[],
                Some(body),
                None,
            )
            .await
    }
}

/// Read your wallet balance.
pub struct Balances<'a> {
    client: &'a Client,
}

impl Balances<'_> {
    /// The EUR balance.
    pub async fn retrieve(&self) -> Result<Balance, Error> {
        self.retrieve_currency("EUR").await
    }

    /// The balance of one ledger currency: EUR, USD or JPY.
    pub async fn retrieve_currency(&self, currency: &str) -> Result<Balance, Error> {
        self.client
            .request(
                Method::GET,
                "/v1/balance",
                &[("currency", Some(currency.to_string()))],
                None,
                None,
            )
            .await
    }
}

/// List ledger entries.
pub struct Transactions<'a> {
    client: &'a Client,
}

impl Transactions<'_> {
    /// `kind` is purchase, spend, refund or adjustment.
    pub async fn list(
        &self,
        limit: Option<u32>,
        kind: Option<&str>,
    ) -> Result<List<Transaction>, Error> {
        self.client
            .request(
                Method::GET,
                "/v1/transactions",
                &[
                    ("limit", limit.map(|l| l.to_string())),
                    ("type", kind.map(str::to_string)),
                ],
                None,
                None,
            )
            .await
    }
}
