//! Health check and OpenAPI endpoint tests.

mod common;

use common::TestApp;

#[tokio::test]
async fn test_health_returns_ok() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/health").await;

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn test_openapi_returns_json() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    // OpenAPI spec should have an "openapi" version field and "info"
    assert!(
        body.get("openapi").is_some(),
        "Missing openapi version field"
    );
    assert!(body.get("info").is_some(), "Missing info field");
}
