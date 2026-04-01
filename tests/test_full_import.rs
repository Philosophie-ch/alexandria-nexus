//! Integration tests for the full CSV import pipeline.
//!
//! Tests the three endpoints:
//! - POST /api/v1/admin/validate-full-csv
//! - POST /api/v1/admin/import-entities-from-full-csv
//! - POST /api/v1/admin/import-full-csv

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

/// Helper: upload a CSV to one of the full-import endpoints.
async fn upload_csv(app: &TestApp, path: &str, csv_content: &str) -> reqwest::Response {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(csv_content.as_bytes().to_vec())
            .file_name("test.csv")
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

/// Helper: seed an author via the CRUD API and return its ID.
async fn seed_author(app: &TestApp, key: &str, family: &str, given: &str) -> i64 {
    let payload = json!({
        "author_key": key,
        "family_name_latex": family,
        "family_name_unicode": family,
        "family_name_simplified": family.to_lowercase(),
        "given_name_latex": given,
        "given_name_unicode": given,
        "given_name_simplified": given.to_lowercase(),
    });
    let resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to seed author {key}");
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"].as_i64().unwrap()
}

/// Helper: seed a journal via the CRUD API and return its ID.
async fn seed_journal(app: &TestApp, key: &str, name: &str) -> i64 {
    let payload = json!({
        "journal_key": key,
        "name_latex": name,
        "name_unicode": name,
        "name_simplified": name.to_lowercase(),
    });
    let resp = app.post_json("/api/v1/journals", &payload).await;
    assert_eq!(resp.status(), 200, "Failed to seed journal {key}");
    let body: serde_json::Value = resp.json().await.unwrap();
    body["id"].as_i64().unwrap()
}

// ============================================================================
// VALIDATE
// ============================================================================

#[tokio::test]
async fn test_validate_clean_csv() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed entities that the CSV will reference
    seed_author(&app, &format!("kant-{s}"), "Kant", "Immanuel").await;
    seed_journal(&app, &format!("mind-{s}"), &format!("Mind-{s}")).await;

    let csv = format!(
        "entry_type,bibkey,title,author,journal,date\n\
         article,kant{s}:1781,Critique of Pure Reason,\"Kant, Immanuel\",Mind-{s},1781"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["total_rows"], 1);
    assert_eq!(body["valid_rows"], 1);
    assert_eq!(body["errors"].as_array().unwrap().len(), 0);
    assert_eq!(body["missing_authors"].as_array().unwrap().len(), 0);
    assert_eq!(body["missing_journals"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_validate_reports_missing_author() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,nobody{s}:2024,A Book,\"Nobody, Person\",2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let missing = body["missing_authors"].as_array().unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0], "Nobody, Person");
}

#[tokio::test]
async fn test_validate_reports_missing_journal() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,journal,date\n\
         article,test{s}:2024,A Paper,Nonexistent Review,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let missing = body["missing_journals"].as_array().unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0], "Nonexistent Review");
}

#[tokio::test]
async fn test_validate_reports_parse_errors() {
    let app = TestApp::spawn().await;

    let csv = "entry_type,bibkey,title,date\n\
               book,good:2024,Good Book,2024\n\
               book,bad:2024,Bad Book,not-a-date";

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["total_rows"], 2);
    assert_eq!(body["valid_rows"], 1);
    assert_eq!(body["errors"].as_array().unwrap().len(), 1);
    assert_eq!(body["errors"][0]["bibkey"], "bad:2024");
}

#[tokio::test]
async fn test_validate_reports_stale_bibitems() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed a bibitem that won't be in the CSV
    let payload = json!({
        "bibkey": format!("stale{s}:2024"),
        "entry_type": "book",
        "title_latex": "Old Book",
        "title_unicode": "Old Book",
        "title_simplified": "old book",
    });
    app.post_json("/api/v1/bibitems", &payload).await;

    // CSV with a different bibitem
    let csv = format!(
        "entry_type,bibkey,title,date\n\
         book,new{s}:2024,New Book,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let stale = body["stale_bibitems"].as_array().unwrap();
    assert!(
        stale.iter().any(|b| b == &format!("stale{s}:2024")),
        "stale{s}:2024 should be in stale list, got: {stale:?}"
    );
}

#[tokio::test]
async fn test_validate_reports_ambiguous_author() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Create two authors with the same latex name but different keys
    seed_author(&app, &format!("smith1-{s}"), "Smith", "John").await;
    seed_author(&app, &format!("smith2-{s}"), "Smith", "John").await;

    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,smith{s}:2024,A Book,\"Smith, John\",2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let ambiguous = body["ambiguous_authors"].as_array().unwrap();
    assert_eq!(ambiguous.len(), 1);
    assert_eq!(ambiguous[0]["name"], "Smith, John");
    assert_eq!(ambiguous[0]["matching_ids"].as_array().unwrap().len(), 2);
}

// ============================================================================
// IMPORT ENTITIES
// ============================================================================

#[tokio::test]
async fn test_import_entities_creates_missing() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,author,journal,publisher,date\n\
         article,test{s}:2024,A Paper,\"NewAuthor-{s}, Given\",NewJournal-{s},NewPublisher-{s},2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["created_authors"], 1);
    assert_eq!(body["created_journals"], 1);
    assert_eq!(body["created_publishers"], 1);
    assert_eq!(body["errors"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_import_entities_creates_keywords() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,_kw-level1,_kw-level2,date\n\
         book,test{s}:2024,A Book,epistemology-{s},logic-{s},2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["created_keywords"], 2);
}

#[tokio::test]
async fn test_import_entities_skips_existing() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed an author first
    seed_author(&app, &format!("existing-{s}"), "Existing", "Author").await;

    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,test{s}:2024,A Book,\"Existing, Author\",2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["created_authors"], 0,
        "Should not create duplicate author"
    );
}

// ============================================================================
// IMPORT BIBITEMS
// ============================================================================

#[tokio::test]
async fn test_import_full_csv_success() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed entities
    seed_author(&app, &format!("kant-{s}"), "Kant", "Immanuel").await;

    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,kant{s}:1781,Critique of Pure Reason,\"Kant, Immanuel\",1781"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["updated"], 0);
    assert_eq!(body["failed"], 0);

    // Verify bibitem exists
    let bib_resp = app
        .get(&format!("/api/v1/bibitems/by-key/kant{s}:1781"))
        .await;
    assert_eq!(bib_resp.status(), 200);
    let bib: serde_json::Value = bib_resp.json().await.unwrap();
    assert_eq!(bib["title_latex"], "Critique of Pure Reason");
    assert_eq!(bib["date_year"], 1781);
}

#[tokio::test]
async fn test_import_full_csv_deletes_stale() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed a bibitem that will become stale
    let payload = json!({
        "bibkey": format!("stale{s}:2024"),
        "entry_type": "book",
        "title_latex": "Will Be Deleted",
        "title_unicode": "Will Be Deleted",
        "title_simplified": "will be deleted",
    });
    app.post_json("/api/v1/bibitems", &payload).await;

    // Import a CSV without the stale bibitem
    let csv = format!(
        "entry_type,bibkey,title,date\n\
         book,new{s}:2024,Replacement Book,2024"
    );

    // Without delete_stale: stale bibitem should survive
    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["deleted"], 0, "Should not delete without delete_stale");
    assert_eq!(body["imported"], 1);

    // With delete_stale=true: stale bibitem should be deleted
    let resp2 = upload_csv(
        &app,
        "/api/v1/admin/import-full-csv?delete_stale=true",
        &csv,
    )
    .await;
    assert_eq!(resp2.status(), 200);
    let body2: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body2["deleted"], 1, "Should delete 1 stale bibitem");

    // Verify stale bibitem is gone
    let stale_resp = app
        .get(&format!("/api/v1/bibitems/by-key/stale{s}:2024"))
        .await;
    assert_eq!(stale_resp.status(), 404);
}

#[tokio::test]
async fn test_import_full_csv_updates_existing() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // First import
    let csv1 = format!(
        "entry_type,bibkey,title,date\n\
         book,update{s}:2024,Original Title,2024"
    );
    let resp1 = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv1).await;
    assert_eq!(resp1.status(), 200);

    // Second import with updated title
    let csv2 = format!(
        "entry_type,bibkey,title,date\n\
         book,update{s}:2024,Updated Title,2024"
    );
    let resp2 = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv2).await;
    assert_eq!(resp2.status(), 200);

    let body: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body["updated"], 1);
    assert_eq!(body["imported"], 0);

    // Verify title was updated
    let bib_resp = app
        .get(&format!("/api/v1/bibitems/by-key/update{s}:2024"))
        .await;
    assert_eq!(bib_resp.status(), 200);
    let bib: serde_json::Value = bib_resp.json().await.unwrap();
    assert_eq!(bib["title_latex"], "Updated Title");
}

#[tokio::test]
async fn test_import_full_csv_fails_on_missing_entities() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,journal,date\n\
         article,test{s}:2024,A Paper,Nonexistent-{s},2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 422);

    let body: serde_json::Value = resp.json().await.unwrap();
    let missing = body["missing_journals"].as_array().unwrap();
    assert_eq!(missing.len(), 1);
}

#[tokio::test]
async fn test_import_full_csv_fails_on_ambiguous_author() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Two authors with same name
    seed_author(&app, &format!("dup1-{s}"), "Duplicate", "Name").await;
    seed_author(&app, &format!("dup2-{s}"), "Duplicate", "Name").await;

    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,dup{s}:2024,A Book,\"Duplicate, Name\",2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 422);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["ambiguous_authors"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_import_full_csv_with_author_junctions() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    seed_author(&app, &format!("auth-{s}"), "Author", "One").await;
    seed_author(&app, &format!("edit-{s}"), "Editor", "Two").await;

    let csv = format!(
        "entry_type,bibkey,title,author,editor,date\n\
         incollection,junc{s}:2024,A Chapter,\"Author, One\",\"Editor, Two\",2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);

    // Verify junctions via by-key lookup then /authors
    let bib_resp = app
        .get(&format!("/api/v1/bibitems/by-key/junc{s}:2024"))
        .await;
    let bib: serde_json::Value = bib_resp.json().await.unwrap();
    let bib_id = bib["id"].as_i64().unwrap();

    let authors_resp = app.get(&format!("/api/v1/bibitems/{bib_id}/authors")).await;
    let authors: Vec<serde_json::Value> = authors_resp.json().await.unwrap();
    assert_eq!(authors.len(), 2, "Should have author + editor junctions");

    let has_author = authors.iter().any(|a| a["role"] == "author");
    let has_editor = authors.iter().any(|a| a["role"] == "editor");
    assert!(has_author, "Should have an author-role junction");
    assert!(has_editor, "Should have an editor-role junction");
}

// ============================================================================
// FULL PIPELINE (validate → import-entities → import-bibitems)
// ============================================================================

#[tokio::test]
async fn test_full_pipeline() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,author,journal,date,_kw-level1\n\
         article,pipe{s}:2024,Pipeline Paper,\"NewPipe-{s}, Author\",PipeJournal-{s},2024,pipe-kw-{s}"
    );

    // Step 1: Validate — should report missing entities
    let v_resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(v_resp.status(), 200);
    let v_body: serde_json::Value = v_resp.json().await.unwrap();
    assert_eq!(v_body["valid_rows"], 1);
    assert_eq!(v_body["missing_authors"].as_array().unwrap().len(), 1);
    assert_eq!(v_body["missing_journals"].as_array().unwrap().len(), 1);

    // Step 2: Import entities — creates the missing ones
    let e_resp = upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv).await;
    assert_eq!(e_resp.status(), 200);
    let e_body: serde_json::Value = e_resp.json().await.unwrap();
    assert_eq!(e_body["created_authors"], 1);
    assert_eq!(e_body["created_journals"], 1);
    assert_eq!(e_body["created_keywords"], 1);

    // Step 3: Import bibitems — should now succeed
    let i_resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(i_resp.status(), 200);
    let i_body: serde_json::Value = i_resp.json().await.unwrap();
    assert_eq!(i_body["imported"], 1);
    assert_eq!(i_body["failed"], 0);

    // Verify the bibitem exists with correct data
    let bib_resp = app
        .get(&format!("/api/v1/bibitems/by-key/pipe{s}:2024"))
        .await;
    assert_eq!(bib_resp.status(), 200);
    let bib: serde_json::Value = bib_resp.json().await.unwrap();
    assert_eq!(bib["title_latex"], "Pipeline Paper");
    assert_eq!(bib["date_year"], 2024);
    assert!(bib["journal_id"].is_number(), "Should have journal_id set");
}

// ============================================================================
// DUPLICATE BIBKEYS
// ============================================================================

#[tokio::test]
async fn test_validate_reports_duplicate_bibkeys() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,date\n\
         book,dup{s}:2024,First Copy,2024\n\
         book,dup{s}:2024,Second Copy,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let dups = body["duplicate_bibkeys"].as_array().unwrap();
    assert_eq!(dups.len(), 1);
    assert_eq!(dups[0]["bibkey"], format!("dup{s}:2024"));
    assert_eq!(dups[0]["rows"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_import_rejects_duplicate_bibkeys() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,date\n\
         book,dup{s}:2024,First,2024\n\
         book,dup{s}:2024,Second,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 422);

    let body: serde_json::Value = resp.json().await.unwrap();
    let dups = body["duplicate_bibkeys"].as_array().unwrap();
    assert_eq!(dups.len(), 1, "Should report 1 duplicate bibkey");
}

// ============================================================================
// EXPORT
// ============================================================================

#[tokio::test]
async fn test_export_full_csv() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed entities
    seed_author(&app, &format!("kant-{s}"), "Kant", "Immanuel").await;
    seed_journal(&app, &format!("mind-{s}"), &format!("Mind-{s}")).await;

    // Import a bibitem
    let csv = format!(
        "entry_type,bibkey,title,author,journal,date\n\
         article,kant{s}:1781,Critique of Pure Reason,\"Kant, Immanuel\",Mind-{s},1781"
    );
    let import_resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(import_resp.status(), 200);

    // Export
    let export_resp = app
        .client
        .post(app.url("/api/v1/admin/export-full-csv"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(export_resp.status(), 200);

    let exported_csv = export_resp.text().await.unwrap();

    // Verify headers
    assert!(exported_csv.starts_with("entry_type,bibkey,"));

    // Verify our bibitem is in the export
    assert!(
        exported_csv.contains(&format!("kant{s}:1781")),
        "Export should contain our bibitem bibkey"
    );
    assert!(
        exported_csv.contains("Critique of Pure Reason"),
        "Export should contain title"
    );
    assert!(
        exported_csv.contains("Kant, Immanuel"),
        "Export should contain author name"
    );
    assert!(
        exported_csv.contains(&format!("Mind-{s}")),
        "Export should contain journal name"
    );
}

#[tokio::test]
async fn test_export_import_round_trip() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed entities
    seed_author(&app, &format!("round-{s}"), "Roundtrip", "Author").await;

    // Import
    let csv = format!(
        "entry_type,bibkey,title,author,date,pubstate\n\
         book,round{s}:2024,Round Trip Book,\"Roundtrip, Author\",2024,published"
    );
    let import_resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(import_resp.status(), 200);

    // Export
    let export_resp = app
        .client
        .post(app.url("/api/v1/admin/export-full-csv"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    let exported_csv = export_resp.text().await.unwrap();

    // Re-import the exported CSV (should update, not create new)
    let reimport_resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &exported_csv).await;
    assert_eq!(reimport_resp.status(), 200);
    let body: serde_json::Value = reimport_resp.json().await.unwrap();
    assert_eq!(body["failed"], 0, "Round-trip re-import should not fail");
    assert_eq!(body["updated"], 1, "Should update the existing bibitem");
    assert_eq!(body["imported"], 0, "Should not create new bibitems");
}
