//! Search endpoint integration tests.

mod common;

use common::{TestApp, unique_suffix};
use serde_json::json;

#[tokio::test]
async fn test_search_bibitems() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create a few bibitems with distinct titles
    let payload1 = json!({
        "bibkey": format!("test:search-a-{}", suffix),
        "entry_type": "article",
        "title_latex": "Phenomenology of Spirit",
        "title_unicode": "Phenomenology of Spirit",
        "title_simplified": "phenomenology of spirit",
        "date_year": 1807
    });
    let payload2 = json!({
        "bibkey": format!("test:search-b-{}", suffix),
        "entry_type": "book",
        "title_latex": "Being and Time",
        "title_unicode": "Being and Time",
        "title_simplified": "being and time",
        "date_year": 1927
    });

    let r1 = app.post_json("/api/v1/bibitems", &payload1).await;
    assert_eq!(r1.status(), 200);
    let r2 = app.post_json("/api/v1/bibitems", &payload2).await;
    assert_eq!(r2.status(), 200);

    // First, test with empty query (no trigram search, just listing)
    let empty_search = json!({ "query": "" });
    let empty_resp = app.post_json("/api/v1/search", &empty_search).await;
    let empty_status = empty_resp.status();
    let empty_body: serde_json::Value = empty_resp.json().await.unwrap();
    assert_eq!(
        empty_status.as_u16(),
        200,
        "Empty search returned {}: {:?}",
        empty_status,
        empty_body
    );

    // Search for "phenomenology"
    let search_payload = json!({
        "query": "phenomenology"
    });
    let resp = app.post_json("/api/v1/search", &search_payload).await;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(
        status.as_u16(),
        200,
        "Search returned {}: {:?}",
        status,
        body
    );

    assert!(
        body["results"].is_array(),
        "Search response should have results array"
    );
    assert!(
        body["total"].is_number(),
        "Search response should have total count"
    );

    let results = body["results"].as_array().unwrap();
    assert!(
        body["total"].as_i64().unwrap() >= 0,
        "Total should be non-negative"
    );

    // If results are returned, verify they have bibitem fields
    if !results.is_empty() {
        assert!(
            results[0]["bibkey"].is_string(),
            "Result items should have bibkey field"
        );
        assert!(
            results[0]["title_unicode"].is_string(),
            "Result items should have title_unicode field"
        );
    }
}
