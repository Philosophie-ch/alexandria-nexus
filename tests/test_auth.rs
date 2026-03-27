//! Authentication and permission tests.

mod common;

use common::TestApp;

#[tokio::test]
async fn test_no_auth_returns_forbidden() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/api/v1/authors").await;

    assert_eq!(
        resp.status(),
        403,
        "Expected 403 Forbidden without Bearer token, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_valid_auth_returns_ok() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/authors").await;

    assert_eq!(
        resp.status(),
        200,
        "Expected 200 OK with valid Bearer token, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_invalid_auth_returns_forbidden() {
    let app = TestApp::spawn().await;

    let resp = app
        .get_with_token("/api/v1/authors", "totally-wrong-key-99999")
        .await;

    assert_eq!(
        resp.status(),
        403,
        "Expected 403 Forbidden with invalid Bearer token, got {}",
        resp.status()
    );
}
