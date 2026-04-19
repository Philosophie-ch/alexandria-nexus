//! Import CSV integration tests.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

/// Helper: upload a CSV file to an import endpoint.
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

// ============================================================================
// Author import
// ============================================================================

#[tokio::test]
async fn test_import_authors_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "author_key,family_name_latex,family_name_unicode,family_name_simplified,given_name_latex,given_name_unicode,given_name_simplified\n\
         kant-{suffix},Kant,Kant,kant,Immanuel,Immanuel,immanuel\n\
         plato-{suffix},,,,,,"
    );

    let resp = upload_csv(
        &app,
        "/api/v1/admin/import/authors?auto_assign_ids=true",
        &csv,
    )
    .await;
    assert_eq!(resp.status(), 200, "Import authors should return 200");

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1, "Should import 1 valid author (Kant)");
    // Plato row has no family/given/mononym names — fails validation.

    // Verify Kant exists
    let get_resp = app
        .get(&format!("/api/v1/authors/by-key/kant-{suffix}"))
        .await;
    assert_eq!(get_resp.status(), 200);
    let author: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(author["family_name_latex"], "Kant");
    assert_eq!(author["given_name_latex"], "Immanuel");
}

#[tokio::test]
async fn test_import_authors_with_mononym() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "author_key,mononym_latex,mononym_unicode,mononym_simplified\n\
         plato-{suffix},Plato,Plato,plato\n\
         aristotle-{suffix},Aristotle,Aristotle,aristotle"
    );

    let resp = upload_csv(
        &app,
        "/api/v1/admin/import/authors?auto_assign_ids=true",
        &csv,
    )
    .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 2, "Should import 2 mononym authors");
    assert_eq!(body["failed"], 0);

    // Verify both exist
    let plato_resp = app
        .get(&format!("/api/v1/authors/by-key/plato-{suffix}"))
        .await;
    assert_eq!(plato_resp.status(), 200);
    let plato: serde_json::Value = plato_resp.json().await.unwrap();
    assert_eq!(plato["mononym_latex"], "Plato");

    let aristotle_resp = app
        .get(&format!("/api/v1/authors/by-key/aristotle-{suffix}"))
        .await;
    assert_eq!(aristotle_resp.status(), 200);
}

#[tokio::test]
async fn test_import_authors_duplicate_key() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // First create an author via API
    let payload = json!({
        "author_key": format!("kant-{suffix}"),
        "family_name_latex": "Kant",
        "given_name_latex": "Immanuel"
    });
    let create_resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(create_resp.status(), 200);

    // Now try importing a CSV with the same key
    let csv = format!(
        "author_key,family_name_latex\n\
         kant-{suffix},Kant Duplicate"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/authors", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    // The duplicate should fail due to unique constraint
    assert_eq!(body["imported"], 0);
    assert_eq!(body["failed"], 1);
}

// ============================================================================
// Journal import
// ============================================================================

#[tokio::test]
async fn test_import_journals_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "journal_key,name_latex,name_unicode,name_simplified,issn_print,issn_electronic\n\
         mind-{suffix},Mind,Mind,mind,0026-4423,1460-2113\n\
         nous-{suffix},Nous,Nous,nous,,"
    );

    let resp = upload_csv(
        &app,
        "/api/v1/admin/import/journals?auto_assign_ids=true",
        &csv,
    )
    .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 2, "Should import 2 journals");
    assert_eq!(body["failed"], 0);

    // Verify
    let mind_resp = app
        .get(&format!("/api/v1/journals/by-key/mind-{suffix}"))
        .await;
    assert_eq!(mind_resp.status(), 200);
    let mind: serde_json::Value = mind_resp.json().await.unwrap();
    assert_eq!(mind["name_latex"], "Mind");
    assert_eq!(mind["issn_print"], "0026-4423");
}

// ============================================================================
// Publisher import
// ============================================================================

#[tokio::test]
async fn test_import_publishers_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "publisher_key,name_latex,name_unicode,name_simplified,default_address\n\
         oup-{suffix},Oxford University Press,Oxford University Press,oxford university press,Oxford"
    );

    let resp = upload_csv(
        &app,
        "/api/v1/admin/import/publishers?auto_assign_ids=true",
        &csv,
    )
    .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);

    let get_resp = app
        .get(&format!("/api/v1/publishers/by-key/oup-{suffix}"))
        .await;
    assert_eq!(get_resp.status(), 200);
    let publisher: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(publisher["default_address"], "Oxford");
}

// ============================================================================
// Institution import
// ============================================================================

#[tokio::test]
async fn test_import_institutions_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "institution_key,name_latex,name_unicode,name_simplified,default_address\n\
         mit-{suffix},MIT,MIT,mit,Cambridge"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/institutions", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);
}

// ============================================================================
// School import
// ============================================================================

#[tokio::test]
async fn test_import_schools_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "school_key,name_latex,name_unicode,name_simplified,default_address\n\
         eth-{suffix},ETH Zurich,ETH Zurich,eth zurich,Zurich"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/schools", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);
}

// ============================================================================
// Series import
// ============================================================================

#[tokio::test]
async fn test_import_series_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "series_key,name_latex,name_unicode,name_simplified\n\
         lncs-{suffix},Lecture Notes in Computer Science,Lecture Notes in Computer Science,lecture notes in computer science"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/series", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);
}

// ============================================================================
// Keyword import
// ============================================================================

#[tokio::test]
async fn test_import_keywords_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "name,level\n\
         epistemology-{suffix},1\n\
         ethics-{suffix},2\n\
         logic-{suffix},3"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/keywords", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 3, "Should import 3 keywords");
    assert_eq!(body["failed"], 0);
}

// ============================================================================
// Bibitem import (IDs format)
// ============================================================================

#[tokio::test]
async fn test_import_bibitems_csv() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Step 1: Create an author via API
    let author_payload = json!({
        "author_key": format!("kant-{suffix}"),
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

    // Step 2: Create a journal via API
    let journal_payload = json!({
        "journal_key": format!("mind-{suffix}"),
        "name_latex": "Mind",
        "name_unicode": "Mind",
        "name_simplified": "mind"
    });
    let journal_resp = app.post_json("/api/v1/journals", &journal_payload).await;
    assert_eq!(journal_resp.status(), 200);
    let journal: serde_json::Value = journal_resp.json().await.unwrap();
    let journal_id = journal["id"].as_i64().unwrap();

    // Step 3: Import bibitem CSV referencing the author and journal
    let csv = format!(
        "entry_type,bibkey,title_latex,title_unicode,title_simplified,author_ids,journal_id,date_year\n\
         article,kant:1781-{suffix},Critique of Pure Reason,Critique of Pure Reason,critique of pure reason,{author_id},{journal_id},1781"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/bibitems", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1, "Should import 1 bibitem");
    assert_eq!(body["failed"], 0);

    // Step 4: Verify the bibitem was created
    let bibitem_resp = app
        .get(&format!("/api/v1/bibitems/by-key/kant:1781-{suffix}"))
        .await;
    assert_eq!(bibitem_resp.status(), 200);
    let bibitem: serde_json::Value = bibitem_resp.json().await.unwrap();
    assert_eq!(bibitem["entry_type"], "article");
    assert_eq!(bibitem["title_unicode"], "Critique of Pure Reason");
    assert_eq!(bibitem["journal_id"], journal_id);
    assert_eq!(bibitem["date_year"], 1781);

    // Step 5: Verify the author junction was created
    let bibitem_id = bibitem["id"].as_i64().unwrap();
    let authors_resp = app
        .get(&format!("/api/v1/bibitems/{bibitem_id}/authors"))
        .await;
    assert_eq!(authors_resp.status(), 200);
    let authors_list: Vec<serde_json::Value> = authors_resp.json().await.unwrap();
    assert_eq!(authors_list.len(), 1);
    assert_eq!(authors_list[0]["author_id"], author_id);
    assert_eq!(authors_list[0]["role"], "author");
}

#[tokio::test]
async fn test_import_bibitems_with_editors() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create two authors -- one as author, one as editor
    let author1_resp = app
        .post_json(
            "/api/v1/authors",
            &json!({
                "author_key": format!("author1-{suffix}"),
                "family_name_latex": "Smith"
            }),
        )
        .await;
    let author1: serde_json::Value = author1_resp.json().await.unwrap();
    let author1_id = author1["id"].as_i64().unwrap();

    let author2_resp = app
        .post_json(
            "/api/v1/authors",
            &json!({
                "author_key": format!("editor1-{suffix}"),
                "family_name_latex": "Jones"
            }),
        )
        .await;
    let author2: serde_json::Value = author2_resp.json().await.unwrap();
    let editor_id = author2["id"].as_i64().unwrap();

    // Import bibitem with both author_ids and editor_ids
    let csv = format!(
        "entry_type,bibkey,title_latex,title_unicode,title_simplified,author_ids,editor_ids\n\
         incollection,test:collection-{suffix},A Chapter,A Chapter,a chapter,{author1_id},{editor_id}"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/bibitems", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);

    // Verify junction: should have author and editor
    let bib_resp = app
        .get(&format!("/api/v1/bibitems/by-key/test:collection-{suffix}"))
        .await;
    let bib: serde_json::Value = bib_resp.json().await.unwrap();
    let bib_id = bib["id"].as_i64().unwrap();

    let authors_resp = app.get(&format!("/api/v1/bibitems/{bib_id}/authors")).await;
    let authors_list: Vec<serde_json::Value> = authors_resp.json().await.unwrap();
    assert_eq!(
        authors_list.len(),
        2,
        "Should have 2 author links (author + editor)"
    );

    // Check that roles are correct
    let has_author = authors_list
        .iter()
        .any(|a| a["author_id"] == author1_id && a["role"] == "author");
    let has_editor = authors_list
        .iter()
        .any(|a| a["author_id"] == editor_id && a["role"] == "editor");
    assert!(has_author, "Should have an author-role link");
    assert!(has_editor, "Should have an editor-role link");
}

#[tokio::test]
async fn test_import_bibitems_missing_references() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // CSV with non-existent author_id
    let csv = format!(
        "entry_type,bibkey,title_latex,title_unicode,title_simplified,author_ids\n\
         article,test:missing-{suffix},Title,Title,title,99999"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/bibitems", &csv).await;
    assert_eq!(
        resp.status(),
        422,
        "Should return 422 for missing references"
    );

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["error"], "missing_references");
    let missing_authors = body["missing_author_ids"]
        .as_array()
        .expect("Expected missing_author_ids array");
    assert!(
        missing_authors.iter().any(|id| id.as_i64() == Some(99999)),
        "Should report 99999 as missing author ID"
    );

    // Verify nothing was inserted
    let bib_resp = app
        .get(&format!("/api/v1/bibitems/by-key/test:missing-{suffix}"))
        .await;
    assert_eq!(
        bib_resp.status(),
        404,
        "Bibitem should not exist after failed import"
    );
}

#[tokio::test]
async fn test_import_bibitems_with_keywords() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create keywords at different levels
    let kw1_resp = app
        .post_json(
            "/api/v1/keywords",
            &json!({
                "name": format!("epistemology-{suffix}"),
                "level": 1
            }),
        )
        .await;
    assert_eq!(kw1_resp.status(), 200);
    let kw1: serde_json::Value = kw1_resp.json().await.unwrap();
    let kw1_id = kw1["id"].as_i64().unwrap();

    let kw2_resp = app
        .post_json(
            "/api/v1/keywords",
            &json!({
                "name": format!("perception-{suffix}"),
                "level": 2
            }),
        )
        .await;
    assert_eq!(kw2_resp.status(), 200);
    let kw2: serde_json::Value = kw2_resp.json().await.unwrap();
    let kw2_id = kw2["id"].as_i64().unwrap();

    // Import bibitem with keyword_ids (comma-separated within the field)
    let csv = format!(
        "entry_type,bibkey,title_latex,title_unicode,title_simplified,keyword_ids\n\
         article,test:kw-{suffix},Knowledge,Knowledge,knowledge,\"{kw1_id},{kw2_id}\""
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/bibitems", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);

    // Verify keywords junction
    let bib_resp = app
        .get(&format!("/api/v1/bibitems/by-key/test:kw-{suffix}"))
        .await;
    let bib: serde_json::Value = bib_resp.json().await.unwrap();
    let bib_id = bib["id"].as_i64().unwrap();

    let kw_resp = app
        .get(&format!("/api/v1/bibitems/{bib_id}/keywords"))
        .await;
    assert_eq!(kw_resp.status(), 200);
    let kw_list: Vec<serde_json::Value> = kw_resp.json().await.unwrap();

    // Check that both keywords were linked
    let has_kw1 = kw_list.iter().any(|k| k["keyword_id"] == kw1_id);
    let has_kw2 = kw_list.iter().any(|k| k["keyword_id"] == kw2_id);
    assert!(
        has_kw1 && has_kw2,
        "Both keywords should be linked; got: {kw_list:?}"
    );
}

// ============================================================================
// Import without file field
// ============================================================================

#[tokio::test]
async fn test_import_no_file_field() {
    let app = TestApp::spawn().await;

    // Send multipart with wrong field name
    let form = reqwest::multipart::Form::new().part(
        "wrong_field",
        reqwest::multipart::Part::bytes(b"some data".to_vec())
            .file_name("test.csv")
            .mime_str("text/csv")
            .unwrap(),
    );

    let resp = app
        .client
        .post(app.url("/api/v1/admin/import/authors"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert!(
        resp.status() == 400 || resp.status() == 422,
        "Should reject missing file field, got {}",
        resp.status()
    );
}

// ============================================================================
// Import with missing required column
// ============================================================================

#[tokio::test]
async fn test_import_authors_missing_required_column() {
    let app = TestApp::spawn().await;

    // CSV without the required "author_key" column
    let csv = "family_name_latex,given_name_latex\nKant,Immanuel";

    let resp = upload_csv(&app, "/api/v1/admin/import/authors", csv).await;
    assert!(
        resp.status() == 400 || resp.status() == 422,
        "Should reject missing required column, got {}",
        resp.status()
    );
}

// ============================================================================
// Multiple bibitems import
// ============================================================================

#[tokio::test]
async fn test_import_multiple_bibitems() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "entry_type,bibkey,title_latex,title_unicode,title_simplified\n\
         article,test:multi-a-{suffix},Title A,Title A,title a\n\
         book,test:multi-b-{suffix},Title B,Title B,title b\n\
         misc,test:multi-c-{suffix},Title C,Title C,title c"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/bibitems", &csv).await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 3, "Should import 3 bibitems");
    assert_eq!(body["failed"], 0);

    // Verify each one
    for label in &["multi-a", "multi-b", "multi-c"] {
        let bib_resp = app
            .get(&format!("/api/v1/bibitems/by-key/test:{label}-{suffix}"))
            .await;
        assert_eq!(bib_resp.status(), 200, "Bibitem {label} should exist");
    }
}

// ============================================================================
// ID-based upsert — all four cases
// ============================================================================

/// Case: CSV has ID that does not exist in DB → creates the row with that ID.
#[tokio::test]
async fn test_import_author_with_explicit_id_creates() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "id,author_key,family_name_latex,given_name_latex\n\
         80001,kant-{suffix},Kant,Immanuel"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/authors", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);

    let get_resp = app.get("/api/v1/authors/80001").await;
    assert_eq!(get_resp.status(), 200);
    let author: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(author["id"], 80001);
    assert_eq!(author["author_key"], format!("kant-{suffix}"));
    assert_eq!(author["family_name_latex"], "Kant");
}

/// Case: CSV has ID that exists with the same key → updates the row.
#[tokio::test]
async fn test_import_author_id_exists_matching_key_updates() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create with explicit ID
    let csv_create = format!(
        "id,author_key,family_name_latex\n\
         80002,kant-{suffix},Kant"
    );
    let r = upload_csv(&app, "/api/v1/admin/import/authors", &csv_create).await;
    assert_eq!(r.json::<serde_json::Value>().await.unwrap()["imported"], 1);

    // Re-import same ID + same key, different field value → update
    let csv_update = format!(
        "id,author_key,family_name_latex,given_name_latex\n\
         80002,kant-{suffix},Kant,Immanuel"
    );
    let resp = upload_csv(&app, "/api/v1/admin/import/authors", &csv_update).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["updated"], 1, "should update existing author");
    assert_eq!(body["failed"], 0);

    let author: serde_json::Value = app.get("/api/v1/authors/80002").await.json().await.unwrap();
    assert_eq!(author["given_name_latex"], "Immanuel");
}

/// Case: CSV has ID that exists but with a different key → row-level error.
#[tokio::test]
async fn test_import_author_id_exists_mismatched_key_errors() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv_create = format!(
        "id,author_key,family_name_latex\n\
         80003,kant-{suffix},Kant"
    );
    upload_csv(&app, "/api/v1/admin/import/authors", &csv_create).await;

    let csv_mismatch = format!(
        "id,author_key,family_name_latex\n\
         80003,hegel-{suffix},Hegel"
    );
    let resp = upload_csv(&app, "/api/v1/admin/import/authors", &csv_mismatch).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["failed"], 1, "key mismatch must fail");
    assert_eq!(body["imported"], 0);
    assert_eq!(body["updated"], 0);
}

/// Case: CSV has no ID and ?auto_assign_ids is absent → row-level error.
#[tokio::test]
async fn test_import_author_no_id_without_flag_errors() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "author_key,family_name_latex\n\
         kant-{suffix},Kant"
    );
    let resp = upload_csv(&app, "/api/v1/admin/import/authors", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        body["failed"], 1,
        "missing id without auto_assign must fail"
    );
    assert_eq!(body["imported"], 0);
}

// ============================================================================
// ?auto_assign_ids=true
// ============================================================================

#[tokio::test]
async fn test_import_authors_auto_assign_ids() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "author_key,family_name_latex,given_name_latex\n\
         kant-{suffix},Kant,Immanuel\n\
         hegel-{suffix},Hegel,Georg"
    );

    let resp = upload_csv(
        &app,
        "/api/v1/admin/import/authors?auto_assign_ids=true",
        &csv,
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 2);
    assert_eq!(body["failed"], 0);

    // Both must exist with server-assigned IDs
    let kant = app
        .get(&format!("/api/v1/authors/by-key/kant-{suffix}"))
        .await;
    assert_eq!(kant.status(), 200);

    let hegel = app
        .get(&format!("/api/v1/authors/by-key/hegel-{suffix}"))
        .await;
    assert_eq!(hegel.status(), 200);

    // IDs must be distinct
    let kant_id = kant.json::<serde_json::Value>().await.unwrap()["id"].clone();
    let hegel_id = hegel.json::<serde_json::Value>().await.unwrap()["id"].clone();
    assert_ne!(kant_id, hegel_id);
}

#[tokio::test]
async fn test_import_journals_auto_assign_ids() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let csv = format!(
        "journal_key,name_latex,name_unicode\n\
         mind-{suffix},Mind,Mind\n\
         nous-{suffix},No\\^{{u}}s,Noûs"
    );

    let resp = upload_csv(
        &app,
        "/api/v1/admin/import/journals?auto_assign_ids=true",
        &csv,
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 2);
    assert_eq!(body["failed"], 0);
}

// ============================================================================
// Author name variants import
// ============================================================================

#[tokio::test]
async fn test_import_author_name_variants() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create an author
    let payload = json!({
        "author_key": format!("kant-{suffix}"),
        "family_name_latex": "Kant",
        "given_name_latex": "Immanuel"
    });
    let author: serde_json::Value = app
        .post_json("/api/v1/authors", &payload)
        .await
        .json()
        .await
        .unwrap();
    let author_id = author["id"].as_i64().unwrap();

    // Import two LaTeX name variants ("Kant, I." is quoted because it contains a comma)
    let csv = format!(
        "name_variant,type,profile_id\n\
         Kant I.,latex,{author_id}\n\
         \"Kant, I.\",latex,{author_id}"
    );

    let resp = upload_csv(&app, "/api/v1/admin/import/author-name-variants", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["updated"], 2, "two variants should be stored");
    assert_eq!(body["failed"], 0);

    // Verify both variants appear on the author record
    let updated: serde_json::Value = app
        .get(&format!("/api/v1/authors/by-key/kant-{suffix}"))
        .await
        .json()
        .await
        .unwrap();
    let variants = updated["name_variants_latex"].as_array().unwrap();
    assert!(
        variants.iter().any(|v| v.as_str() == Some("Kant I.")),
        "variant 'Kant I.' should be stored"
    );
    assert!(
        variants.iter().any(|v| v.as_str() == Some("Kant, I.")),
        "variant 'Kant, I.' should be stored"
    );
}

#[tokio::test]
async fn test_import_name_variant_duplicate_is_idempotent() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let payload = json!({
        "author_key": format!("hegel-{suffix}"),
        "family_name_latex": "Hegel"
    });
    let author: serde_json::Value = app
        .post_json("/api/v1/authors", &payload)
        .await
        .json()
        .await
        .unwrap();
    let author_id = author["id"].as_i64().unwrap();

    let csv = format!(
        "name_variant,type,profile_id\n\
         Hegel G.,latex,{author_id}"
    );

    // Import the same variant twice
    upload_csv(&app, "/api/v1/admin/import/author-name-variants", &csv).await;
    let resp2 = upload_csv(&app, "/api/v1/admin/import/author-name-variants", &csv).await;
    assert_eq!(resp2.status(), 200);
    let body: serde_json::Value = resp2.json().await.unwrap();
    assert_eq!(body["failed"], 0, "duplicate variant should not error");

    // Still only one entry in the array
    let updated: serde_json::Value = app
        .get(&format!("/api/v1/authors/by-key/hegel-{suffix}"))
        .await
        .json()
        .await
        .unwrap();
    let variants = updated["name_variants_latex"].as_array().unwrap();
    assert_eq!(
        variants
            .iter()
            .filter(|v| v.as_str() == Some("Hegel G."))
            .count(),
        1,
        "variant should appear exactly once"
    );
}

#[tokio::test]
async fn test_import_name_variant_unknown_author_errors() {
    let app = TestApp::spawn().await;

    let csv = "name_variant,type,profile_id\n\
               Some Variant,latex,99999999";

    let resp = upload_csv(&app, "/api/v1/admin/import/author-name-variants", csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["failed"], 1, "unknown author id should fail");
    assert_eq!(body["updated"], 0);
}

// ============================================================================
// Bibitem refs import
// ============================================================================

fn zip_file_content(bytes: &[u8], path: &str) -> String {
    use std::io::Read;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut file = archive
        .by_name(path)
        .unwrap_or_else(|_| panic!("'{path}' not found in ZIP"));
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    content
}

async fn snapshot_bytes(app: &TestApp) -> Vec<u8> {
    app.client
        .post(app.url("/api/v1/admin/snapshot"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .to_vec()
}

#[tokio::test]
async fn test_import_bibitem_refs_requires_auth() {
    let app = TestApp::spawn().await;
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"source_id,target_id,ref_type\n1,2,further_ref".to_vec())
            .file_name("test.csv")
            .mime_str("text/csv")
            .unwrap(),
    );
    let resp = app
        .client
        .post(app.url("/api/v1/admin/import/bibitem-refs"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_import_bibitem_refs_happy_path() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let bib1: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("source:{s}"), "entry_type": "article",
                      "title_latex": "S", "title_unicode": "S" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let bib2: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("target:{s}"), "entry_type": "book",
                      "title_latex": "T", "title_unicode": "T" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let src_id = bib1["id"].as_i64().unwrap();
    let tgt_id = bib2["id"].as_i64().unwrap();

    let csv = format!("source_id,target_id,ref_type\n{src_id},{tgt_id},further_ref");
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);

    // Verify via snapshot
    let zip = snapshot_bytes(&app).await;
    let refs_csv = zip_file_content(&zip, "bibitem_refs/all.csv");
    assert!(refs_csv.contains(&src_id.to_string()));
    assert!(refs_csv.contains(&tgt_id.to_string()));
    assert!(refs_csv.contains("further_ref"));
}

#[tokio::test]
async fn test_import_bibitem_refs_idempotent() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let bib1: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("src2:{s}"), "entry_type": "article",
                      "title_latex": "S", "title_unicode": "S" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let bib2: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("tgt2:{s}"), "entry_type": "book",
                      "title_latex": "T", "title_unicode": "T" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let src_id = bib1["id"].as_i64().unwrap();
    let tgt_id = bib2["id"].as_i64().unwrap();

    let csv = format!("source_id,target_id,ref_type\n{src_id},{tgt_id},depends_on");
    upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &csv).await;
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    // ON CONFLICT DO NOTHING — second import counts as 1 imported (row processed), 0 failed
    assert_eq!(body["failed"], 0);
}

#[tokio::test]
async fn test_import_bibitem_refs_missing_bibitem_ids() {
    let app = TestApp::spawn().await;
    let csv = "source_id,target_id,ref_type\n99991,99992,further_ref";
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", csv).await;
    assert!(
        resp.status() == 400 || resp.status() == 422,
        "missing bibitem IDs must return 4xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_import_bibitem_refs_invalid_ref_type() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let bib1: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("src3:{s}"), "entry_type": "article",
                      "title_latex": "S", "title_unicode": "S" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let bib2: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("tgt3:{s}"), "entry_type": "book",
                      "title_latex": "T", "title_unicode": "T" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let src_id = bib1["id"].as_i64().unwrap();
    let tgt_id = bib2["id"].as_i64().unwrap();

    let csv = format!("source_id,target_id,ref_type\n{src_id},{tgt_id},bad_type");
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["failed"], 1);
    assert_eq!(body["imported"], 0);
}

// ============================================================================
// Bibitem notes import
// ============================================================================

#[tokio::test]
async fn test_import_bibitem_notes_requires_auth() {
    let app = TestApp::spawn().await;
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"bibitem_id,note_perso\n1,hello".to_vec())
            .file_name("test.csv")
            .mime_str("text/csv")
            .unwrap(),
    );
    let resp = app
        .client
        .post(app.url("/api/v1/admin/import/bibitem-notes"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_import_bibitem_notes_happy_path() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let bib: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("bib-notes:{s}"), "entry_type": "book",
                      "title_latex": "B", "title_unicode": "B" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let bib_id = bib["id"].as_i64().unwrap();

    let csv = format!("bibitem_id,note_perso\n{bib_id},a personal note");
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-notes", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);

    // Verify via snapshot
    let zip = snapshot_bytes(&app).await;
    let notes_csv = zip_file_content(&zip, "bibitem_notes/all.csv");
    assert!(notes_csv.contains(&bib_id.to_string()));
    assert!(notes_csv.contains("a personal note"));
}

#[tokio::test]
async fn test_import_bibitem_notes_upsert_updates_existing() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let bib: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("bib-notes2:{s}"), "entry_type": "book",
                      "title_latex": "B", "title_unicode": "B" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let bib_id = bib["id"].as_i64().unwrap();

    // First import
    let csv1 = format!("bibitem_id,note_perso\n{bib_id},first note");
    upload_csv(&app, "/api/v1/admin/import/bibitem-notes", &csv1).await;

    // Second import with different note — should upsert
    let csv2 = format!("bibitem_id,note_perso\n{bib_id},updated note");
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-notes", &csv2).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["failed"], 0);

    // Snapshot should show only one row for this bibitem with the updated note
    let zip = snapshot_bytes(&app).await;
    let notes_csv = zip_file_content(&zip, "bibitem_notes/all.csv");
    let count = notes_csv
        .lines()
        .filter(|l| l.contains(&bib_id.to_string()))
        .count();
    assert_eq!(count, 1, "upsert must not create a duplicate row");
    assert!(notes_csv.contains("updated note"));
}

#[tokio::test]
async fn test_import_bibitem_notes_missing_bibitem_id() {
    let app = TestApp::spawn().await;
    let csv = "bibitem_id,note_perso\n99993,some note";
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-notes", csv).await;
    assert!(
        resp.status() == 400 || resp.status() == 422,
        "missing bibitem ID must return 4xx, got {}",
        resp.status()
    );
}

#[tokio::test]
async fn test_import_bibitem_notes_multiple_columns() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    let bib: serde_json::Value = app
        .post_json(
            "/api/v1/bibitems",
            &json!({ "bibkey": format!("bib-notes3:{s}"), "entry_type": "book",
                      "title_latex": "B", "title_unicode": "B" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let bib_id = bib["id"].as_i64().unwrap();

    let csv = format!(
        "bibitem_id,note_perso,note_stock,change_request\n{bib_id},perso note,stock note,fix this"
    );
    let resp = upload_csv(&app, "/api/v1/admin/import/bibitem-notes", &csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1);
    assert_eq!(body["failed"], 0);

    let zip = snapshot_bytes(&app).await;
    let notes_csv = zip_file_content(&zip, "bibitem_notes/all.csv");
    assert!(notes_csv.contains("perso note"));
    assert!(notes_csv.contains("stock note"));
    assert!(notes_csv.contains("fix this"));
}

// ============================================================================
// Sequence sync after explicit-ID import
// ============================================================================

#[tokio::test]
async fn test_sequence_synced_after_explicit_id_import() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Import an author with a large explicit ID
    let csv = format!(
        "id,author_key,family_name_latex\n\
         90000,kant-{suffix},Kant"
    );
    let r = upload_csv(&app, "/api/v1/admin/import/authors", &csv).await;
    assert_eq!(r.json::<serde_json::Value>().await.unwrap()["imported"], 1);

    // Create a new author via the API (no explicit ID).
    // If the sequence was NOT synced past 90000 this would conflict and return 500.
    let payload = json!({
        "author_key": format!("hegel-{suffix}"),
        "family_name_latex": "Hegel"
    });
    let create_resp = app.post_json("/api/v1/authors", &payload).await;
    assert_eq!(
        create_resp.status(),
        200,
        "new author must not conflict with explicitly-imported ID 90000"
    );
    let new_author: serde_json::Value = create_resp.json().await.unwrap();
    assert!(
        new_author["id"].as_i64().unwrap() > 90000,
        "auto-assigned ID must be above the imported explicit ID"
    );
}
