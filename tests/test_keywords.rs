//! Keyword CRUD and keyword tree integration tests.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

// ============================================================================
// CREATE
// ============================================================================

#[tokio::test]
async fn test_create_keyword() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let payload = json!({
        "name": format!("Ethics-{}", suffix),
        "level": 1
    });

    let resp = app.post_json("/api/v1/keywords", &payload).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].as_i64().is_some(), "Response should contain id");
    assert_eq!(body["name"], format!("Ethics-{}", suffix));
    assert_eq!(body["level"], 1);
}

// ============================================================================
// KEYWORD TREE
// ============================================================================

#[tokio::test]
async fn test_keyword_tree() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create keywords at all three levels
    let kw1 = json!({ "name": format!("Philosophy-{}", suffix), "level": 1 });
    let kw2 = json!({ "name": format!("Ethics-{}", suffix), "level": 2 });
    let kw3 = json!({ "name": format!("Virtue Ethics-{}", suffix), "level": 3 });

    let r1 = app.post_json("/api/v1/keywords", &kw1).await;
    assert_eq!(r1.status(), 200);
    let r2 = app.post_json("/api/v1/keywords", &kw2).await;
    assert_eq!(r2.status(), 200);
    let r3 = app.post_json("/api/v1/keywords", &kw3).await;
    assert_eq!(r3.status(), 200);

    // Get keyword tree
    let resp = app.get("/api/v1/keywords/tree").await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();

    let level_1 = body["level_1"].as_array().expect("level_1 should be array");
    let level_2 = body["level_2"].as_array().expect("level_2 should be array");
    let level_3 = body["level_3"].as_array().expect("level_3 should be array");

    assert!(
        !level_1.is_empty(),
        "level_1 should have at least one keyword"
    );
    assert!(
        !level_2.is_empty(),
        "level_2 should have at least one keyword"
    );
    assert!(
        !level_3.is_empty(),
        "level_3 should have at least one keyword"
    );

    // Verify all level_1 keywords have level=1
    assert!(
        level_1.iter().all(|k| k["level"] == 1),
        "All level_1 keywords should have level=1"
    );
    assert!(
        level_2.iter().all(|k| k["level"] == 2),
        "All level_2 keywords should have level=2"
    );
    assert!(
        level_3.iter().all(|k| k["level"] == 3),
        "All level_3 keywords should have level=3"
    );
}
