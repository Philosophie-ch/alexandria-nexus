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

#[tokio::test]
async fn test_validate_resolves_author_via_name_variant() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed an author with a name variant
    let payload = json!({
        "author_key": format!("aristotle-{s}"),
        "mononym_latex": "Aristotle",
        "mononym_unicode": "Aristotle",
        "name_variants_latex": ["Aristote", "Aristoteles"]
    });
    let create = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(create.status(), 200, "Failed to seed author");

    // CSV uses a variant name — should resolve, not report as missing
    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,variant{s}:2024,A Book,Aristote,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["missing_authors"].as_array().unwrap().len(),
        0,
        "Author should resolve via name_variant 'Aristote'"
    );
}

#[tokio::test]
async fn test_import_resolves_author_via_name_variant() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Seed an author with a name variant
    let payload = json!({
        "author_key": format!("aristotle-{s}"),
        "mononym_latex": "Aristotle",
        "mononym_unicode": "Aristotle",
        "name_variants_latex": ["Aristote", "Aristoteles"]
    });
    let create = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(create.status(), 200, "Failed to seed author");

    // Import using variant name
    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,variant{s}:2024,A Book,Aristoteles,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1, "Should import via name variant");
    assert_eq!(body["failed"], 0);
}

#[tokio::test]
async fn test_variant_collision_reported_as_ambiguous() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Two authors — one has "Stageirites" as a variant, the other has it as mononym
    let payload1 = json!({
        "author_key": format!("aristotle-{s}"),
        "mononym_latex": "Aristotle",
        "mononym_unicode": "Aristotle",
        "name_variants_latex": ["Stageirites"]
    });
    app.post_json("/api/v1/authors", &payload1).await;

    let payload2 = json!({
        "author_key": format!("stageirites-{s}"),
        "mononym_latex": "Stageirites",
        "mononym_unicode": "Stageirites",
    });
    app.post_json("/api/v1/authors", &payload2).await;

    // CSV uses "Stageirites" — should be ambiguous (matches variant of author1 + mononym of author2)
    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,ambig{s}:2024,A Book,Stageirites,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let ambiguous = body["ambiguous_authors"].as_array().unwrap();
    assert_eq!(ambiguous.len(), 1, "Stageirites should be ambiguous");
    assert_eq!(ambiguous[0]["matching_ids"].as_array().unwrap().len(), 2);
}

// ============================================================================
// IMPORT ENTITIES
// ============================================================================

#[tokio::test]
async fn test_import_entities_creates_missing() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // This endpoint creates institutions, schools, series, keywords.
    // Authors, journals, publishers must be imported via their own endpoints.
    let csv = format!(
        "entry_type,bibkey,title,institution,school,series,_kw-level1,date\n\
         book,test{s}:2024,A Book,MIT-{s},ETH-{s},LNM-{s},kw-{s},2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["created_institutions"], 1);
    assert_eq!(body["created_schools"], 1);
    assert_eq!(body["created_series"], 1);
    assert_eq!(body["created_keywords"], 1);
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

    // First import creates the keyword
    let csv = format!(
        "entry_type,bibkey,title,_kw-level1,date\n\
         book,test{s}:2024,A Book,existing-kw-{s},2024"
    );
    let r: serde_json::Value =
        upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv)
            .await
            .json()
            .await
            .unwrap();
    assert_eq!(r["created_keywords"], 1);

    // Second import with same keyword must not create a duplicate
    let resp = upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["created_keywords"], 0,
        "Should not create duplicate keyword"
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

    // Record the ID after first import
    let bib_resp1 = app
        .get(&format!("/api/v1/bibitems/by-key/update{s}:2024"))
        .await;
    assert_eq!(bib_resp1.status(), 200);
    let bib1: serde_json::Value = bib_resp1.json().await.unwrap();
    let id_after_first_import = bib1["id"].as_i64().unwrap();

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

    // Verify title was updated and ID is preserved
    let bib_resp2 = app
        .get(&format!("/api/v1/bibitems/by-key/update{s}:2024"))
        .await;
    assert_eq!(bib_resp2.status(), 200);
    let bib2: serde_json::Value = bib_resp2.json().await.unwrap();
    assert_eq!(bib2["title_latex"], "Updated Title");
    assert_eq!(
        bib2["id"].as_i64().unwrap(),
        id_after_first_import,
        "ID must remain stable across import updates"
    );
}

#[tokio::test]
async fn test_import_preserves_id_and_timestamps() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // First import
    let csv1 = format!(
        "entry_type,bibkey,title,date\n\
         book,ts{s}:2024,Original Title,2024"
    );
    let resp1 = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv1).await;
    assert_eq!(resp1.status(), 200);

    // Record id and created_at
    let bib_resp1 = app
        .get(&format!("/api/v1/bibitems/by-key/ts{s}:2024"))
        .await;
    assert_eq!(bib_resp1.status(), 200);
    let bib1: serde_json::Value = bib_resp1.json().await.unwrap();
    let original_id = bib1["id"].as_i64().unwrap();
    let original_created_at = bib1["created_at"].as_str().unwrap().to_string();
    let original_updated_at = bib1["updated_at"].as_str().unwrap().to_string();

    // Brief pause so updated_at will differ
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Re-import with changed title
    let csv2 = format!(
        "entry_type,bibkey,title,date\n\
         book,ts{s}:2024,Changed Title,2024"
    );
    let resp2 = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv2).await;
    assert_eq!(resp2.status(), 200);
    let body: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body["updated"], 1);

    let bib_resp2 = app
        .get(&format!("/api/v1/bibitems/by-key/ts{s}:2024"))
        .await;
    assert_eq!(bib_resp2.status(), 200);
    let bib2: serde_json::Value = bib_resp2.json().await.unwrap();

    assert_eq!(
        bib2["id"].as_i64().unwrap(),
        original_id,
        "id must be preserved"
    );
    assert_eq!(
        bib2["created_at"].as_str().unwrap(),
        original_created_at,
        "created_at must be preserved"
    );
    assert_ne!(
        bib2["updated_at"].as_str().unwrap(),
        original_updated_at,
        "updated_at must change on update"
    );
    assert_eq!(bib2["title_latex"], "Changed Title");
}

#[tokio::test]
async fn test_import_full_csv_skips_row_with_missing_journal() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title,journal,date\n\
         article,test{s}:2024,A Paper,Nonexistent-{s},2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 0);
    assert_eq!(body["skipped"], 1);

    // Verify the bibkey was not inserted
    let get = app
        .get(&format!("/api/v1/bibitems/by-key/test{s}:2024"))
        .await;
    assert_eq!(get.status(), 404);
}

#[tokio::test]
async fn test_import_full_csv_skips_row_with_ambiguous_author() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Two authors with the same name → ambiguous, cannot resolve
    seed_author(&app, &format!("dup1-{s}"), "Duplicate", "Name").await;
    seed_author(&app, &format!("dup2-{s}"), "Duplicate", "Name").await;

    let csv = format!(
        "entry_type,bibkey,title,author,date\n\
         book,dup{s}:2024,A Book,\"Duplicate, Name\",2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 0);
    assert_eq!(body["skipped"], 1);

    // Verify the bibkey was not inserted
    let get = app
        .get(&format!("/api/v1/bibitems/by-key/dup{s}:2024"))
        .await;
    assert_eq!(get.status(), 404);
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

    // Authors and journals must be seeded via their own endpoints.
    // import-entities-from-full-csv only creates institutions/schools/series/keywords.
    seed_author(&app, &format!("kant-{s}"), "Kant", "Immanuel").await;
    seed_journal(&app, &format!("mind-{s}"), "Mind").await;

    let csv = format!(
        "entry_type,bibkey,title,author,journal,date,_kw-level1\n\
         article,pipe{s}:2024,Pipeline Paper,\"Kant, Immanuel\",Mind,2024,pipe-kw-{s}"
    );

    // Step 1: Validate — author and journal are seeded, only keyword is missing
    let v_resp = upload_csv(&app, "/api/v1/admin/validate-full-csv", &csv).await;
    assert_eq!(v_resp.status(), 200);
    let v_body: serde_json::Value = v_resp.json().await.unwrap();
    assert_eq!(v_body["valid_rows"], 1);
    assert_eq!(v_body["missing_authors"].as_array().unwrap().len(), 0);
    assert_eq!(v_body["missing_journals"].as_array().unwrap().len(), 0);

    // Step 2: Import entities — creates the keyword
    let e_resp = upload_csv(&app, "/api/v1/admin/import-entities-from-full-csv", &csv).await;
    assert_eq!(e_resp.status(), 200);
    let e_body: serde_json::Value = e_resp.json().await.unwrap();
    assert_eq!(e_body["created_keywords"], 1);
    assert_eq!(e_body["errors"].as_array().unwrap().len(), 0);

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
    assert!(
        bib["journal_key"].is_string(),
        "Should have journal_key set"
    );
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
// BIBITEM REFS EXTRACTION FROM TEXT FIELDS
// ============================================================================

#[tokio::test]
async fn test_import_populates_further_refs_from_note() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let bibkey_target = format!("ref-tgt-{s}:2000");
    let bibkey_source = format!("ref-src-{s}:2024");

    // Source cites target via \citet in the note field.
    let csv = format!(
        "entry_type,bibkey,title,note,date\n\
         book,{bibkey_target},Target Book,,2000\n\
         book,{bibkey_source},Source Book,\\citet{{{bibkey_target}}},2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 2, "Both bibitems should import");
    assert_eq!(body["failed"], 0);

    // Verify: render source with include_further_refs — target must appear in further_refs_html.
    let render_resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [bibkey_source], "include_further_refs": true }),
        )
        .await;
    assert_eq!(render_resp.status(), 200);

    let render_body: serde_json::Value = render_resp.json().await.unwrap();
    let further = render_body["further_refs_html"].as_str();
    assert!(
        further.is_some(),
        "further_refs_html should be present when source cites target in note"
    );
    assert!(
        further
            .unwrap()
            .contains(&format!("data-bibkey=\"{bibkey_target}\"")),
        "further_refs_html should contain the cited bibitem"
    );
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

    // Record ID before re-import
    let bib_before = app
        .get(&format!("/api/v1/bibitems/by-key/round{s}:2024"))
        .await;
    assert_eq!(bib_before.status(), 200);
    let bib_before: serde_json::Value = bib_before.json().await.unwrap();
    let id_before = bib_before["id"].as_i64().unwrap();

    // Re-import the exported CSV (should update, not create new)
    let reimport_resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &exported_csv).await;
    assert_eq!(reimport_resp.status(), 200);
    let body: serde_json::Value = reimport_resp.json().await.unwrap();
    assert_eq!(body["failed"], 0, "Round-trip re-import should not fail");
    assert_eq!(body["updated"], 1, "Should update the existing bibitem");
    assert_eq!(body["imported"], 0, "Should not create new bibitems");

    // Verify ID stability across round-trip
    let bib_after = app
        .get(&format!("/api/v1/bibitems/by-key/round{s}:2024"))
        .await;
    assert_eq!(bib_after.status(), 200);
    let bib_after: serde_json::Value = bib_after.json().await.unwrap();
    assert_eq!(
        bib_after["id"].as_i64().unwrap(),
        id_before,
        "Round-trip re-import must preserve bibitem ID"
    );
}

// ============================================================================
// CROSSREF DEFERRED CONSTRAINTS
// ============================================================================

#[tokio::test]
async fn test_import_crossref_within_same_batch() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    seed_author(&app, &format!("auth-{s}"), "Xref", "Author").await;

    // The chapter crossrefs the collection; the chapter row appears FIRST in the CSV,
    // so its crossref target does not exist yet when the INSERT is executed.
    // Without deferred FK constraints this would fail.
    let csv = format!(
        "entry_type,bibkey,title,author,crossref,date\n\
         incollection,chapter{s}:2024,A Chapter,\"Xref, Author\",coll{s}:2024,2024\n\
         collection,coll{s}:2024,The Collection,\"Xref, Author\",,2024"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 2);
    assert_eq!(body["failed"], 0);
    assert_eq!(body["nulled_crossrefs"].as_array().unwrap().len(), 0);

    // Verify the crossref resolved correctly
    let chapter = app
        .get(&format!("/api/v1/bibitems/by-key/chapter{s}:2024"))
        .await;
    assert_eq!(chapter.status(), 200);
    let chapter: serde_json::Value = chapter.json().await.unwrap();
    assert_eq!(chapter["crossref"], format!("coll{s}:2024"));
}

#[tokio::test]
async fn test_import_crossref_update_to_new_bibitem() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    seed_author(&app, &format!("auth-{s}"), "Xref", "Author").await;

    // First import: a standalone book with no crossref
    let csv1 = format!(
        "entry_type,bibkey,title,author,date\n\
         book,existing{s}:2024,Old Book,\"Xref, Author\",2024"
    );
    let resp1 = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv1).await;
    assert_eq!(resp1.status(), 200);

    // Second import: the existing book now crossrefs a new collection.
    // The UPDATE of the existing book sets crossref to a bibkey that
    // only gets INSERTed later in the same batch.
    let csv2 = format!(
        "entry_type,bibkey,title,author,crossref,date\n\
         incollection,existing{s}:2024,Old Book,\"Xref, Author\",newcoll{s}:2024,2024\n\
         collection,newcoll{s}:2024,New Collection,\"Xref, Author\",,2024"
    );
    let resp2 = upload_csv(&app, "/api/v1/admin/import-full-csv", &csv2).await;
    assert_eq!(resp2.status(), 200);

    let body: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body["updated"], 1);
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);
    assert_eq!(body["nulled_crossrefs"].as_array().unwrap().len(), 0);

    let existing = app
        .get(&format!("/api/v1/bibitems/by-key/existing{s}:2024"))
        .await;
    assert_eq!(existing.status(), 200);
    let existing: serde_json::Value = existing.json().await.unwrap();
    assert_eq!(existing["crossref"], format!("newcoll{s}:2024"));
}
