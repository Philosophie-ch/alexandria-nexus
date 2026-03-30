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

    let resp = upload_csv(&app, "/api/v1/admin/import/authors", &csv).await;
    assert_eq!(resp.status(), 200, "Import authors should return 200");

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["imported"], 1, "Should import 1 valid author (Kant)");
    // Plato row has no family/given/mononym names and only author_key, so it
    // depends on validation rules whether it fails. At minimum Kant should import.

    // Verify Kant exists
    let get_resp = app
        .get(&format!("/api/v1/authors/by-key/kant-{suffix}"))
        .await;
    assert_eq!(get_resp.status(), 200);
    let author: serde_json::Value = get_resp.json().await.unwrap();
    assert_eq!(author["family_name_latex"], "Kant");
    assert_eq!(author["given_name_simplified"], "immanuel");
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

    let resp = upload_csv(&app, "/api/v1/admin/import/authors", &csv).await;
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

    let resp = upload_csv(&app, "/api/v1/admin/import/journals", &csv).await;
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

    let resp = upload_csv(&app, "/api/v1/admin/import/publishers", &csv).await;
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
    assert_eq!(bibitem["title_simplified"], "critique of pure reason");
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
