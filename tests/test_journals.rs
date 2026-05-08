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

// ============================================================================
// BATCH LOOKUP BY JOURNAL KEY
// ============================================================================

#[tokio::test]
async fn test_list_journals_by_journal_keys() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let key_a = format!("test-journal-a-{}", suffix);
    let key_b = format!("test-journal-b-{}", suffix);
    let key_c = format!("test-journal-c-{}", suffix);

    for (key, name) in [(&key_a, "Mind"), (&key_b, "Nous"), (&key_c, "Synthese")] {
        let resp = app
            .post_json(
                "/api/v1/journals",
                &json!({
                    "journal_key": key,
                    "name_latex": name,
                    "name_unicode": name,
                    "name_simplified": name.to_lowercase()
                }),
            )
            .await;
        assert_eq!(resp.status(), 200, "Failed to create journal {key}");
    }

    // Fetch two of the three by journal_key
    let resp = app
        .get(&format!(
            "/api/v1/journals?journal_keys[]={}&journal_keys[]={}",
            key_a, key_c
        ))
        .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");

    let returned_keys: Vec<&str> = items
        .iter()
        .filter_map(|i| i["journal_key"].as_str())
        .collect();
    assert_eq!(
        returned_keys.len(),
        2,
        "Expected exactly 2 journals, got {returned_keys:?}"
    );
    assert!(returned_keys.contains(&key_a.as_str()), "Missing {key_a}");
    assert!(returned_keys.contains(&key_c.as_str()), "Missing {key_c}");
    assert!(
        !returned_keys.contains(&key_b.as_str()),
        "Should not include {key_b}"
    );
}
