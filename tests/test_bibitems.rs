//! BibItem CRUD and lookup integration tests.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

// ============================================================================
// CREATE
// ============================================================================

#[tokio::test]
async fn test_create_bibitem() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let payload = json!({
        "bibkey": format!("test:article-{}", suffix),
        "entry_type": "article",
        "title_latex": "On the Critique of Pure Reason",
        "title_unicode": "On the Critique of Pure Reason",
        "title_simplified": "on the critique of pure reason",
        "date_year": 1781
    });

    let resp = app.post_json("/api/v1/bibitems", &payload).await;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(status, 200, "Create bibitem failed: {body}");
    assert!(body["id"].as_i64().is_some(), "Response should contain id");
    assert_eq!(body["bibkey"], format!("test:article-{}", suffix));
    assert_eq!(body["entry_type"], "article");
    assert_eq!(body["date_year"], 1781);
}

// ============================================================================
// BY-BIBKEY LOOKUP
// ============================================================================

#[tokio::test]
async fn test_get_bibitem_by_bibkey() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();
    let bibkey = format!("test:lookup-{}", suffix);

    // Create
    let payload = json!({
        "bibkey": &bibkey,
        "entry_type": "book",
        "title_latex": "Tractatus Logico-Philosophicus",
        "title_unicode": "Tractatus Logico-Philosophicus",
        "title_simplified": "tractatus logico-philosophicus",
        "date_year": 1921
    });
    let create_resp = app.post_json("/api/v1/bibitems", &payload).await;
    assert_eq!(create_resp.status(), 200);

    // Lookup by bibkey
    let resp = app
        .get(&format!("/api/v1/bibitems/by-key/{}", bibkey))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["bibkey"], bibkey);
    assert_eq!(body["title_unicode"], "Tractatus Logico-Philosophicus");
}

// ============================================================================
// LIST WITH FILTERS
// ============================================================================

#[tokio::test]
async fn test_list_bibitems_with_filter() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create an article from 2020
    let payload1 = json!({
        "bibkey": format!("test:filter-a-{}", suffix),
        "entry_type": "article",
        "title_latex": "Modern Philosophy Today",
        "title_unicode": "Modern Philosophy Today",
        "title_simplified": "modern philosophy today",
        "date_year": 2020
    });
    // Create a book from 1900
    let payload2 = json!({
        "bibkey": format!("test:filter-b-{}", suffix),
        "entry_type": "book",
        "title_latex": "The Old Treatise",
        "title_unicode": "The Old Treatise",
        "title_simplified": "the old treatise",
        "date_year": 1900
    });

    app.post_json("/api/v1/bibitems", &payload1).await;
    app.post_json("/api/v1/bibitems", &payload2).await;

    // Filter by entry_type=article
    let resp = app.get("/api/v1/bibitems?entry_type=article").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert!(
        items.iter().all(|i| i["entry_type"] == "article"),
        "All filtered items should be articles"
    );

    // Filter by year range
    let resp2 = app
        .get("/api/v1/bibitems?year_from=2000&year_to=2025")
        .await;
    assert_eq!(resp2.status(), 200);
    let body2: serde_json::Value = resp2.json().await.unwrap();
    let items2 = body2["items"].as_array().expect("Expected items array");
    for item in items2 {
        let year = item["date_year"].as_i64().unwrap();
        assert!(
            (2000..=2025).contains(&year),
            "Year {} should be in 2000..=2025",
            year
        );
    }
}
