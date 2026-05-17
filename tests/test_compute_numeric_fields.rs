//! Integration tests for POST /api/v1/admin/compute-numeric-fields.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

async fn post_compute_numeric_fields(app: &TestApp) -> reqwest::Response {
    app.client
        .post(app.url("/api/v1/admin/compute-numeric-fields"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .expect("Failed to POST /admin/compute-numeric-fields")
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_compute_numeric_fields_requires_auth() {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .post(app.url("/api/v1/admin/compute-numeric-fields"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

// ── Empty DB ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_compute_numeric_fields_empty_db_returns_zero() {
    let app = TestApp::spawn().await;
    let resp = post_compute_numeric_fields(&app).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["updated"], 0, "Empty DB should report 0 updated");
}

// ── Computation ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_compute_numeric_fields_numeric_pages() {
    let app = TestApp::spawn().await;
    let bibkey = format!("cnf-num-{}:2024", unique_suffix());

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

    let resp = post_compute_numeric_fields(&app).await;
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
async fn test_compute_numeric_fields_roman_numeral_pages() {
    let app = TestApp::spawn().await;
    let bibkey = format!("cnf-rom-{}:2024", unique_suffix());

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

    let resp = post_compute_numeric_fields(&app).await;
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
async fn test_compute_numeric_fields_null_pages_produces_null() {
    let app = TestApp::spawn().await;
    let bibkey = format!("cnf-null-{}:2024", unique_suffix());

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

    let resp = post_compute_numeric_fields(&app).await;
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
async fn test_compute_numeric_fields_populates_volume_and_number_numeric() {
    let app = TestApp::spawn().await;
    let bibkey = format!("cnf-vnm-{}:2024", unique_suffix());

    let create = app
        .post_json(
            "/api/v1/bibitems",
            &json!({
                "bibkey": &bibkey,
                "entry_type": "article",
                "title_latex": "Test",
                "title_unicode": "Test",
                "pages": "100--200",
                "volume": "s2-4",
                "number": "3/4",
            }),
        )
        .await;
    assert_eq!(create.status(), 200);
    let id = create.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();

    let resp = post_compute_numeric_fields(&app).await;
    assert_eq!(resp.status(), 200);

    let bib: serde_json::Value = app
        .get(&format!("/api/v1/bibitems/{id}"))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(bib["start_page"], 100);
    assert_eq!(
        bib["volume_numeric"], 2004,
        "s2-4 should produce volume_numeric=2004"
    );
    assert_eq!(
        bib["number_numeric"], 3,
        "3/4 should produce number_numeric=3"
    );
}

#[tokio::test]
async fn test_compute_numeric_fields_idempotent() {
    let app = TestApp::spawn().await;
    let bibkey = format!("cnf-idem-{}:2024", unique_suffix());

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
    let resp1 = post_compute_numeric_fields(&app).await;
    assert_eq!(resp1.status(), 200);
    let body1: serde_json::Value = resp1.json().await.unwrap();
    assert_eq!(body1["updated"], 1);

    // Second run — same result
    let resp2 = post_compute_numeric_fields(&app).await;
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
