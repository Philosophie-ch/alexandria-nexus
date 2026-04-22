//! Integration tests for POST /api/v1/admin/bulk-import/{table}.
//!
//! Covers: all supported tables, column filtering (extra columns dropped),
//! data round-trip integrity, missing required columns → 400, unknown table → 400.

mod common;
use common::TestApp;

const BIBITEM_HEADER: &str = "id,entry_type,bibkey,options,shorthand,date_year,pubstate,title_latex,title_unicode,booktitle_latex,booktitle_unicode,crossref,journal_key,volume,number,pages,eid,series_key,address,institution_key,school_key,publisher_key,type_field,edition,note_latex,note_unicode,issuetitle_latex,issuetitle_unicode,extra_note_latex,extra_note_unicode,urn,eprint,doi,url,langid,is_translation,epoch,author_keys,editor_keys,guesteditor_keys,keyword_keys";

const AUTHOR_HEADER: &str = "id,author_key,given_name_latex,given_name_unicode,family_name_latex,family_name_unicode,mononym_latex,mononym_unicode,shorthand_latex,shorthand_unicode,famous_name_latex,famous_name_unicode,famous,name_variants_latex,name_variants_unicode";

async fn bulk_import(app: &TestApp, table: &str, csv: &str) -> reqwest::Response {
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(csv.as_bytes().to_vec())
            .file_name("test.csv")
            .mime_str("text/csv")
            .unwrap(),
    );
    app.client
        .post(app.url(&format!("/api/v1/admin/bulk-import/{table}")))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .multipart(form)
        .send()
        .await
        .expect("request failed")
}

// ── error cases ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_bulk_import_unknown_table_returns_400() {
    let app = TestApp::spawn().await;
    let resp = bulk_import(&app, "nonexistent_table", "a,b\n1,2\n").await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_bulk_import_missing_required_column_returns_400() {
    let app = TestApp::spawn().await;
    // journals requires journal_key — omit it
    let csv = "name_latex,name_unicode\nPhilosophy Review,Philosophy Review\n";
    let resp = bulk_import(&app, "journals", csv).await;
    assert_eq!(resp.status(), 400);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("missing required columns"),
        "got: {body}"
    );
}

// ── entity tables ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_bulk_import_journals() {
    let app = TestApp::spawn().await;
    let csv = "\
id,journal_key,name_latex,name_unicode,issn_print,issn_electronic
1,phil-review,Philosophical Review,Philosophical Review,0031-8108,
2,mind-journal,Mind,Mind,,0026-4423
";
    let resp = bulk_import(&app, "journals", csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["rows"], 2);

    let j = app.get("/api/v1/journals/by-key/phil-review").await;
    assert_eq!(j.status(), 200);
    let j: serde_json::Value = j.json().await.unwrap();
    assert_eq!(j["name_latex"], "Philosophical Review");
    assert_eq!(j["issn_print"], "0031-8108");
    assert_eq!(j["issn_electronic"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_bulk_import_publishers() {
    let app = TestApp::spawn().await;
    let csv = "\
id,publisher_key,name_latex,name_unicode,default_address
1,oxford-up,Oxford University Press,Oxford University Press,Oxford
2,cambridge-up,Cambridge University Press,Cambridge University Press,
";
    let resp = bulk_import(&app, "publishers", csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["rows"], 2);

    let p = app.get("/api/v1/publishers/by-key/oxford-up").await;
    assert_eq!(p.status(), 200);
    let p: serde_json::Value = p.json().await.unwrap();
    assert_eq!(p["default_address"], "Oxford");
}

#[tokio::test]
async fn test_bulk_import_institutions() {
    let app = TestApp::spawn().await;
    let csv = "\
id,institution_key,name_latex,name_unicode,default_address
1,cnrs,CNRS,CNRS,Paris
";
    let resp = bulk_import(&app, "institutions", csv).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);
}

#[tokio::test]
async fn test_bulk_import_schools_extra_column_silently_dropped() {
    let app = TestApp::spawn().await;
    // snapshot CSVs may include extra columns — they should be silently dropped
    let csv = "\
id,school_key,name_latex,name_unicode,legacy_extra
1,eth-zurich,ETH,ETH,ignored
";
    let resp = bulk_import(&app, "schools", csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);
}

#[tokio::test]
async fn test_bulk_import_series() {
    let app = TestApp::spawn().await;
    let csv = "id,series_key,name_latex,name_unicode\n1,synthese-lib,Synth Library,Synth Library\n";
    let resp = bulk_import(&app, "series", csv).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);
}

#[tokio::test]
async fn test_bulk_import_keywords() {
    let app = TestApp::spawn().await;
    let csv = "\
id,keyword_key,name,level
1,1:epistemology,epistemology,1
2,2:knowledge,knowledge,2
3,3:justified-belief,justified belief,3
";
    let resp = bulk_import(&app, "keywords", csv).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 3);
}

// ── authors ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_bulk_import_authors_basic() {
    let app = TestApp::spawn().await;
    // mononym_latex is column 6 (0-indexed), must be non-empty for Aristotle
    let csv = format!(
        "{AUTHOR_HEADER}\n\
         1,kant_i,Immanuel,Immanuel,Kant,Kant,,,,,,,false,,\n\
         2,aristotle,,,,,Aristotle,Aristotle,,,,,false,,\n"
    );
    let resp = bulk_import(&app, "authors", &csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["rows"], 2);

    let a = app.get("/api/v1/authors/by-key/kant_i").await;
    assert_eq!(a.status(), 200);
    let a: serde_json::Value = a.json().await.unwrap();
    assert_eq!(a["family_name_latex"], "Kant");
    assert_eq!(a["famous"], false);
}

#[tokio::test]
async fn test_bulk_import_authors_extra_columns_dropped() {
    let app = TestApp::spawn().await;
    let csv = format!(
        "{AUTHOR_HEADER},legacy_column\n\
         1,smith_j,John,John,Smith,Smith,,,,,,,false,,,should_be_ignored\n"
    );
    let resp = bulk_import(&app, "authors", &csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);
}

// ── bibitems ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_bulk_import_bibitems_basic() {
    let app = TestApp::spawn().await;

    bulk_import(
        &app,
        "journals",
        "id,journal_key,name_latex,name_unicode,issn_print,issn_electronic\n\
         1,test-journal,Test Journal,Test Journal,,\n",
    )
    .await;

    // Correct 41-column row (generated by Python script above)
    let csv = format!(
        "{BIBITEM_HEADER}\n\
         1,article,smith:2020,,,2020,,A Test Article,A Test Article,,,,test-journal,5,2,123--130,,,,,,,,,,,,,,,,,,,,false,,smith_j,,,1:epistemology\n"
    );
    let resp = bulk_import(&app, "bibitems", &csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);

    let b = app.get("/api/v1/bibitems/by-key/smith:2020").await;
    assert_eq!(b.status(), 200);
    let b: serde_json::Value = b.json().await.unwrap();
    assert_eq!(b["bibkey"], "smith:2020");
    assert_eq!(b["title_latex"], "A Test Article");
    assert_eq!(b["journal_key"], "test-journal");
    // Junction columns are not on the bibitem row
    assert!(b.get("author_keys").is_none());
}

#[tokio::test]
async fn test_bulk_import_bibitems_nullable_fields_are_null() {
    let app = TestApp::spawn().await;
    let csv = format!(
        "{BIBITEM_HEADER}\n\
         1,book,min:2021,,,2021,,Minimal Book,Minimal Book,,,,,,,,,,,,,,,,,,,,,,,,,,,false,,,,,\n"
    );
    let resp = bulk_import(&app, "bibitems", &csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());

    let b = app.get("/api/v1/bibitems/by-key/min:2021").await;
    let b: serde_json::Value = b.json().await.unwrap();
    assert_eq!(b["journal_key"], serde_json::Value::Null);
    assert_eq!(b["crossref"], serde_json::Value::Null);
}

// ── junction tables ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_bulk_import_bibitem_authors() {
    let app = TestApp::spawn().await;

    bulk_import(
        &app,
        "journals",
        "id,journal_key,name_latex,name_unicode,issn_print,issn_electronic\n1,j,J,J,,\n",
    )
    .await;
    bulk_import(
        &app,
        "authors",
        &format!("{AUTHOR_HEADER}\n1,kant_i,Immanuel,Immanuel,Kant,Kant,,,,,,,false,,\n"),
    )
    .await;
    bulk_import(&app, "bibitems", &format!(
        "{BIBITEM_HEADER}\n\
         1,article,kant:1781,,,1781,,Critique,Critique,,,,j,,,,,,,,,,,,,,,,,,,,,,,false,,kant_i,,,\n"
    )).await;

    let csv = "\
bibkey,author_key,role,position,name_variant_latex,name_variant_unicode
kant:1781,kant_i,author,0,,
";
    let resp = bulk_import(&app, "bibitem_authors", csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);

    let b = app.get("/api/v1/bibitems/by-key/kant:1781").await;
    let b: serde_json::Value = b.json().await.unwrap();
    let id = b["id"].as_i64().unwrap();
    let authors = app.get(&format!("/api/v1/bibitems/{id}/authors")).await;
    assert_eq!(authors.status(), 200);
    let authors: Vec<serde_json::Value> = authors.json().await.unwrap();
    assert_eq!(authors.len(), 1);
}

#[tokio::test]
async fn test_bulk_import_bibitem_keywords() {
    let app = TestApp::spawn().await;

    bulk_import(
        &app,
        "keywords",
        "id,keyword_key,name,level\n1,1:epistemology,epistemology,1\n",
    )
    .await;
    bulk_import(
        &app,
        "bibitems",
        &format!(
            "{BIBITEM_HEADER}\n\
         1,book,test:2000,,,2000,,Test,Test,,,,,,,,,,,,,,,,,,,,,,,,,,,false,,,,,\n"
        ),
    )
    .await;

    let csv = "\
bibkey,keyword_key,keyword_level
test:2000,1:epistemology,1
";
    let resp = bulk_import(&app, "bibitem_keywords", csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);
}

#[tokio::test]
async fn test_bulk_import_bibitem_refs() {
    let app = TestApp::spawn().await;

    bulk_import(
        &app,
        "bibitems",
        &format!(
            "{BIBITEM_HEADER}\n\
         1,book,source:2000,,,2000,,Source,Source,,,,,,,,,,,,,,,,,,,,,,,,,,,false,,,,,\n\
         2,book,target:1999,,,1999,,Target,Target,,,,,,,,,,,,,,,,,,,,,,,,,,,false,,,,,\n"
        ),
    )
    .await;

    let csv = "\
source_key,target_key,ref_type
source:2000,target:1999,depends_on
";
    let resp = bulk_import(&app, "bibitem_refs", csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);
}

#[tokio::test]
async fn test_bulk_import_bibitem_notes() {
    let app = TestApp::spawn().await;

    bulk_import(
        &app,
        "bibitems",
        &format!(
            "{BIBITEM_HEADER}\n\
         1,book,noted:2000,,,2000,,Noted,Noted,,,,,,,,,,,,,,,,,,,,,,,,,,,false,,,,,\n"
        ),
    )
    .await;

    let csv = "\
bibkey,note_perso,note_stock,note_missing,change_request,dltc_copyediting_note,todo_general
noted:2000,my note,,,,,
";
    let resp = bulk_import(&app, "bibitem_notes", csv).await;
    assert_eq!(resp.status(), 200, "{}", resp.text().await.unwrap());
    assert_eq!(resp.json::<serde_json::Value>().await.unwrap()["rows"], 1);
}

// ── count verification ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_bulk_import_row_count_matches_db() {
    let app = TestApp::spawn().await;
    let csv = "\
id,journal_key,name_latex,name_unicode,issn_print,issn_electronic
1,j1,Journal One,Journal One,,
2,j2,Journal Two,Journal Two,,
3,j3,Journal Three,Journal Three,,
";
    let resp = bulk_import(&app, "journals", csv).await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["rows"], 3);

    let list = app.get("/api/v1/journals").await;
    let list: serde_json::Value = list.json().await.unwrap();
    assert_eq!(list["total"], 3);
}
