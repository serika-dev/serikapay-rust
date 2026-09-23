use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A charge against your wallet.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Payment {
    pub id: String,
    pub object: String,
    pub amount: f64,
    pub currency: String,
    /// pending, completed, failed, refunded or expired
    pub status: String,
    pub description: Option<String>,
    pub customer_email: Option<String>,
    pub customer_id: Option<String>,
    #[serde(default)]
    pub metadata: Map<String, Value>,
    /// ISO 8601
    pub created_at: String,
    pub completed_at: Option<String>,
    pub expires_at: Option<String>,
}

/// Balance of one ledger currency.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Balance {
    pub object: String,
    pub available: f64,
    pub reserved: f64,
    pub total: f64,
    pub currency: String,
}

/// One ledger entry.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub id: String,
    /// purchase, spend, refund or adjustment
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub amount: f64,
    pub balance_after: f64,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

/// A page of results, newest first.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct List<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub has_more: bool,
}

/// Fields for [`Payments::create`](crate::Payments::create).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePayment {
    /// Amount in coins, at least 0.01.
    pub amount: f64,
    /// EUR (default), USD or JPY ledger.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    /// Makes the request safe to retry. Generated when `None`.
    #[serde(skip)]
    pub idempotency_key: Option<String>,
}

/// A webhook delivery.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub object: String,
    /// e.g. `payment.completed`
    #[serde(rename = "type")]
    pub event_type: String,
    pub created_at: String,
    pub data: Value,
}

impl Event {
    /// Decode `data` as a payment, for `payment.*` events.
    pub fn payment(&self) -> Result<Payment, serde_json::Error> {
        serde_json::from_value(self.data.clone())
    }
}
