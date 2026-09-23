//! Official SerikaPay library for Rust.
//!
//! ```no_run
//! # async fn run() -> Result<(), serikapay::Error> {
//! use serikapay::{Client, CreatePayment};
//!
//! let serika = Client::new("sk_test_...");
//! let payment = serika
//!     .payments()
//!     .create(CreatePayment { amount: 4.99, description: Some("Sticker pack".into()), ..Default::default() })
//!     .await?;
//! println!("{} {}", payment.id, payment.status);
//! # Ok(()) }
//! ```
//!
//! See <https://wallet.serika.dev/developers/docs>.

mod client;
mod error;
mod models;
pub mod webhook;

pub use client::{
    Balances, Client, ClientBuilder, Payments, Transactions, DEFAULT_BASE_URL, VERSION,
};
pub use error::{ApiError, Error};
pub use models::*;
