//! Integration tests for POST /api/v1/admin/convert-latex-columns.
//!
//! Note: full LaTeX→Unicode conversion requires pylatexenc (a Python library
//! not available in the CI test environment). Tests here cover auth, response
//! structure, and idempotent no-op behaviour on an empty database — which does
//! not invoke the Python subprocess (no rows → no convert call).

mod common;

use common::TestApp;

async fn post_convert_latex_columns(app: &TestApp) -> reqwest::Response {
    app.client
        .post(app.url("/api/v1/admin/convert-latex-columns"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .expect("Failed to POST /admin/convert-latex-columns")
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_convert_latex_columns_requires_auth() {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .post(app.url("/api/v1/admin/convert-latex-columns"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

// ── Empty DB ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_convert_latex_columns_empty_db_returns_ok() {
    let app = TestApp::spawn().await;
    let resp = post_convert_latex_columns(&app).await;

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["total_updated"], 0,
        "No rows in DB — nothing should be updated"
    );
    assert_eq!(
        body["errors"].as_array().map(|a| a.len()),
        Some(0),
        "No errors expected on empty DB"
    );
    assert_eq!(
        body["missing_citation_keys"].as_array().map(|a| a.len()),
        Some(0),
        "No missing citation keys on empty DB"
    );

    // 15 column pairs (5 bibitem + 5 author + journals/publishers/institutions/schools/series)
    let columns = body["columns"]
        .as_array()
        .expect("columns should be an array");
    assert_eq!(columns.len(), 15, "Should report all 15 column pairs");
    for col in columns {
        assert_eq!(
            col["updated"], 0,
            "Each column should have 0 updates on empty DB"
        );
    }
}

#[tokio::test]
async fn test_convert_latex_columns_idempotent_on_empty_db() {
    let app = TestApp::spawn().await;

    let resp1 = post_convert_latex_columns(&app).await;
    assert_eq!(resp1.status(), 200);
    let body1: serde_json::Value = resp1.json().await.unwrap();

    let resp2 = post_convert_latex_columns(&app).await;
    assert_eq!(resp2.status(), 200);
    let body2: serde_json::Value = resp2.json().await.unwrap();

    assert_eq!(body1["total_updated"], body2["total_updated"]);
    assert_eq!(
        body1["columns"].as_array().map(|a| a.len()),
        body2["columns"].as_array().map(|a| a.len())
    );
}
