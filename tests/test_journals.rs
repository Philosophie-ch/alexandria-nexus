//! Journal CRUD integration tests (representative of simpler entities).

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

#[tokio::test]
async fn test_journal_crud() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();
    let key = format!("test-journal-{}", suffix);

    // === CREATE ===
    let create_payload = json!({
        "journal_key": &key,
        "name_latex": "Journal of Philosophy",
        "name_unicode": "Journal of Philosophy",
        "name_simplified": "journal of philosophy"
    });

    let create_resp = app.post_json("/api/v1/journals", &create_payload).await;
    assert_eq!(
        create_resp.status(),
        200,
        "Create journal should return 200"
    );

    let created: serde_json::Value = create_resp.json().await.unwrap();
    let id = created["id"].as_i64().expect("Response should contain id");
    assert_eq!(created["journal_key"], key);
    assert_eq!(created["name_latex"], "Journal of Philosophy");

    // === READ (by ID) ===
    let get_resp = app.get(&format!("/api/v1/journals/{}", id)).await;
    assert_eq!(get_resp.status(), 200);

    let fetched: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(fetched["id"], id);
    assert_eq!(fetched["journal_key"], key);

    // === UPDATE ===
    let update_payload = json!({
        "issn_print": "0022-362X"
    });
    let update_resp = app
        .put_json(&format!("/api/v1/journals/{}", id), &update_payload)
        .await;
    assert_eq!(update_resp.status(), 200);

    let updated: serde_json::Value = update_resp.json().await.unwrap();
    assert_eq!(updated["issn_print"], "0022-362X");
    // Original fields preserved
    assert_eq!(updated["name_latex"], "Journal of Philosophy");

    // === DELETE ===
    let delete_resp = app.delete(&format!("/api/v1/journals/{}", id)).await;
    assert_eq!(delete_resp.status(), 204);

    // Verify deleted
    let verify_resp = app.get(&format!("/api/v1/journals/{}", id)).await;
    assert_eq!(verify_resp.status(), 404);
}
