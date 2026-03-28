//! Export CSV integration tests.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;
use std::collections::HashMap;

/// Parse CSV body into a Vec of row HashMaps (header -> value).
fn parse_csv(body: &str) -> Vec<HashMap<String, String>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(body.as_bytes());
    reader
        .deserialize()
        .map(|r| r.expect("Failed to parse CSV row"))
        .collect()
}

/// Helper: create an author via the API and return (id, key).
async fn create_author(app: &TestApp, suffix: &str, key_prefix: &str) -> (i64, String) {
    let key = format!("{key_prefix}-{suffix}");
    let payload = json!({
        "author_key": &key,
        "family_name_latex": "TestFamily",
        "family_name_unicode": "TestFamily",
        "family_name_simplified": "testfamily",
        "given_name_latex": "TestGiven",
        "given_name_unicode": "TestGiven",
        "given_name_simplified": "testgiven"
    });
    let resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to create author {key}");
    let body: serde_json::Value = resp.json().await.unwrap();
    (body["id"].as_i64().unwrap(), key)
}

/// Helper: create a journal via the API and return (id, key).
async fn create_journal(app: &TestApp, suffix: &str, key_prefix: &str) -> (i64, String) {
    let key = format!("{key_prefix}-{suffix}");
    let payload = json!({
        "journal_key": &key,
        "name_latex": "Test Journal",
        "name_unicode": "Test Journal",
        "name_simplified": "test journal"
    });
    let resp = app.post_json("/api/v1/journals", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to create journal {key}");
    let body: serde_json::Value = resp.json().await.unwrap();
    (body["id"].as_i64().unwrap(), key)
}

/// Helper: create a keyword via the API and return id.
async fn create_keyword(app: &TestApp, name: &str, level: i16) -> i64 {
    let payload = json!({
        "name": name,
        "level": level
    });
    let resp = app.post_json("/api/v1/keywords", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to create keyword {name}");
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"].as_i64().unwrap()
}

/// Helper: create a bibitem via the API and return id.
async fn create_bibitem(app: &TestApp, suffix: &str, bibkey_prefix: &str) -> i64 {
    let payload = json!({
        "bibkey": format!("{bibkey_prefix}-{suffix}"),
        "entry_type": "article",
        "title_latex": "Test Title",
        "title_unicode": "Test Title",
        "title_simplified": "test title",
        "date_year": 2024
    });
    let resp = app.post_json("/api/v1/bibitems", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to create bibitem");
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"].as_i64().unwrap()
}

// ============================================================================
// Export authors by IDs
// ============================================================================

#[tokio::test]
async fn test_export_authors_by_ids() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let (id1, key1) = create_author(&app, &suffix, "export-a1").await;
    let (id2, _key2) = create_author(&app, &suffix, "export-a2").await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/authors",
            &json!({
                "ids": [id1, id2]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/csv"),
        "Response should be CSV; got: {content_type}"
    );

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert_eq!(rows.len(), 2, "Should export 2 author rows");

    // Verify the first author's fields are present
    let row1 = rows
        .iter()
        .find(|r| r.get("author_key") == Some(&key1))
        .unwrap();
    assert_eq!(
        row1.get("family_name_latex").map(String::as_str),
        Some("TestFamily")
    );
    assert_eq!(
        row1.get("id").map(String::as_str),
        Some(id1.to_string().as_str())
    );
}

// ============================================================================
// Export authors by keys
// ============================================================================

#[tokio::test]
async fn test_export_authors_by_keys() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let (_id1, key1) = create_author(&app, &suffix, "expkey-a1").await;
    let (_id2, key2) = create_author(&app, &suffix, "expkey-a2").await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/authors",
            &json!({
                "keys": [key1, key2]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert_eq!(rows.len(), 2, "Should export 2 authors by keys");
}

// ============================================================================
// Export with missing IDs returns 422
// ============================================================================

#[tokio::test]
async fn test_export_authors_missing_ids() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/authors",
            &json!({
                "ids": [99999, 88888]
            }),
        )
        .await;
    assert_eq!(
        resp.status(),
        422,
        "Should return 422 for missing author IDs"
    );

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "not_found");
    let missing_ids = body["missing_ids"]
        .as_array()
        .expect("Expected missing_ids");
    assert_eq!(missing_ids.len(), 2);
}

// ============================================================================
// Export with missing keys returns 422
// ============================================================================

#[tokio::test]
async fn test_export_authors_missing_keys() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/authors",
            &json!({
                "keys": ["nonexistent-key-abc", "nonexistent-key-xyz"]
            }),
        )
        .await;
    assert_eq!(resp.status(), 422);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "not_found");
    let missing_keys = body["missing_keys"]
        .as_array()
        .expect("Expected missing_keys");
    assert_eq!(missing_keys.len(), 2);
}

// ============================================================================
// Export journals
// ============================================================================

#[tokio::test]
async fn test_export_journals_by_ids() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let (id, _key) = create_journal(&app, &suffix, "expj").await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/journals",
            &json!({
                "ids": [id]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("name_simplified").map(String::as_str),
        Some("test journal")
    );
}

// ============================================================================
// Export keywords
// ============================================================================

#[tokio::test]
async fn test_export_keywords_by_ids() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let kw_name = format!("epistemology-{suffix}");
    let kw_id = create_keyword(&app, &kw_name, 1).await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/keywords",
            &json!({
                "ids": [kw_id]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("name").map(String::as_str),
        Some(kw_name.as_str())
    );
    assert_eq!(rows[0].get("level").map(String::as_str), Some("1"));
}

// ============================================================================
// Export bibitems (IDs format)
// ============================================================================

#[tokio::test]
async fn test_export_bibitems_ids_format() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create author and bibitem
    let (author_id, _) = create_author(&app, &suffix, "exp-bib-auth").await;
    let bibitem_id = create_bibitem(&app, &suffix, "test:exp-bib").await;

    // Add author to bibitem via junction endpoint
    let add_author_resp = app
        .post_json(
            &format!("/api/v1/bibitems/{bibitem_id}/authors"),
            &json!({
                "author_id": author_id,
                "role": "author",
                "position": 0
            }),
        )
        .await;
    assert_eq!(add_author_resp.status(), 200);

    // Export in IDs format
    let resp = app
        .post_json(
            "/api/v1/admin/export/bibitems",
            &json!({
                "format": "ids",
                "ids": [bibitem_id]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert_eq!(rows.len(), 1, "Should export 1 bibitem");

    let row = &rows[0];
    assert_eq!(row.get("entry_type").map(String::as_str), Some("article"));
    assert_eq!(
        row.get("bibkey").map(String::as_str),
        Some(format!("test:exp-bib-{suffix}").as_str())
    );
    assert_eq!(row.get("date_year").map(String::as_str), Some("2024"));

    // The author_ids field should contain the author ID
    let author_ids_str = row.get("author_ids").map(String::as_str).unwrap_or("");
    assert!(
        author_ids_str.contains(&author_id.to_string()),
        "author_ids should contain {author_id}; got: {author_ids_str}"
    );
}

// ============================================================================
// Export bibitems (expanded format)
// ============================================================================

#[tokio::test]
async fn test_export_bibitems_expanded_format() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create author
    let author_key = format!("exp-expand-auth-{suffix}");
    let author_payload = json!({
        "author_key": &author_key,
        "family_name_latex": "Kant",
        "family_name_unicode": "Kant",
        "family_name_simplified": "kant",
        "given_name_latex": "Immanuel",
        "given_name_unicode": "Immanuel",
        "given_name_simplified": "immanuel"
    });
    let author_resp = app.post_json("/api/v1/authors", &author_payload).await;
    assert_eq!(author_resp.status(), 200);
    let author: serde_json::Value = author_resp.json().await.unwrap();
    let author_id = author["id"].as_i64().unwrap();

    // Create bibitem
    let bibitem_id = create_bibitem(&app, &suffix, "test:exp-expand").await;

    // Add author
    app.post_json(
        &format!("/api/v1/bibitems/{bibitem_id}/authors"),
        &json!({
            "author_id": author_id,
            "role": "author",
            "position": 0
        }),
    )
    .await;

    // Export expanded (default format)
    let resp = app
        .post_json(
            "/api/v1/admin/export/bibitems",
            &json!({
                "format": "expanded",
                "ids": [bibitem_id]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert_eq!(rows.len(), 1);

    let row = &rows[0];
    assert_eq!(row.get("entry_type").map(String::as_str), Some("article"));

    // In expanded format, the "author" column should contain the resolved name,
    // not the numeric ID
    let author_col = row.get("author").map(String::as_str).unwrap_or("");
    assert!(
        !author_col.is_empty(),
        "Expanded author column should not be empty"
    );
    // The expanded format uses simplified names
    assert!(
        author_col.contains("kant")
            || author_col.contains("Kant")
            || author_col.contains("immanuel")
            || author_col.contains("Immanuel"),
        "Expanded author should contain author name; got: {author_col}"
    );
}

// ============================================================================
// Export bibitems by bibkeys
// ============================================================================

#[tokio::test]
async fn test_export_bibitems_by_bibkeys() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let bibkey = format!("test:exp-bykey-{suffix}");
    let payload = json!({
        "bibkey": &bibkey,
        "entry_type": "book",
        "title_latex": "A Book",
        "title_unicode": "A Book",
        "title_simplified": "a book"
    });
    let resp = app.post_json("/api/v1/bibitems", &payload).await;
    assert_eq!(resp.status(), 200);

    let export_resp = app
        .post_json(
            "/api/v1/admin/export/bibitems",
            &json!({
                "bibkeys": [bibkey]
            }),
        )
        .await;
    assert_eq!(export_resp.status(), 200);

    let body = export_resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert_eq!(rows.len(), 1);
    assert_eq!(row_field(&rows[0], "bibkey"), bibkey);
}

/// Convenience to get a field value from a CSV row, returning empty string if absent.
fn row_field<'a>(row: &'a HashMap<String, String>, key: &str) -> &'a str {
    row.get(key).map(String::as_str).unwrap_or("")
}

// ============================================================================
// Export bibitems with missing IDs
// ============================================================================

#[tokio::test]
async fn test_export_bibitems_missing_ids() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/bibitems",
            &json!({
                "ids": [99999]
            }),
        )
        .await;
    assert_eq!(resp.status(), 422);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "not_found");
}

// ============================================================================
// Export bibitems with missing bibkeys
// ============================================================================

#[tokio::test]
async fn test_export_bibitems_missing_bibkeys() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json(
            "/api/v1/admin/export/bibitems",
            &json!({
                "bibkeys": ["nonexistent:key"]
            }),
        )
        .await;
    assert_eq!(resp.status(), 422);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "not_found");
}

// ============================================================================
// Export requires either ids or keys param
// ============================================================================

#[tokio::test]
async fn test_export_authors_no_params() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json("/api/v1/admin/export/authors", &json!({}))
        .await;
    assert!(
        resp.status() == 400 || resp.status() == 422,
        "Should reject missing params, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_export_bibitems_no_params() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json("/api/v1/admin/export/bibitems", &json!({}))
        .await;
    assert!(
        resp.status() == 400 || resp.status() == 422,
        "Should reject missing params, got {}",
        resp.status()
    );
}

// ============================================================================
// Export all authors
// ============================================================================

#[tokio::test]
async fn test_export_all_authors() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let (_id1, key1) = create_author(&app, &suffix, "all-a1").await;
    let (_id2, key2) = create_author(&app, &suffix, "all-a2").await;

    let resp = app
        .post_json("/api/v1/admin/export/authors", &json!({ "all": true }))
        .await;
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert!(
        rows.len() >= 2,
        "Should export at least the 2 created authors; got {}",
        rows.len()
    );

    let keys: Vec<&str> = rows
        .iter()
        .filter_map(|r| r.get("author_key").map(String::as_str))
        .collect();
    assert!(keys.contains(&key1.as_str()), "Should contain {key1}");
    assert!(keys.contains(&key2.as_str()), "Should contain {key2}");
}

// ============================================================================
// Export all bibitems
// ============================================================================

#[tokio::test]
async fn test_export_all_bibitems() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let bibitem_id = create_bibitem(&app, &suffix, "test:all-bib").await;

    let resp = app
        .post_json("/api/v1/admin/export/bibitems", &json!({ "all": true }))
        .await;
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    let rows = parse_csv(&body);
    assert!(!rows.is_empty(), "Should export at least 1 bibitem");

    // The created bibitem should be present
    let bibkey = format!("test:all-bib-{suffix}");
    let found = rows
        .iter()
        .any(|r| r.get("bibkey").map(String::as_str) == Some(bibkey.as_str()));
    assert!(
        found,
        "Exported bibitems should contain {bibkey}; got {} rows, first bibkey: {:?}",
        rows.len(),
        rows.first().and_then(|r| r.get("bibkey")),
    );

    // Verify the bibitem id column is present
    let bib_row = rows
        .iter()
        .find(|r| r.get("bibkey").map(String::as_str) == Some(bibkey.as_str()))
        .unwrap();
    // Expanded format (default) does not have "id" column
    assert!(
        bib_row.contains_key("entry_type"),
        "Row should have entry_type column"
    );
    let _ = bibitem_id; // suppress unused warning
}

// ============================================================================
// Round-trip: create -> export -> verify CSV is valid and parseable
// ============================================================================

#[tokio::test]
async fn test_export_import_roundtrip_authors() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create authors via API
    let (id1, key1) = create_author(&app, &suffix, "rt-a1").await;
    let (id2, key2) = create_author(&app, &suffix, "rt-a2").await;

    // Export as CSV
    let export_resp = app
        .post_json(
            "/api/v1/admin/export/authors",
            &json!({
                "ids": [id1, id2]
            }),
        )
        .await;
    assert_eq!(export_resp.status(), 200);

    let csv_body = export_resp.text().await.unwrap();

    // Verify the CSV is well-formed and contains the expected data
    let rows = parse_csv(&csv_body);
    assert_eq!(rows.len(), 2, "Exported CSV should have 2 rows");

    let keys: Vec<&str> = rows
        .iter()
        .map(|r| r.get("author_key").unwrap().as_str())
        .collect();
    assert!(keys.contains(&key1.as_str()), "CSV should contain {key1}");
    assert!(keys.contains(&key2.as_str()), "CSV should contain {key2}");

    // Verify every row has the expected columns
    for row in &rows {
        assert!(row.contains_key("id"), "Row should have 'id' column");
        assert!(
            row.contains_key("author_key"),
            "Row should have 'author_key' column"
        );
        assert!(
            row.contains_key("family_name_latex"),
            "Row should have 'family_name_latex' column"
        );
        assert!(
            row.contains_key("given_name_latex"),
            "Row should have 'given_name_latex' column"
        );
    }
}

#[tokio::test]
async fn test_export_import_roundtrip_bibitems() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create author + bibitem + link them
    let (author_id, _) = create_author(&app, &suffix, "rt-bib-auth").await;
    let (journal_id, _) = create_journal(&app, &suffix, "rt-bib-jrnl").await;

    let bibkey = format!("test:rt-bib-{suffix}");
    let bib_payload = json!({
        "bibkey": &bibkey,
        "entry_type": "article",
        "title_latex": "Round Trip Test",
        "title_unicode": "Round Trip Test",
        "title_simplified": "round trip test",
        "journal_id": journal_id,
        "date_year": 2024
    });
    let bib_resp = app.post_json("/api/v1/bibitems", &bib_payload).await;
    assert_eq!(bib_resp.status(), 200);
    let bib: serde_json::Value = bib_resp.json().await.unwrap();
    let bibitem_id = bib["id"].as_i64().unwrap();

    // Link author
    app.post_json(
        &format!("/api/v1/bibitems/{bibitem_id}/authors"),
        &json!({
            "author_id": author_id,
            "role": "author",
            "position": 0
        }),
    )
    .await;

    // Export in IDs format
    let export_resp = app
        .post_json(
            "/api/v1/admin/export/bibitems",
            &json!({
                "format": "ids",
                "ids": [bibitem_id]
            }),
        )
        .await;
    assert_eq!(export_resp.status(), 200);

    let csv_body = export_resp.text().await.unwrap();
    let rows = parse_csv(&csv_body);
    assert_eq!(rows.len(), 1);

    let row = &rows[0];

    // Verify key fields survived the round trip
    assert_eq!(row_field(row, "bibkey"), bibkey);
    assert_eq!(row_field(row, "entry_type"), "article");
    assert_eq!(row_field(row, "title_latex"), "Round Trip Test");
    assert_eq!(row_field(row, "date_year"), "2024");
    assert_eq!(row_field(row, "journal_id"), journal_id.to_string());

    // Author IDs should be in the CSV
    let exported_author_ids = row_field(row, "author_ids");
    assert!(
        exported_author_ids.contains(&author_id.to_string()),
        "Exported author_ids should contain {author_id}; got: {exported_author_ids}"
    );
}
