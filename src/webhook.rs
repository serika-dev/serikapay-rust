//! Verify and parse webhook deliveries.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::{Error, Event};

type HmacSha256 = Hmac<Sha256>;

/// The `Serika-Signature` value for `payload`. Useful in tests.
pub fn generate_signature(payload: impl AsRef<[u8]>, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC takes keys of any size");
    mac.update(payload.as_ref());
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// True when `signature` matches the raw payload (constant-time).
pub fn verify_signature(payload: impl AsRef<[u8]>, signature: &str, secret: &str) -> bool {
    let Some(hex_sig) = signature.trim().strip_prefix("sha256=") else {
        return false;
    };
    let Ok(given) = hex::decode(hex_sig) else {
        return false;
    };
    if secret.is_empty() {
        return false;
    }
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC takes keys of any size");
    mac.update(payload.as_ref());
    mac.verify_slice(&given).is_ok()
}

/// Verify the `Serika-Signature` header and parse the event. Pass the raw request body.
pub fn construct_event(
    payload: impl AsRef<[u8]>,
    signature: &str,
    secret: &str,
) -> Result<Event, Error> {
    if !verify_signature(payload.as_ref(), signature, secret) {
        return Err(Error::InvalidSignature);
    }
    Ok(serde_json::from_slice(payload.as_ref())?)
}
