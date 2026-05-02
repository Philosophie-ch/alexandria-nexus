//! Integration tests for POST /api/v1/admin/compute-start-pages.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

async fn post_compute_start_pages(app: &TestApp) -> reqwest::Response {
    app.client
        .post(app.url("/api/v1/admin/compute-start-pages"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .expect("Failed to POST /admin/compute-start-pages")
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_compute_start_pages_requires_auth() {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .post(app.url("/api/v1/admin/compute-start-pages"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

// ── Empty DB ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_compute_start_pages_empty_db_returns_zero() {
    let app = TestApp::spawn().await;
    let resp = post_compute_start_pages(&app).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["updated"], 0, "Empty DB should report 0 updated");
}

// ── Computation ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_compute_start_pages_numeric_pages() {
    let app = TestApp::spawn().await;
    let bibkey = format!("csp-num-{}:2024", unique_suffix());

    let create = app
        .post_json(
            "/api/v1/bibitems",
            &json!({
                "bibkey": &bibkey,
                "entry_type": "article",
                "title_latex": "Test",
                "title_unicode": "Test",
                "pages": "123--456",
            }),
        )
        .await;
    assert_eq!(create.status(), 200, "Failed to create bibitem");
    let id = create.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    let resp = post_compute_start_pages(&app).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["updated"], 1);

    let get = app.get(&format!("/api/v1/bibitems/{id}")).await;
    assert_eq!(get.status(), 200);
    let bib: serde_json::Value = get.json().await.unwrap();
    assert_eq!(
        bib["start_page"], 123,
        "start_page should be 123 for pages=123--456"
    );
}

#[tokio::test]
async fn test_compute_start_pages_roman_numeral_pages() {
    let app = TestApp::spawn().await;
    let bibkey = format!("csp-rom-{}:2024", unique_suffix());

    let create = app
        .post_json(
            "/api/v1/bibitems",
            &json!({
                "bibkey": &bibkey,
                "entry_type": "article",
                "title_latex": "Test",
                "title_unicode": "Test",
                "pages": "xii--xiv",
            }),
        )
        .await;
    assert_eq!(create.status(), 200);
    let id = create.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    let resp = post_compute_start_pages(&app).await;
    assert_eq!(resp.status(), 200);

    let bib: serde_json::Value = app
        .get(&format!("/api/v1/bibitems/{id}"))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(
        bib["start_page"], 12,
        "start_page should be 12 for Roman numeral pages=xii--xiv"
    );
}

#[tokio::test]
async fn test_compute_start_pages_null_pages_produces_null() {
    let app = TestApp::spawn().await;
    let bibkey = format!("csp-null-{}:2024", unique_suffix());

    let create = app
        .post_json(
            "/api/v1/bibitems",
            &json!({
                "bibkey": &bibkey,
                "entry_type": "article",
                "title_latex": "Test",
                "title_unicode": "Test",
            }),
        )
        .await;
    assert_eq!(create.status(), 200);
    let id = create.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    let resp = post_compute_start_pages(&app).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["updated"], 1);

    let bib: serde_json::Value = app
        .get(&format!("/api/v1/bibitems/{id}"))
        .await
        .json()
        .await
        .unwrap();
    assert!(
        bib["start_page"].is_null(),
        "start_page should be null when pages is null"
    );
}

#[tokio::test]
async fn test_compute_start_pages_idempotent() {
    let app = TestApp::spawn().await;
    let bibkey = format!("csp-idem-{}:2024", unique_suffix());

    let create = app
        .post_json(
            "/api/v1/bibitems",
            &json!({
                "bibkey": &bibkey,
                "entry_type": "article",
                "title_latex": "Test",
                "title_unicode": "Test",
                "pages": "50--60",
            }),
        )
        .await;
    assert_eq!(create.status(), 200);
    let id = create.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    // First run
    let resp1 = post_compute_start_pages(&app).await;
    assert_eq!(resp1.status(), 200);
    let body1: serde_json::Value = resp1.json().await.unwrap();
    assert_eq!(body1["updated"], 1);

    // Second run — same result
    let resp2 = post_compute_start_pages(&app).await;
    assert_eq!(resp2.status(), 200);
    let body2: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body2["updated"], 1);

    let bib: serde_json::Value = app
        .get(&format!("/api/v1/bibitems/{id}"))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(
        bib["start_page"], 50,
        "start_page should still be 50 after idempotent re-run"
    );
}
