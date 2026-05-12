//! Integration tests for sortable_columns configuration.
//!
//! Verifies that each entity's declared sortable columns are accepted and that
//! undeclared columns are rejected with 400.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

// ============================================================================
// KEYWORDS — the bug-fix target (name, not name_unicode)
// ============================================================================

#[tokio::test]
async fn test_keyword_sort_by_name_asc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let names = ["Zebra", "Alpha", "Middle"];
    for (i, name) in names.iter().enumerate() {
        let resp = app
            .post_json(
                "/api/v1/keywords",
                &json!({
                    "keyword_key": format!("{}:{}-{}", i + 1, name, suffix),
                    "name": format!("{}-{}", name, suffix),
                    "level": 1
                }),
            )
            .await;
        assert_eq!(resp.status(), 200, "Failed to create keyword {name}");
    }

    let resp = app
        .get(&format!(
            "/api/v1/keywords?sort_by=name&sort_dir=asc&keyword_key={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert_eq!(items.len(), 3);

    let returned_names: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
    for window in returned_names.windows(2) {
        assert!(
            window[0] <= window[1],
            "Expected ascending order, got {:?}",
            returned_names
        );
    }
}

#[tokio::test]
async fn test_keyword_sort_by_name_desc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let names = ["Zebra", "Alpha", "Middle"];
    for (i, name) in names.iter().enumerate() {
        let resp = app
            .post_json(
                "/api/v1/keywords",
                &json!({
                    "keyword_key": format!("{}:{}-{}", i + 1, name, suffix),
                    "name": format!("{}-{}", name, suffix),
                    "level": 1
                }),
            )
            .await;
        assert_eq!(resp.status(), 200);
    }

    let resp = app
        .get(&format!(
            "/api/v1/keywords?sort_by=name&sort_dir=desc&keyword_key={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert_eq!(items.len(), 3);

    let returned_names: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
    for window in returned_names.windows(2) {
        assert!(
            window[0] >= window[1],
            "Expected descending order, got {:?}",
            returned_names
        );
    }
}

#[tokio::test]
async fn test_keyword_sort_by_keyword_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/keywords?sort_by=keyword_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_keyword_sort_by_level() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/keywords?sort_by=level&sort_dir=asc").await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_keyword_sort_by_name_unicode_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/keywords?sort_by=name_unicode").await;
    assert_eq!(
        resp.status(),
        400,
        "Keyword has no name_unicode column; sort should be rejected"
    );
}

#[tokio::test]
async fn test_keyword_sort_by_nonexistent_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/keywords?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

// ============================================================================
// AUTHORS
// ============================================================================

#[tokio::test]
async fn test_author_sort_by_family_name_unicode() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for (key, family) in [("z", "Zeno"), ("a", "Aristotle"), ("m", "Marx")] {
        let resp = app
            .post_json(
                "/api/v1/authors",
                &json!({
                    "author_key": format!("{key}-{suffix}"),
                    "family_name_latex": family,
                    "family_name_unicode": family,
                    "family_name_simplified": family.to_lowercase()
                }),
            )
            .await;
        assert_eq!(resp.status(), 200, "Failed to create author {family}");
    }

    let resp = app
        .get(&format!(
            "/api/v1/authors?sort_by=family_name_unicode&sort_dir=asc&family_name={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert_eq!(items.len(), 3);

    let returned: Vec<&str> = items
        .iter()
        .filter_map(|i| i["family_name_unicode"].as_str())
        .collect();
    for window in returned.windows(2) {
        assert!(
            window[0] <= window[1],
            "Expected ascending order, got {:?}",
            returned
        );
    }
}

#[tokio::test]
async fn test_author_sort_by_author_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/authors?sort_by=author_key&sort_dir=desc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_author_sort_by_given_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/authors?sort_by=given_name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_author_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/authors?sort_by=name_unicode").await;
    assert_eq!(
        resp.status(),
        400,
        "Author has no name_unicode; sort should be rejected"
    );
}

// ============================================================================
// JOURNALS
// ============================================================================

#[tokio::test]
async fn test_journal_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/journals?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_journal_sort_by_journal_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/journals?sort_by=journal_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// PUBLISHERS
// ============================================================================

#[tokio::test]
async fn test_publisher_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/publishers?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_publisher_sort_by_publisher_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/publishers?sort_by=publisher_key&sort_dir=desc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// INSTITUTIONS
// ============================================================================

#[tokio::test]
async fn test_institution_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/institutions?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_institution_sort_by_institution_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/institutions?sort_by=institution_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// SCHOOLS
// ============================================================================

#[tokio::test]
async fn test_school_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/schools?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_school_sort_by_school_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/schools?sort_by=school_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// SERIES
// ============================================================================

#[tokio::test]
async fn test_series_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/series?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_series_sort_by_series_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/series?sort_by=series_key&sort_dir=desc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// BIBITEMS
// ============================================================================

#[tokio::test]
async fn test_bibitem_sort_by_bibkey() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/bibitems?sort_by=bibkey&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_bibitem_sort_by_title_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/bibitems?sort_by=title_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_bibitem_sort_by_entry_type() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/bibitems?sort_by=entry_type&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_bibitem_sort_by_date_year() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/bibitems?sort_by=date_year&sort_dir=desc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_bibitem_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/bibitems?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}
