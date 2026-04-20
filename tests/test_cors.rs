//! CORS configuration tests.
//!
//! Verifies that `CorsConfig::origins(...)` correctly restricts cross-origin access
//! and that permissive mode allows any origin.
//!
//! CORS middleware is applied to API routes (`/api/v1/*`). The `/health` endpoint
//! is mounted on the outer router and is intentionally outside the CORS layer
//! (health checks are for infrastructure, not browser clients).

mod common;

use common::TestApp;
use hexforge::CorsConfig;
use reqwest::Method;

const ALLOWED_ORIGIN: &str = "https://philosophie.ch";
const OTHER_ALLOWED_ORIGIN: &str = "https://alexandria.philosophie.ch";
const DISALLOWED_ORIGIN: &str = "https://evil.example.com";

// Use a stable, lightweight API route for all CORS checks.
// Keywords tree returns 200 with an empty list when no data is present.
const TEST_PATH: &str = "/api/v1/keywords/tree";

// =============================================================================
// Permissive CORS
// =============================================================================

#[tokio::test]
async fn test_permissive_cors_any_origin_gets_acao_header() {
    let app = TestApp::spawn().await;

    let resp = app
        .client
        .get(app.url(TEST_PATH))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .header("Origin", DISALLOWED_ORIGIN)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let acao = resp.headers().get("access-control-allow-origin");
    assert!(
        acao.is_some(),
        "Expected Access-Control-Allow-Origin header with permissive config"
    );
}

#[tokio::test]
async fn test_permissive_cors_preflight_succeeds_for_any_origin() {
    let app = TestApp::spawn().await;

    // Preflight is handled by CorsLayer before auth — no auth header needed.
    let resp = app
        .client
        .request(Method::OPTIONS, app.url(TEST_PATH))
        .header("Origin", DISALLOWED_ORIGIN)
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_success(),
        "Preflight should succeed with permissive CORS, got {}",
        resp.status()
    );
    let acao = resp.headers().get("access-control-allow-origin");
    assert!(
        acao.is_some(),
        "Expected ACAO header in preflight response with permissive config"
    );
}

// =============================================================================
// Restricted CORS
// =============================================================================

#[tokio::test]
async fn test_restricted_cors_allows_listed_origin() {
    let app =
        TestApp::spawn_with_cors(CorsConfig::origins([ALLOWED_ORIGIN, OTHER_ALLOWED_ORIGIN])).await;

    let resp = app
        .client
        .get(app.url(TEST_PATH))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .header("Origin", ALLOWED_ORIGIN)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok());
    assert_eq!(
        acao,
        Some(ALLOWED_ORIGIN),
        "Expected ACAO header to reflect allowed origin"
    );
}

#[tokio::test]
async fn test_restricted_cors_allows_second_listed_origin() {
    let app =
        TestApp::spawn_with_cors(CorsConfig::origins([ALLOWED_ORIGIN, OTHER_ALLOWED_ORIGIN])).await;

    let resp = app
        .client
        .get(app.url(TEST_PATH))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .header("Origin", OTHER_ALLOWED_ORIGIN)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok());
    assert_eq!(
        acao,
        Some(OTHER_ALLOWED_ORIGIN),
        "Expected ACAO header to reflect second allowed origin"
    );
}

#[tokio::test]
async fn test_restricted_cors_blocks_unlisted_origin() {
    let app = TestApp::spawn_with_cors(CorsConfig::origins([ALLOWED_ORIGIN])).await;

    let resp = app
        .client
        .get(app.url(TEST_PATH))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .header("Origin", DISALLOWED_ORIGIN)
        .send()
        .await
        .unwrap();

    // CORS doesn't block server-side — the request still returns 200.
    // What matters is that the ACAO header is absent or doesn't reflect the disallowed origin.
    assert_eq!(resp.status(), 200);
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok());
    assert!(
        acao != Some(DISALLOWED_ORIGIN),
        "ACAO header must not reflect a disallowed origin"
    );
}

#[tokio::test]
async fn test_restricted_cors_preflight_succeeds_for_allowed_origin() {
    let app = TestApp::spawn_with_cors(CorsConfig::origins([ALLOWED_ORIGIN])).await;

    let resp = app
        .client
        .request(Method::OPTIONS, app.url(TEST_PATH))
        .header("Origin", ALLOWED_ORIGIN)
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .unwrap();

    assert!(
        resp.status().is_success(),
        "Preflight should succeed for allowed origin, got {}",
        resp.status()
    );
    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok());
    assert_eq!(
        acao,
        Some(ALLOWED_ORIGIN),
        "Preflight ACAO header should reflect allowed origin"
    );
}

#[tokio::test]
async fn test_restricted_cors_preflight_blocked_for_disallowed_origin() {
    let app = TestApp::spawn_with_cors(CorsConfig::origins([ALLOWED_ORIGIN])).await;

    let resp = app
        .client
        .request(Method::OPTIONS, app.url(TEST_PATH))
        .header("Origin", DISALLOWED_ORIGIN)
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .unwrap();

    let acao = resp
        .headers()
        .get("access-control-allow-origin")
        .and_then(|v| v.to_str().ok());
    assert!(
        acao != Some(DISALLOWED_ORIGIN),
        "Preflight ACAO header must not reflect a disallowed origin"
    );
}
