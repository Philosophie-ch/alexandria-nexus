//! Integration tests for the HTML bibliography render endpoint.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

/// Helper: upload a CSV file to an admin endpoint.
async fn upload_csv(app: &TestApp, path: &str, csv: &str) -> reqwest::Response {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(csv.as_bytes().to_vec())
            .file_name("data.csv")
            .mime_str("text/csv")
            .unwrap(),
    );
    app.client
        .post(app.url(path))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .multipart(form)
        .send()
        .await
        .expect("Failed to send multipart request")
}

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
async fn link_author(app: &TestApp, bibitem_id: i64, author_key: &str, role: &str, position: i16) {
    let payload = json!({
        "author_key": author_key,
        "role": role,
        "position": position
    });
    let resp = app
        .post_json(&format!("/api/v1/bibitems/{bibitem_id}/authors"), &payload)
        .await;
    assert!(
        resp.status().is_success(),
        "Failed to link author {author_key} to bibitem {bibitem_id}: {}",
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
    let (_, author_key) = create_author(&app, &suffix, "render-author", "Jane", "Smith").await;
    let bibitem_id =
        create_bibitem_with_details(&app, &suffix, "smith", "article", "Some Title", Some(2024))
            .await;

    // Link author
    link_author(&app, bibitem_id, &author_key, "author", 1).await;

    // Render by bibkey
    let bibkey = format!("smith-{suffix}");
    let resp = app
        .post_json("/api/v1/render", &json!({ "bibkeys": [bibkey] }))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let html = body["main_html"]
        .as_str()
        .expect("main_html should be a string");
    assert!(html.contains("data-type=\"article\""), "entry type present");
    assert!(
        html.contains(&format!("data-bibkey=\"smith-{suffix}\"")),
        "bibkey present"
    );
    assert!(
        html.contains("class=\"smallcaps\">Smith</span>"),
        "author in smallcaps"
    );
    assert!(
        html.contains("data-field=\"date\">2024</span>"),
        "year present"
    );
    assert!(html.contains("Some Title"), "title present");
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
// Test: include_further_refs=true returns further_refs_html
// =============================================================================

#[tokio::test]
async fn test_render_include_further_refs_populated() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Create two bibitems: A will reference B.
    create_bibitem_with_details(&app, &s, "fr-b", "book", "Further Ref Target", Some(2000)).await;
    create_bibitem_with_details(&app, &s, "fr-a", "book", "Further Ref Source", Some(2024)).await;

    // Insert a further_ref row from A to B.
    let key_a = format!("fr-a-{s}");
    let key_b = format!("fr-b-{s}");
    let refs_csv = format!("source_key,target_key,ref_type\n{key_a},{key_b},further_ref");
    let refs_resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &refs_csv).await;
    assert_eq!(refs_resp.status(), 200);

    // Rebuild the transitive closure.
    let rc_resp = app
        .client
        .post(app.url("/api/v1/admin/recompute-deps"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(rc_resp.status(), 200);
    let rc: serde_json::Value = rc_resp.json().await.unwrap();
    assert_eq!(rc["further_refs"], 1);

    // Render A with include_further_refs: true.
    let bibkey_a = format!("fr-a-{s}");
    let bibkey_b = format!("fr-b-{s}");
    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [bibkey_a], "include_further_refs": true }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["main_html"].as_str().is_some(),
        "main_html should be present"
    );

    let further = body["further_refs_html"].as_str();
    assert!(
        further.is_some(),
        "further_refs_html should be present when include_further_refs=true and refs exist"
    );
    assert!(
        further
            .unwrap()
            .contains(&format!("data-bibkey=\"{bibkey_b}\"")),
        "further_refs_html should contain the referenced bibitem"
    );
}

#[tokio::test]
async fn test_render_include_further_refs_no_refs() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Create a bibitem with no references.
    create_bibitem_with_details(&app, &s, "lonely", "book", "Lonely Book", Some(2024)).await;
    let bibkey = format!("lonely-{s}");

    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [bibkey], "include_further_refs": true }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["main_html"].as_str().is_some());
    assert!(
        body["further_refs_html"].is_null(),
        "further_refs_html should be null when bibitem has no further refs"
    );
}

#[tokio::test]
async fn test_render_include_further_refs_flag_omitted() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Set up A→B ref (same as test_render_include_further_refs_populated).
    let id_b =
        create_bibitem_with_details(&app, &s, "omit-b", "book", "Omit Target", Some(2000)).await;
    let id_a =
        create_bibitem_with_details(&app, &s, "omit-a", "book", "Omit Source", Some(2024)).await;

    let refs_csv = format!("source_id,target_id,ref_type\n{id_a},{id_b},further_ref");
    upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &refs_csv).await;
    app.client
        .post(app.url("/api/v1/admin/recompute-deps"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();

    // Render WITHOUT include_further_refs — further_refs_html must be null.
    let bibkey_a = format!("omit-a-{s}");
    let resp = app
        .post_json("/api/v1/render", &json!({ "bibkeys": [bibkey_a] }))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["main_html"].as_str().is_some());
    assert!(
        body["further_refs_html"].is_null(),
        "further_refs_html should be null when include_further_refs is omitted"
    );
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
