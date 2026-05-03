//! Integration tests for POST /api/v1/admin/wipe.

mod common;

use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn test_wipe_requires_confirm() {
    let app = TestApp::spawn().await;

    let resp = app
        .client
        .post(app.url("/api/v1/admin/wipe"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_wipe_deletes_all_data() {
    let app = TestApp::spawn().await;

    let payload = json!({
        "author_key": "kant-wipe-test",
        "family_name_latex": "Kant",
        "given_name_latex": "Immanuel"
    });
    let resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(resp.status(), 200);

    let resp = app
        .client
        .post(app.url("/api/v1/admin/wipe?confirm=true"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = app.get("/api/v1/authors").await;
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert!(items.is_empty(), "Expected no authors after wipe");
}

#[tokio::test]
async fn test_wipe_resets_sequences() {
    let app = TestApp::spawn().await;

    // Create an author — consumes id=1 (and possibly higher)
    let payload = json!({
        "author_key": "pre-wipe-author",
        "family_name_latex": "PreWipe",
        "given_name_latex": "Author"
    });
    let resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(resp.status(), 200);
    let pre: serde_json::Value = resp.json().await.unwrap();
    let pre_id = pre["id"].as_i64().unwrap();
    assert_eq!(pre_id, 1);

    // Wipe — should RESTART IDENTITY so sequences go back to 1
    let resp = app
        .client
        .post(app.url("/api/v1/admin/wipe?confirm=true"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Create a new author — should get id=1 again, not id=2
    let payload = json!({
        "author_key": "post-wipe-author",
        "family_name_latex": "PostWipe",
        "given_name_latex": "Author"
    });
    let resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(resp.status(), 200);
    let post: serde_json::Value = resp.json().await.unwrap();
    let post_id = post["id"].as_i64().unwrap();
    assert_eq!(
        post_id, 1,
        "After wipe, sequence should restart from 1 but got id={post_id}"
    );
}
