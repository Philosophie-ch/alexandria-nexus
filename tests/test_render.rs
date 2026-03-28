//! Integration tests for the HTML bibliography render endpoint.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

/// Helper: create an author via the API and return (id, key).
async fn create_author(
    app: &TestApp,
    suffix: &str,
    key_prefix: &str,
    given: &str,
    family: &str,
) -> (i64, String) {
    let key = format!("{key_prefix}-{suffix}");
    let payload = json!({
        "author_key": &key,
        "family_name_latex": family,
        "family_name_unicode": family,
        "family_name_simplified": family.to_lowercase(),
        "given_name_latex": given,
        "given_name_unicode": given,
        "given_name_simplified": given.to_lowercase()
    });
    let resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to create author {key}");
    let body: serde_json::Value = resp.json().await.unwrap();
    (body["id"].as_i64().unwrap(), key)
}

/// Helper: create a bibitem and return its id.
async fn create_bibitem_with_details(
    app: &TestApp,
    suffix: &str,
    bibkey_prefix: &str,
    entry_type: &str,
    title: &str,
    year: Option<i16>,
) -> i64 {
    let mut payload = json!({
        "bibkey": format!("{bibkey_prefix}-{suffix}"),
        "entry_type": entry_type,
        "title_latex": title,
        "title_unicode": title,
        "title_simplified": title.to_lowercase()
    });
    if let Some(y) = year {
        payload["date_year"] = json!(y);
    }
    let resp = app.post_json("/api/v1/bibitems", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to create bibitem");
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"].as_i64().unwrap()
}

/// Helper: link an author to a bibitem via the junction table.
async fn link_author(app: &TestApp, bibitem_id: i64, author_id: i64, role: &str, position: i16) {
    let payload = json!({
        "author_id": author_id,
        "role": role,
        "position": position
    });
    let resp = app
        .post_json(&format!("/api/v1/bibitems/{bibitem_id}/authors"), &payload)
        .await;
    assert!(
        resp.status().is_success(),
        "Failed to link author {author_id} to bibitem {bibitem_id}: {}",
        resp.status()
    );
}

// =============================================================================
// Test: basic render with a single article
// =============================================================================

#[tokio::test]
async fn test_render_single_article() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create author and bibitem
    let (author_id, _) = create_author(&app, &suffix, "render-author", "Jane", "Smith").await;
    let bibitem_id =
        create_bibitem_with_details(&app, &suffix, "smith", "article", "Some Title", Some(2024))
            .await;

    // Link author
    link_author(&app, bibitem_id, author_id, "author", 1).await;

    // Render by bibkey
    let bibkey = format!("smith-{suffix}");
    let resp = app
        .post_json("/api/v1/render", &json!({ "bibkeys": [bibkey] }))
        .await;
    assert_eq!(resp.status(), 200);

    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("text/html"),
        "Expected text/html, got {content_type}"
    );

    let body = resp.text().await.unwrap();
    assert!(body.contains("data-type=\"article\""), "entry type present");
    assert!(
        body.contains(&format!("data-bibkey=\"smith-{suffix}\"")),
        "bibkey present"
    );
    assert!(
        body.contains("class=\"smallcaps\">Smith</span>"),
        "author in smallcaps"
    );
    assert!(
        body.contains("data-field=\"date\">2024</span>"),
        "year present"
    );
    assert!(body.contains("Some Title"), "title present");
}

// =============================================================================
// Test: request too many items (> 1000)
// =============================================================================

#[tokio::test]
async fn test_render_too_many_items() {
    let app = TestApp::spawn().await;

    // Create a list of 1001 fake IDs
    let ids: Vec<i64> = (1..=1001).collect();
    let resp = app
        .post_json("/api/v1/render", &json!({ "ids": ids }))
        .await;
    assert_eq!(resp.status(), 422, "Should reject > 1000 items");

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "too_many_items");
}

// =============================================================================
// Test: missing bibkeys returns 422 with all missing
// =============================================================================

#[tokio::test]
async fn test_render_missing_bibkeys() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": ["nonexistent:2024", "also-missing:2025"] }),
        )
        .await;
    assert_eq!(resp.status(), 422, "Should return 422 for missing bibkeys");

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "not_found");
    let missing = body["missing_bibkeys"].as_array().unwrap();
    assert_eq!(missing.len(), 2, "Should report all missing bibkeys");
}
