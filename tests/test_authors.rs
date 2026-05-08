//! Author CRUD integration tests.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

// ============================================================================
// LIST
// ============================================================================

#[tokio::test]
async fn test_list_authors_empty() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/authors").await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert!(items.is_empty(), "Expected empty items, got {:?}", items);
}

// ============================================================================
// CREATE
// ============================================================================

#[tokio::test]
async fn test_create_author() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let payload = json!({
        "author_key": format!("kant-{}", suffix),
        "family_name_latex": "Kant",
        "family_name_unicode": "Kant",
        "family_name_simplified": "kant",
        "given_name_latex": "Immanuel",
        "given_name_unicode": "Immanuel",
        "given_name_simplified": "immanuel"
    });

    let resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(resp.status(), 200, "Create author should return 200");

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].as_i64().is_some(), "Response should contain id");
    assert_eq!(body["author_key"], format!("kant-{}", suffix));
    assert_eq!(body["family_name_latex"], "Kant");
}

// ============================================================================
// GET BY ID
// ============================================================================

#[tokio::test]
async fn test_get_author_by_id() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create
    let payload = json!({
        "author_key": format!("hegel-{}", suffix),
        "family_name_latex": "Hegel",
        "given_name_latex": "Georg Wilhelm Friedrich"
    });
    let create_resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(create_resp.status(), 200);
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let id = created["id"].as_i64().unwrap();

    // Get
    let resp = app.get(&format!("/api/v1/authors/{}", id)).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], id);
    assert_eq!(body["author_key"], format!("hegel-{}", suffix));
}

// ============================================================================
// UPDATE
// ============================================================================

#[tokio::test]
async fn test_update_author() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create
    let payload = json!({
        "author_key": format!("nietzsche-{}", suffix),
        "family_name_latex": "Nietzsche",
        "given_name_latex": "Friedrich"
    });
    let create_resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(create_resp.status(), 200);
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let id = created["id"].as_i64().unwrap();

    // Update
    let update_payload = json!({
        "given_name_unicode": "Friedrich Wilhelm"
    });
    let resp = app
        .put_json(&format!("/api/v1/authors/{}", id), &update_payload)
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["given_name_unicode"], "Friedrich Wilhelm");
    // Original fields should be preserved
    assert_eq!(body["family_name_latex"], "Nietzsche");
}

// ============================================================================
// DELETE
// ============================================================================

#[tokio::test]
async fn test_delete_author() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create
    let payload = json!({
        "author_key": format!("delete-me-{}", suffix),
        "family_name_latex": "ToDelete"
    });
    let create_resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(create_resp.status(), 200);
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let id = created["id"].as_i64().unwrap();

    // Delete
    let resp = app.delete(&format!("/api/v1/authors/{}", id)).await;
    assert_eq!(resp.status(), 204);

    // Verify deleted
    let get_resp = app.get(&format!("/api/v1/authors/{}", id)).await;
    assert_eq!(get_resp.status(), 404);
}

// ============================================================================
// BY-KEY LOOKUP
// ============================================================================

#[tokio::test]
async fn test_get_author_by_key() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();
    let key = format!("plato-{}", suffix);

    // Create
    let payload = json!({
        "author_key": &key,
        "mononym_latex": "Plato",
        "mononym_unicode": "Plato",
        "mononym_simplified": "plato"
    });
    let create_resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(create_resp.status(), 200);

    // Lookup by key
    let resp = app.get(&format!("/api/v1/authors/by-key/{}", key)).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["author_key"], key);
    assert_eq!(body["mononym_latex"], "Plato");
}

// ============================================================================
// FILTER
// ============================================================================

#[tokio::test]
async fn test_list_authors_with_filter() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create two authors with different family names
    let payload1 = json!({
        "author_key": format!("filter-a-{}", suffix),
        "family_name_latex": "Schopenhauer",
        "family_name_simplified": "schopenhauer"
    });
    let payload2 = json!({
        "author_key": format!("filter-b-{}", suffix),
        "family_name_latex": "Wittgenstein",
        "family_name_simplified": "wittgenstein"
    });

    app.post_json("/api/v1/authors", &payload1).await;
    app.post_json("/api/v1/authors", &payload2).await;

    // Filter by family_name
    let resp = app.get("/api/v1/authors?family_name=schopenhauer").await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert_eq!(items.len(), 1, "Filter should return exactly 1 author");
    assert_eq!(items[0]["family_name_unicode"], "Schopenhauer");
}

// ============================================================================
// BATCH LOOKUP BY AUTHOR KEY
// ============================================================================

#[tokio::test]
async fn test_list_authors_by_author_keys() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let key_a = format!("batch-kant-{}", suffix);
    let key_b = format!("batch-hegel-{}", suffix);
    let key_c = format!("batch-fichte-{}", suffix);

    for (key, name) in [(&key_a, "Kant"), (&key_b, "Hegel"), (&key_c, "Fichte")] {
        let resp = app
            .post_json(
                "/api/v1/authors",
                &json!({
                    "author_key": key,
                    "family_name_latex": name,
                    "family_name_simplified": name.to_lowercase()
                }),
            )
            .await;
        assert_eq!(resp.status(), 200, "Failed to create author {key}");
    }

    // Fetch two of the three by author_key
    let resp = app
        .get(&format!(
            "/api/v1/authors?author_keys[]={}&author_keys[]={}",
            key_a, key_c
        ))
        .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");

    let returned_keys: Vec<&str> = items
        .iter()
        .filter_map(|i| i["author_key"].as_str())
        .collect();
    assert_eq!(
        returned_keys.len(),
        2,
        "Expected exactly 2 authors, got {returned_keys:?}"
    );
    assert!(returned_keys.contains(&key_a.as_str()), "Missing {key_a}");
    assert!(returned_keys.contains(&key_c.as_str()), "Missing {key_c}");
    assert!(
        !returned_keys.contains(&key_b.as_str()),
        "Should not include {key_b}"
    );
}
