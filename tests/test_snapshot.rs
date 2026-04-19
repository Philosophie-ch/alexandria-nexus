//! Snapshot endpoint integration tests.

mod common;

use std::io::Read;

use common::{TestApp, unique_suffix};
use serde_json::json;

// ── ZIP helpers ───────────────────────────────────────────────────────────────

fn zip_file_names(bytes: &[u8]) -> Vec<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    names.sort();
    names
}

fn zip_file_content(bytes: &[u8], path: &str) -> String {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut file = archive
        .by_name(path)
        .unwrap_or_else(|_| panic!("'{path}' not found in ZIP"));
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    content
}

async fn post_snapshot(app: &TestApp) -> reqwest::Response {
    app.client
        .post(app.url("/api/v1/admin/snapshot"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .expect("Failed to POST /admin/snapshot")
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_requires_auth() {
    let app = TestApp::spawn().await;
    let resp = app
        .client
        .post(app.url("/api/v1/admin/snapshot"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

// ── Empty DB ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_empty_db_returns_valid_zip() {
    let app = TestApp::spawn().await;
    let resp = post_snapshot(&app).await;

    assert_eq!(resp.status(), 200);

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("application/zip"),
        "expected application/zip, got: {content_type}"
    );

    let bytes = resp.bytes().await.unwrap();
    let files = zip_file_names(&bytes);

    for table in &[
        "journal",
        "publisher",
        "institution",
        "school",
        "series",
        "keyword",
    ] {
        assert!(
            files.contains(&format!("{table}/all.csv")),
            "missing {table}/all.csv in ZIP"
        );
    }
    assert!(files.contains(&"bibitem_refs/all.csv".to_string()));
    assert!(files.contains(&"bibitem_notes/all.csv".to_string()));

    // No per-prefix files when DB is empty
    assert!(!files.iter().any(|f| f.starts_with("author/")));
    assert!(!files.iter().any(|f| f.starts_with("bibitem/")));
}

// ── Data content ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_contains_seeded_entities() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Author with key starting with 'k' → goes into author/k.csv
    let author_key = format!("kant-{s}");
    let resp = app
        .post_json(
            "/api/v1/authors",
            &json!({
                "author_key": &author_key,
                "family_name_latex": "Kant",
                "family_name_unicode": "Kant",
            }),
        )
        .await;
    assert_eq!(resp.status(), 200, "Failed to create author");

    // Journal → goes into journal/all.csv
    let journal_key = format!("jnl-{s}");
    let resp = app
        .post_json(
            "/api/v1/journals",
            &json!({
                "journal_key": &journal_key,
                "name_latex": "Test Journal",
                "name_unicode": "Test Journal",
            }),
        )
        .await;
    assert_eq!(resp.status(), 200, "Failed to create journal");

    // Bibitem with bibkey starting with "ka" → goes into bibitem/ka.csv
    let bibkey = format!("ka-{s}:2024");
    let resp = app
        .post_json(
            "/api/v1/bibitems",
            &json!({
                "bibkey": &bibkey,
                "entry_type": "book",
                "title_latex": "Critique of Pure Reason",
                "title_unicode": "Critique of Pure Reason",
            }),
        )
        .await;
    assert_eq!(resp.status(), 200, "Failed to create bibitem");

    let resp = post_snapshot(&app).await;
    assert_eq!(resp.status(), 200);
    let bytes = resp.bytes().await.unwrap();
    let files = zip_file_names(&bytes);

    // Correct prefix files exist
    assert!(
        files.contains(&"author/k.csv".to_string()),
        "missing author/k.csv"
    );
    assert!(
        files.contains(&"bibitem/ka.csv".to_string()),
        "missing bibitem/ka.csv"
    );
    assert!(files.contains(&"bibitem_authors/ka.csv".to_string()));
    assert!(files.contains(&"bibitem_keywords/ka.csv".to_string()));

    // Author CSV contains the seeded author
    let author_csv = zip_file_content(&bytes, "author/k.csv");
    assert!(
        author_csv.contains(&author_key),
        "author_key not in author/k.csv"
    );
    assert!(
        author_csv.contains("Kant"),
        "family_name not in author/k.csv"
    );

    // Bibitem CSV contains the seeded bibitem
    let bib_csv = zip_file_content(&bytes, "bibitem/ka.csv");
    assert!(bib_csv.contains(&bibkey), "bibkey not in bibitem/ka.csv");
    assert!(bib_csv.contains("book"), "entry_type not in bibitem/ka.csv");

    // Journal shows up in the small-table file
    let journal_csv = zip_file_content(&bytes, "journal/all.csv");
    assert!(
        journal_csv.contains(&journal_key),
        "journal_key not in journal/all.csv"
    );
}

// ── Multiple prefix groups ────────────────────────────────────────────────────

#[tokio::test]
async fn test_snapshot_multiple_bibitem_prefixes() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    for (bibkey_prefix, entry_type) in &[("ka", "book"), ("ar", "article")] {
        let bibkey = format!("{bibkey_prefix}-{s}:2024");
        let resp = app
            .post_json(
                "/api/v1/bibitems",
                &json!({
                    "bibkey": bibkey,
                    "entry_type": entry_type,
                    "title_latex": "Title",
                    "title_unicode": "Title",
                }),
            )
            .await;
        assert_eq!(resp.status(), 200);
    }

    let bytes = post_snapshot(&app).await.bytes().await.unwrap();
    let files = zip_file_names(&bytes);

    assert!(
        files.contains(&"bibitem/ka.csv".to_string()),
        "missing bibitem/ka.csv"
    );
    assert!(
        files.contains(&"bibitem/ar.csv".to_string()),
        "missing bibitem/ar.csv"
    );
    assert!(
        !files.contains(&"bibitem/zz.csv".to_string()),
        "unexpected bibitem/zz.csv"
    );
}
