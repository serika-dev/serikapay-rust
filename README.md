# SerikaPay Rust library

[![CI](https://github.com/serika-dev/serikapay-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/serika-dev/serikapay-rust/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/serikapay)](https://crates.io/crates/serikapay)
[![docs.rs](https://img.shields.io/docsrs/serikapay)](https://docs.rs/serikapay)

The official [SerikaPay](https://wallet.serika.dev/developers) library for Rust. Async, built on `reqwest` with rustls, runs on Tokio.

## Install

```bash
cargo add serikapay
```

## Usage

Create a key under **Developers → API keys** in your wallet. Keep it on the server.

```rust
use serikapay::{Client, CreatePayment};

let serika = Client::new(std::env::var("SERIKA_SECRET_KEY")?);

let payment = serika
    .payments()
    .create(CreatePayment {
        amount: 4.99,
        description: Some("Sticker pack".into()),
        customer_email: Some("fan@example.com".into()),
        metadata: Some(serde_json::json!({ "orderId": "1042" })),
        idempotency_key: Some("order_1042".into()), // optional — generated if None
        ..Default::default()
    })
    .await?;

println!("{}", payment.status); // completed
```

### Payments

```rust
let p = serika.payments().retrieve("pay_m1x2y3").await?;
let page = serika.payments().list(Some(10)).await?;       // page.data, page.has_more
serika.payments().refund("pay_m1x2y3", None).await?;       // full refund
serika.payments().refund("pay_m1x2y3", Some(2.00)).await?; // partial
```

### Balance and transactions

```rust
let balance = serika.balance().retrieve().await?;               // EUR
let usd = serika.balance().retrieve_currency("USD").await?;
let txs = serika.transactions().list(Some(20), Some("spend")).await?;
```

### Webhooks (axum)

```rust
use axum::{body::Bytes, http::{HeaderMap, StatusCode}};
use serikapay::webhook;

async fn serika_webhook(headers: HeaderMap, body: Bytes) -> StatusCode {
    let sig = headers.get("serika-signature").and_then(|v| v.to_str().ok()).unwrap_or("");
    let secret = std::env::var("SERIKA_WEBHOOK_SECRET").unwrap();
    match webhook::construct_event(&body, sig, &secret) {
        Ok(event) if event.event_type == "payment.completed" => {
            let paid = event.payment().unwrap();
            // fulfil the order
            StatusCode::OK
        }
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::BAD_REQUEST,
    }
}
```

### Errors

```rust
match serika.payments().create(CreatePayment { amount: 4.99, ..Default::default() }).await {
    Ok(payment) => println!("{}", payment.id),
    Err(serikapay::Error::Api(e)) if e.error_type == "insufficient_balance" => { /* top up */ }
    Err(serikapay::Error::Connection(e)) => eprintln!("network: {e}"),
    Err(e) => return Err(e.into()),
}
```

## Configuration

```rust
let serika = Client::builder(key)
    .base_url("https://wallet-api.serika.dev") // default
    .timeout(std::time::Duration::from_secs(30))
    .max_retries(2)                             // network errors, 429 and 5xx
    .build();
```

`GET` requests and payment creation (which always carries an idempotency key) are retried with exponential backoff; refunds are not. `Client` is cheap to clone.

## Development

```bash
cargo test
```

## License

MIT
