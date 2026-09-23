use serde_json::{json, Value};
use serikapay::{webhook, Client, CreatePayment, Error};
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn payment() -> Value {
    json!({
        "id": "pay_m1x2y3", "object": "payment", "amount": 4.99, "currency": "EUR", "status": "completed",
        "description": "Sticker pack", "customerEmail": null, "customerId": null, "metadata": { "orderId": "1042" },
        "createdAt": "2026-09-23T12:00:00.000Z", "completedAt": "2026-09-23T12:00:00.120Z", "expiresAt": null
    })
}

fn client(server: &MockServer) -> Client {
    Client::builder("sk_test_abc12345")
        .base_url(server.uri())
        .max_retries(2)
        .build()
}

#[tokio::test]
async fn create_payment() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/payments"))
        .and(header("authorization", "Bearer sk_test_abc12345"))
        .and(header("x-idempotency-key", "order_1042"))
        .and(body_json(json!({ "amount": 4.99, "description": "Sticker pack", "metadata": { "orderId": "1042" } })))
        .respond_with(ResponseTemplate::new(200).set_body_json(payment()))
        .expect(1)
        .mount(&server)
        .await;

    let p = client(&server)
        .payments()
        .create(CreatePayment {
            amount: 4.99,
            description: Some("Sticker pack".into()),
            metadata: Some(json!({ "orderId": "1042" })),
            idempotency_key: Some("order_1042".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(p.id, "pay_m1x2y3");
    assert_eq!(p.metadata["orderId"], "1042");

    let reqs = server.received_requests().await.unwrap();
    assert!(reqs[0]
        .headers
        .get("user-agent")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("serikapay-rust/"));
}

#[tokio::test]
async fn create_generates_idempotency_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payment()))
        .mount(&server)
        .await;
    client(&server)
        .payments()
        .create(CreatePayment {
            amount: 1.0,
            ..Default::default()
        })
        .await
        .unwrap();
    let reqs = server.received_requests().await.unwrap();
    assert!(reqs[0].headers.get("x-idempotency-key").unwrap().len() >= 16);
}

#[tokio::test]
async fn routes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/payments/pay_m1x2y3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payment()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/payments"))
        .and(query_param("limit", "3"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "object": "list", "data": [payment()], "hasMore": false })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/payments/pay_m1x2y3/refund"))
        .and(body_json(json!({ "amount": 2.0 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(payment()))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    c.payments().retrieve("pay_m1x2y3").await.unwrap();
    assert_eq!(c.payments().list(Some(3)).await.unwrap().data.len(), 1);
    c.payments().refund("pay_m1x2y3", Some(2.0)).await.unwrap();
}

#[tokio::test]
async fn balance_and_transactions() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/balance"))
        .and(query_param("currency", "EUR"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "object": "balance", "available": 10, "reserved": 0, "total": 10, "currency": "EUR" })))
        .mount(&server)
        .await;
    Mock::given(path("/v1/transactions"))
        .and(query_param("limit", "20"))
        .and(query_param("type", "spend"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "object": "list", "data": [] })),
        )
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    assert_eq!(c.balance().retrieve().await.unwrap().available, 10.0);
    c.transactions()
        .list(Some(20), Some("spend"))
        .await
        .unwrap();
}

#[tokio::test]
async fn api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "error": { "type": "insufficient_balance", "message": "Insufficient", "available": 3 } })))
        .mount(&server)
        .await;
    match client(&server)
        .payments()
        .create(CreatePayment {
            amount: 4.99,
            ..Default::default()
        })
        .await
    {
        Err(Error::Api(e)) => {
            assert_eq!(e.error_type, "insufficient_balance");
            assert_eq!(e.status, 400);
            assert_eq!(e.raw["available"], 3);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(payment()))
        .mount(&server)
        .await;
    client(&server)
        .payments()
        .retrieve("pay_m1x2y3")
        .await
        .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 2);

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    assert!(client(&server)
        .payments()
        .refund("pay_m1x2y3", None)
        .await
        .is_err());
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn connection_error() {
    let c = Client::builder("sk_test_abc12345")
        .base_url("http://127.0.0.1:1")
        .max_retries(0)
        .build();
    assert!(matches!(
        c.balance().retrieve().await,
        Err(Error::Connection(_))
    ));
}

const PAYLOAD: &str = r#"{"id":"evt_1","object":"event","type":"payment.completed","createdAt":"2026-09-23T12:00:00.000Z","data":{"id":"pay_m1x2y3","amount":4.99}}"#;
const SECRET: &str = "whsec_test_secret";
const SIG: &str = "sha256=87f1961717e2bf9fc53d344cf0bcd0b8a22a8f99c40ac890ed7ab48554f47415";

#[test]
fn webhooks() {
    assert_eq!(webhook::generate_signature(PAYLOAD, SECRET), SIG);
    let event = webhook::construct_event(PAYLOAD, SIG, SECRET).unwrap();
    assert_eq!(event.event_type, "payment.completed");
    assert_eq!(event.data["id"], "pay_m1x2y3");
    assert!(matches!(
        webhook::construct_event(PAYLOAD.replace("4.99", "499"), SIG, SECRET),
        Err(Error::InvalidSignature)
    ));
    assert!(matches!(
        webhook::construct_event(PAYLOAD, SIG, "nope"),
        Err(Error::InvalidSignature)
    ));
    assert!(matches!(
        webhook::construct_event(PAYLOAD, "", SECRET),
        Err(Error::InvalidSignature)
    ));
}
