//! Integration tests for sortable_columns configuration.
//!
//! Verifies that each entity's declared sortable columns are accepted and that
//! undeclared columns are rejected with 400.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

// ============================================================================
// KEYWORDS — the bug-fix target (name, not name_unicode)
// ============================================================================

#[tokio::test]
async fn test_keyword_sort_by_name_asc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let names = ["Zebra", "Alpha", "Middle"];
    for (i, name) in names.iter().enumerate() {
        let resp = app
            .post_json(
                "/api/v1/keywords",
                &json!({
                    "keyword_key": format!("{}:{}-{}", i + 1, name, suffix),
                    "name": format!("{}-{}", name, suffix),
                    "level": 1
                }),
            )
            .await;
        assert_eq!(resp.status(), 200, "Failed to create keyword {name}");
    }

    let resp = app
        .get(&format!(
            "/api/v1/keywords?sort_by=name&sort_dir=asc&name={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert_eq!(items.len(), 3);

    let returned_names: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
    for window in returned_names.windows(2) {
        assert!(
            window[0] <= window[1],
            "Expected ascending order, got {:?}",
            returned_names
        );
    }
}

#[tokio::test]
async fn test_keyword_sort_by_name_desc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let names = ["Zebra", "Alpha", "Middle"];
    for (i, name) in names.iter().enumerate() {
        let resp = app
            .post_json(
                "/api/v1/keywords",
                &json!({
                    "keyword_key": format!("{}:{}-{}", i + 1, name, suffix),
                    "name": format!("{}-{}", name, suffix),
                    "level": 1
                }),
            )
            .await;
        assert_eq!(resp.status(), 200);
    }

    let resp = app
        .get(&format!(
            "/api/v1/keywords?sort_by=name&sort_dir=desc&name={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert_eq!(items.len(), 3);

    let returned_names: Vec<&str> = items.iter().filter_map(|i| i["name"].as_str()).collect();
    for window in returned_names.windows(2) {
        assert!(
            window[0] >= window[1],
            "Expected descending order, got {:?}",
            returned_names
        );
    }
}

#[tokio::test]
async fn test_keyword_sort_by_keyword_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/keywords?sort_by=keyword_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_keyword_sort_by_level() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/keywords?sort_by=level&sort_dir=asc").await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_keyword_sort_accepts_all_columns() {
    let app = TestApp::spawn().await;

    for col in ["keyword_key", "name", "level", "created_at", "updated_at"] {
        let resp = app
            .get(&format!("/api/v1/keywords?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "Keyword column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_keyword_sort_by_name_unicode_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/keywords?sort_by=name_unicode").await;
    assert_eq!(
        resp.status(),
        400,
        "Keyword has no name_unicode column; sort should be rejected"
    );
}

#[tokio::test]
async fn test_keyword_sort_by_nonexistent_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/keywords?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

// ============================================================================
// AUTHORS
// ============================================================================

#[tokio::test]
async fn test_author_sort_by_family_name_unicode() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for (key, family) in [("z", "Zeno"), ("a", "Aristotle"), ("m", "Marx")] {
        let resp = app
            .post_json(
                "/api/v1/authors",
                &json!({
                    "author_key": format!("{key}-{suffix}"),
                    "family_name_latex": family,
                    "family_name_unicode": format!("{family}-{suffix}"),
                    "family_name_simplified": family.to_lowercase()
                }),
            )
            .await;
        assert_eq!(resp.status(), 200, "Failed to create author {family}");
    }

    let resp = app
        .get(&format!(
            "/api/v1/authors?sort_by=family_name_unicode&sort_dir=asc&family_name={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().expect("Expected items array");
    assert_eq!(items.len(), 3);

    let returned: Vec<&str> = items
        .iter()
        .filter_map(|i| i["family_name_unicode"].as_str())
        .collect();
    for window in returned.windows(2) {
        assert!(
            window[0] <= window[1],
            "Expected ascending order, got {:?}",
            returned
        );
    }
}

#[tokio::test]
async fn test_author_sort_by_author_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/authors?sort_by=author_key&sort_dir=desc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_author_sort_by_given_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/authors?sort_by=given_name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_author_sort_accepts_all_columns() {
    let app = TestApp::spawn().await;

    for col in [
        "author_key",
        "family_name_unicode",
        "given_name_unicode",
        "famous",
        "mononym_unicode",
        "shorthand_unicode",
        "created_at",
        "updated_at",
    ] {
        let resp = app
            .get(&format!("/api/v1/authors?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "Author column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_author_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/authors?sort_by=name_unicode").await;
    assert_eq!(
        resp.status(),
        400,
        "Author has no name_unicode; sort should be rejected"
    );
}

// ============================================================================
// JOURNALS
// ============================================================================

#[tokio::test]
async fn test_journal_sort_accepts_all_columns() {
    let app = TestApp::spawn().await;

    for col in [
        "journal_key",
        "name_unicode",
        "name_latex",
        "created_at",
        "updated_at",
    ] {
        let resp = app
            .get(&format!("/api/v1/journals?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "Journal column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_journal_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/journals?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_journal_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/journals?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_journal_sort_by_journal_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/journals?sort_by=journal_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// PUBLISHERS
// ============================================================================

#[tokio::test]
async fn test_publisher_sort_accepts_all_columns() {
    let app = TestApp::spawn().await;

    for col in [
        "publisher_key",
        "name_unicode",
        "name_latex",
        "default_address",
        "created_at",
        "updated_at",
    ] {
        let resp = app
            .get(&format!("/api/v1/publishers?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "Publisher column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_publisher_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/publishers?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_publisher_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/publishers?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_publisher_sort_by_publisher_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/publishers?sort_by=publisher_key&sort_dir=desc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// INSTITUTIONS
// ============================================================================

#[tokio::test]
async fn test_institution_sort_accepts_all_columns() {
    let app = TestApp::spawn().await;

    for col in [
        "institution_key",
        "name_unicode",
        "name_latex",
        "default_address",
        "created_at",
        "updated_at",
    ] {
        let resp = app
            .get(&format!("/api/v1/institutions?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "Institution column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_institution_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/institutions?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_institution_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/institutions?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_institution_sort_by_institution_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/institutions?sort_by=institution_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// SCHOOLS
// ============================================================================

#[tokio::test]
async fn test_school_sort_accepts_all_columns() {
    let app = TestApp::spawn().await;

    for col in [
        "school_key",
        "name_unicode",
        "name_latex",
        "created_at",
        "updated_at",
    ] {
        let resp = app
            .get(&format!("/api/v1/schools?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "School column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_school_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/schools?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_school_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/schools?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_school_sort_by_school_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/schools?sort_by=school_key&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// SERIES
// ============================================================================

#[tokio::test]
async fn test_series_sort_accepts_all_columns() {
    let app = TestApp::spawn().await;

    for col in [
        "series_key",
        "name_unicode",
        "name_latex",
        "created_at",
        "updated_at",
    ] {
        let resp = app
            .get(&format!("/api/v1/series?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "Series column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_series_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/series?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_series_sort_by_name_unicode() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/series?sort_by=name_unicode&sort_dir=asc")
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_series_sort_by_series_key() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/series?sort_by=series_key&sort_dir=desc")
        .await;
    assert_eq!(resp.status(), 200);
}

// ============================================================================
// BIBITEMS
// ============================================================================

async fn create_bibitem(app: &TestApp, suffix: &str, bibkey: &str, fields: serde_json::Value) {
    let mut obj = json!({
        "bibkey": format!("{bibkey}-{suffix}:2024"),
        "entry_type": "article",
        "title_latex": format!("{bibkey}-{suffix}"),
    });
    if let serde_json::Value::Object(extra) = fields {
        for (k, v) in extra {
            obj[k.clone()] = v.clone();
        }
    }
    let resp = app.post_json("/api/v1/bibitems", &obj).await;
    assert_eq!(resp.status(), 200, "Failed to create bibitem {bibkey}");
}

fn get_str_values(items: &[serde_json::Value], field: &str) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| i[field].as_str().map(String::from))
        .collect()
}

fn get_i64_values(items: &[serde_json::Value], field: &str) -> Vec<i64> {
    items.iter().filter_map(|i| i[field].as_i64()).collect()
}

#[tokio::test]
async fn test_bibitem_sort_by_bibkey_asc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for key in ["zebra", "alpha", "middle"] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=bibkey&sort_dir=asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    let keys = get_str_values(items, "bibkey");
    for w in keys.windows(2) {
        assert!(w[0] <= w[1], "Expected ascending bibkeys, got {keys:?}");
    }
}

#[tokio::test]
async fn test_bibitem_sort_by_date_year_desc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for (key, year) in [("old", 1990), ("mid", 2005), ("new", 2024)] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"date_year": year, "title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=date_year&sort_dir=desc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    let years = get_i64_values(items, "date_year");
    for w in years.windows(2) {
        assert!(w[0] >= w[1], "Expected descending years, got {years:?}");
    }
}

#[tokio::test]
async fn test_bibitem_sort_by_volume_asc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for (key, vol) in [("c", "30"), ("a", "10"), ("b", "20")] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"volume": vol, "title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=volume&sort_dir=asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    let volumes = get_str_values(items, "volume");
    for w in volumes.windows(2) {
        assert!(w[0] <= w[1], "Expected ascending volumes, got {volumes:?}");
    }
}

#[tokio::test]
async fn test_bibitem_sort_by_start_page_asc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for (key, pages) in [("c", "300--350"), ("a", "1--50"), ("b", "100--150")] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"pages": pages, "title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=start_page&sort_dir=asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    let pages = get_i64_values(items, "start_page");
    assert_eq!(pages.len(), 3);
    for w in pages.windows(2) {
        assert!(w[0] <= w[1], "Expected ascending start_page, got {pages:?}");
    }
}

#[tokio::test]
async fn test_bibitem_sort_by_number_asc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for (key, num) in [("c", "3"), ("a", "1"), ("b", "2")] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"number": num, "title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=number&sort_dir=asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);

    let numbers = get_str_values(items, "number");
    for w in numbers.windows(2) {
        assert!(w[0] <= w[1], "Expected ascending numbers, got {numbers:?}");
    }
}

#[tokio::test]
async fn test_bibitem_sort_by_volume_numeric_beats_lexicographic() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Lexicographic: "9" > "77", but numeric: 9 < 77
    for (key, vol) in [("b", "77"), ("a", "9")] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"volume": vol, "title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=volume_numeric&sort_dir=asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);

    let numerics = get_i64_values(items, "volume_numeric");
    assert_eq!(numerics, vec![9, 77], "Numeric sort should put 9 before 77");
}

#[tokio::test]
async fn test_bibitem_sort_by_number_numeric_beats_lexicographic() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // "3/4" -> number_numeric=3, "11" -> number_numeric=11
    for (key, num) in [("b", "11"), ("a", "3/4")] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"number": num, "title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=number_numeric&sort_dir=asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);

    let numerics = get_i64_values(items, "number_numeric");
    assert_eq!(numerics, vec![3, 11], "Numeric sort should put 3 before 11");
}

#[tokio::test]
async fn test_bibitem_multi_column_sort_numeric_columns() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Same volume_numeric, different number_numeric and start_page
    create_bibitem(
        &app,
        &suffix,
        "v1n2p100",
        json!({"volume": "1", "number": "2", "pages": "100--199", "title_unicode": format!("v1n2p100-{suffix}")}),
    )
    .await;
    create_bibitem(
        &app,
        &suffix,
        "v1n1p50",
        json!({"volume": "1", "number": "1", "pages": "50--99", "title_unicode": format!("v1n1p50-{suffix}")}),
    )
    .await;
    create_bibitem(
        &app,
        &suffix,
        "v2n1p10",
        json!({"volume": "2", "number": "1", "pages": "10--20", "title_unicode": format!("v2n1p10-{suffix}")}),
    )
    .await;
    create_bibitem(
        &app,
        &suffix,
        "v1n1p10",
        json!({"volume": "1", "number": "1", "pages": "10--20", "title_unicode": format!("v1n1p10-{suffix}")}),
    )
    .await;

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=volume_numeric,number_numeric,start_page&sort_dir=asc,asc,asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);

    let bibkeys: Vec<&str> = items.iter().filter_map(|i| i["bibkey"].as_str()).collect();
    assert!(
        bibkeys[0].starts_with("v1n1p10"),
        "First should be v1n1p10, got {bibkeys:?}"
    );
    assert!(
        bibkeys[1].starts_with("v1n1p50"),
        "Second should be v1n1p50, got {bibkeys:?}"
    );
    assert!(
        bibkeys[2].starts_with("v1n2p100"),
        "Third should be v1n2p100, got {bibkeys:?}"
    );
    assert!(
        bibkeys[3].starts_with("v2n1p10"),
        "Fourth should be v2n1p10, got {bibkeys:?}"
    );
}

#[tokio::test]
async fn test_bibitem_multi_column_sort_volume_number_start_page() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Same volume, different number and start_page
    create_bibitem(
        &app,
        &suffix,
        "v1n2p100",
        json!({"volume": "1", "number": "2", "pages": "100--199", "title_unicode": format!("v1n2p100-{suffix}")}),
    )
    .await;
    create_bibitem(
        &app,
        &suffix,
        "v1n1p50",
        json!({"volume": "1", "number": "1", "pages": "50--99", "title_unicode": format!("v1n1p50-{suffix}")}),
    )
    .await;
    create_bibitem(
        &app,
        &suffix,
        "v2n1p10",
        json!({"volume": "2", "number": "1", "pages": "10--20", "title_unicode": format!("v2n1p10-{suffix}")}),
    )
    .await;
    create_bibitem(
        &app,
        &suffix,
        "v1n1p10",
        json!({"volume": "1", "number": "1", "pages": "10--20", "title_unicode": format!("v1n1p10-{suffix}")}),
    )
    .await;

    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=volume,number,start_page&sort_dir=asc,asc,asc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);

    let bibkeys: Vec<&str> = items.iter().filter_map(|i| i["bibkey"].as_str()).collect();
    // Expected order: v1n1p10, v1n1p50, v1n2p100, v2n1p10
    assert!(
        bibkeys[0].starts_with("v1n1p10"),
        "First should be v1n1p10, got {bibkeys:?}"
    );
    assert!(
        bibkeys[1].starts_with("v1n1p50"),
        "Second should be v1n1p50, got {bibkeys:?}"
    );
    assert!(
        bibkeys[2].starts_with("v1n2p100"),
        "Third should be v1n2p100, got {bibkeys:?}"
    );
    assert!(
        bibkeys[3].starts_with("v2n1p10"),
        "Fourth should be v2n1p10, got {bibkeys:?}"
    );
}

#[tokio::test]
async fn test_bibitem_multi_column_sort_partial_dir_defaults_to_asc() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    for (key, year) in [("old", 1990), ("new", 2024)] {
        create_bibitem(
            &app,
            &suffix,
            key,
            json!({"date_year": year, "title_unicode": format!("{key}-{suffix}")}),
        )
        .await;
    }

    // sort_dir only specifies desc for date_year; bibkey should default to asc
    let resp = app
        .get(&format!(
            "/api/v1/bibitems?sort_by=date_year,bibkey&sort_dir=desc&search_term={suffix}"
        ))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);

    let years = get_i64_values(items, "date_year");
    assert!(
        years[0] >= years[1],
        "Expected descending years, got {years:?}"
    );
}

#[tokio::test]
async fn test_bibitem_sort_accepts_all_new_columns() {
    let app = TestApp::spawn().await;

    for col in [
        "journal_key",
        "publisher_key",
        "pubstate",
        "langid",
        "epoch",
        "entry_type",
        "title_unicode",
        "volume_numeric",
        "number_numeric",
        "created_at",
        "updated_at",
    ] {
        let resp = app
            .get(&format!("/api/v1/bibitems?sort_by={col}&sort_dir=asc"))
            .await;
        assert_eq!(
            resp.status(),
            200,
            "Column '{col}' should be accepted as sortable"
        );
    }
}

#[tokio::test]
async fn test_bibitem_multi_column_sort_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app
        .get("/api/v1/bibitems?sort_by=volume,nonexistent&sort_dir=desc,asc")
        .await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_bibitem_sort_by_invalid_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/bibitems?sort_by=nonexistent").await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_bibitem_sort_by_unsortable_column_rejected() {
    let app = TestApp::spawn().await;

    let resp = app.get("/api/v1/bibitems?sort_by=abstract_latex").await;
    assert_eq!(
        resp.status(),
        400,
        "abstract_latex is not in sortable_columns; should be rejected"
    );
}
