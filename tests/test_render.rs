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
        html.contains("class=\"smallcaps\"") && html.contains(">Smith</span>"),
        "author in smallcaps: {html}"
    );
    assert!(
        html.contains(&format!("data-author-key=\"{author_key}\"")),
        "author key present: {html}"
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

    // Render without include_further_refs: further_refs_html must be null.
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
// Test: all bibkeys missing returns 200 with empty HTML
// =============================================================================

#[tokio::test]
async fn test_render_all_bibkeys_missing_returns_empty_html() {
    let app = TestApp::spawn().await;

    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": ["nonexistent:2024", "also-missing:2025"] }),
        )
        .await;
    assert_eq!(
        resp.status(),
        200,
        "Should return 200 even when all bibkeys missing"
    );

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["main_html"], "", "main_html should be empty");
    assert!(
        body["further_refs_html"].is_null(),
        "further_refs_html should be absent"
    );

    let missing = body["missing_bibkeys"].as_array().unwrap();
    assert_eq!(missing.len(), 2, "Should report all missing bibkeys");
    assert!(
        body["missing_ids"].is_null(),
        "missing_ids should be absent for bibkey selection"
    );
}

// =============================================================================
// Test: partial bibkeys: renders found, reports missing
// =============================================================================

#[tokio::test]
async fn test_render_partial_bibkeys_returns_found_with_missing() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    let (_, author_key) = create_author(&app, &suffix, "partial-author", "Alice", "Walker").await;
    let bibitem_id = create_bibitem_with_details(
        &app,
        &suffix,
        "partial-found",
        "article",
        "Found Article",
        Some(2024),
    )
    .await;
    link_author(&app, bibitem_id, &author_key, "author", 1).await;

    let found_key = format!("partial-found-{suffix}");
    let missing_key = "does-not-exist:2099";

    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [found_key, missing_key] }),
        )
        .await;
    assert_eq!(resp.status(), 200, "Should return 200 for partial results");

    let body: serde_json::Value = resp.json().await.unwrap();
    let html = body["main_html"].as_str().unwrap();
    assert!(
        html.contains(&format!("data-bibkey=\"{found_key}\"")),
        "HTML should contain the found bibitem"
    );

    let missing = body["missing_bibkeys"].as_array().unwrap();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0], missing_key);
}

// =============================================================================
// Test: all bibkeys found: missing_bibkeys is empty
// =============================================================================

#[tokio::test]
async fn test_render_all_bibkeys_found_has_empty_missing() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    create_bibitem_with_details(&app, &suffix, "all-found", "book", "A Book", Some(2024)).await;
    let bibkey = format!("all-found-{suffix}");

    let resp = app
        .post_json("/api/v1/render", &json!({ "bibkeys": [bibkey] }))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["main_html"].as_str().unwrap().contains("A Book"));

    let missing = body["missing_bibkeys"].as_array().unwrap();
    assert!(
        missing.is_empty(),
        "missing_bibkeys should be empty when all found"
    );
}

// =============================================================================
// Test: note citations to external bibkeys resolve (not in render set)
// =============================================================================

#[tokio::test]
async fn test_render_note_cites_external_bibkey_resolves() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create bibitem B (the cited target, NOT in the render request)
    let (_, author_b_key) =
        create_author(&app, &suffix, "ext-cited-author", "David", "Lewis").await;
    let b_id = create_bibitem_with_details(
        &app,
        &suffix,
        "ext-cited",
        "book",
        "Philosophical Papers",
        Some(1986),
    )
    .await;
    link_author(&app, b_id, &author_b_key, "author", 1).await;

    // Create bibitem A whose note_latex cites B
    let b_key = format!("ext-cited-{suffix}");
    let (_, author_a_key) =
        create_author(&app, &suffix, "ext-source-author", "Jane", "Smith").await;
    let a_payload = json!({
        "bibkey": format!("ext-source-{suffix}"),
        "entry_type": "article",
        "title_latex": "Some Article",
        "title_unicode": "Some Article",
        "title_simplified": "some article",
        "note_latex": format!("Reprinted in \\citet{{{b_key}}}"),
        "date_year": 2020
    });
    let resp = app.post_json("/api/v1/bibitems", &a_payload).await;
    assert_eq!(resp.status(), 200);
    let a_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, a_id, &author_a_key, "author", 1).await;

    // Render ONLY A (B is not in the render request)
    let a_key = format!("ext-source-{suffix}");
    let resp = app
        .post_json("/api/v1/render", &json!({ "bibkeys": [a_key] }))
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let html = body["main_html"].as_str().unwrap();

    assert!(
        html.contains("Lewis (1986)"),
        "external citation should resolve to author+year: {html}"
    );
    assert!(
        html.contains(&format!("data-bibkey=\"{b_key}\"")),
        "resolved citation should be wrapped with data-bibkey: {html}"
    );

    // B should appear as a further ref (externally-cited items are added to further refs)
    let further = body["further_refs_html"]
        .as_str()
        .expect("further_refs_html should be present for externally-cited items");
    assert!(
        further.contains(&format!("data-bibkey=\"{b_key}\"")),
        "further refs should contain the externally-cited bibitem: {further}"
    );
}

// =============================================================================
// Test: include_further_refs=false suppresses junction deps, inline cites still resolve
// =============================================================================

#[tokio::test]
async fn test_render_further_refs_false_suppresses_junction_but_allows_inline_cites() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Create three bibitems:
    //   A (main) — has junction dep → B, and note_latex cites C
    //   B (junction dep only)
    //   C (inline-cited only)
    let (_, author_key) = create_author(&app, &s, "contract-author", "Alice", "Tester").await;

    let b_id = create_bibitem_with_details(
        &app,
        &s,
        "contract-b",
        "book",
        "Junction Target",
        Some(2010),
    )
    .await;
    link_author(&app, b_id, &author_key, "author", 1).await;

    let c_id =
        create_bibitem_with_details(&app, &s, "contract-c", "book", "Cited Target", Some(2015))
            .await;
    link_author(&app, c_id, &author_key, "author", 1).await;

    let c_key = format!("contract-c-{s}");
    let a_payload = json!({
        "bibkey": format!("contract-a-{s}"),
        "entry_type": "article",
        "title_latex": "Main Article",
        "title_unicode": "Main Article",
        "title_simplified": "main article",
        "note_latex": format!("See \\citet{{{c_key}}}"),
        "date_year": 2020
    });
    let resp = app.post_json("/api/v1/bibitems", &a_payload).await;
    assert_eq!(resp.status(), 200);
    let a_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, a_id, &author_key, "author", 1).await;

    // Set up junction dep A→B
    let a_key = format!("contract-a-{s}");
    let b_key = format!("contract-b-{s}");
    let refs_csv = format!("source_key,target_key,ref_type\n{a_key},{b_key},further_ref");
    let refs_resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &refs_csv).await;
    assert_eq!(refs_resp.status(), 200);

    let rc_resp = app
        .client
        .post(app.url("/api/v1/admin/recompute-deps"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(rc_resp.status(), 200);

    // Render with include_further_refs: false (the default)
    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [a_key], "include_further_refs": false }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let further = body["further_refs_html"]
        .as_str()
        .expect("further_refs_html should be present from inline citation");

    // C (inline-cited) SHOULD appear
    assert!(
        further.contains(&format!("data-bibkey=\"{c_key}\"")),
        "inline-cited item C should appear in further_refs_html: {further}"
    );

    // B (junction dep) should NOT appear
    assert!(
        !further.contains(&format!("data-bibkey=\"{b_key}\"")),
        "junction dep B should be suppressed when include_further_refs=false: {further}"
    );
}

// =============================================================================
// Test: include_further_refs=true includes both junction deps and inline cites
// =============================================================================

#[tokio::test]
async fn test_render_further_refs_true_includes_junction_and_inline_cites() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Same setup: A has junction dep → B, and note_latex cites C
    let (_, author_key) = create_author(&app, &s, "both-author", "Alice", "Tester").await;

    let b_id =
        create_bibitem_with_details(&app, &s, "both-b", "book", "Junction Target", Some(2010))
            .await;
    link_author(&app, b_id, &author_key, "author", 1).await;

    let c_id =
        create_bibitem_with_details(&app, &s, "both-c", "book", "Cited Target", Some(2015)).await;
    link_author(&app, c_id, &author_key, "author", 1).await;

    let c_key = format!("both-c-{s}");
    let a_payload = json!({
        "bibkey": format!("both-a-{s}"),
        "entry_type": "article",
        "title_latex": "Main Article",
        "title_unicode": "Main Article",
        "title_simplified": "main article",
        "note_latex": format!("See \\citet{{{c_key}}}"),
        "date_year": 2020
    });
    let resp = app.post_json("/api/v1/bibitems", &a_payload).await;
    assert_eq!(resp.status(), 200);
    let a_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, a_id, &author_key, "author", 1).await;

    // Junction dep A→B
    let a_key = format!("both-a-{s}");
    let b_key = format!("both-b-{s}");
    let refs_csv = format!("source_key,target_key,ref_type\n{a_key},{b_key},further_ref");
    let refs_resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &refs_csv).await;
    assert_eq!(refs_resp.status(), 200);

    let rc_resp = app
        .client
        .post(app.url("/api/v1/admin/recompute-deps"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(rc_resp.status(), 200);

    // Render with include_further_refs: true
    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [a_key], "include_further_refs": true }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let further = body["further_refs_html"]
        .as_str()
        .expect("further_refs_html should be present");

    // Both B (junction dep) and C (inline-cited) should appear
    assert!(
        further.contains(&format!("data-bibkey=\"{b_key}\"")),
        "junction dep B should appear when include_further_refs=true: {further}"
    );
    assert!(
        further.contains(&format!("data-bibkey=\"{c_key}\"")),
        "inline-cited item C should also appear: {further}"
    );
}

// =============================================================================
// Test: further-ref inline citations also resolve when include_further_refs=true
// =============================================================================

#[tokio::test]
async fn test_render_further_ref_inline_cites_also_resolve() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // A (main) → junction dep B → B's note cites C
    // With include_further_refs=true, C should also appear
    let (_, author_key) = create_author(&app, &s, "chain-author", "Bob", "Chainer").await;

    let c_id =
        create_bibitem_with_details(&app, &s, "chain-c", "book", "Deep Target", Some(2005)).await;
    link_author(&app, c_id, &author_key, "author", 1).await;

    let c_key = format!("chain-c-{s}");
    let b_payload = json!({
        "bibkey": format!("chain-b-{s}"),
        "entry_type": "incollection",
        "title_latex": "Middle Entry",
        "title_unicode": "Middle Entry",
        "title_simplified": "middle entry",
        "note_latex": format!("Reprinted in \\citet{{{c_key}}}"),
        "date_year": 2010
    });
    let resp = app.post_json("/api/v1/bibitems", &b_payload).await;
    assert_eq!(resp.status(), 200);
    let b_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, b_id, &author_key, "author", 1).await;

    let a_id =
        create_bibitem_with_details(&app, &s, "chain-a", "article", "Top Article", Some(2020))
            .await;
    link_author(&app, a_id, &author_key, "author", 1).await;

    // Junction dep A→B
    let a_key = format!("chain-a-{s}");
    let b_key = format!("chain-b-{s}");
    let refs_csv = format!("source_key,target_key,ref_type\n{a_key},{b_key},further_ref");
    let refs_resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &refs_csv).await;
    assert_eq!(refs_resp.status(), 200);

    let rc_resp = app
        .client
        .post(app.url("/api/v1/admin/recompute-deps"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(rc_resp.status(), 200);

    // Render A with include_further_refs: true
    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [a_key], "include_further_refs": true }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let further = body["further_refs_html"]
        .as_str()
        .expect("further_refs_html should be present");

    // B (junction dep) should appear
    assert!(
        further.contains(&format!("data-bibkey=\"{b_key}\"")),
        "junction dep B should be in further refs: {further}"
    );

    // C (cited inside B's note) should also appear because further_items are scanned
    assert!(
        further.contains(&format!("data-bibkey=\"{c_key}\"")),
        "C cited in B's note should also resolve via further-ref scanning: {further}"
    );

    // B's rendered note should contain the resolved citation to C
    assert!(
        further.contains("Chainer (2005)"),
        "B's note should show resolved citation to C: {further}"
    );
}

// =============================================================================
// Test: further refs are sorted by author/year/bibkey after external merge
// =============================================================================

#[tokio::test]
async fn test_render_further_refs_sorted_after_external_merge() {
    let app = TestApp::spawn().await;
    let s = unique_suffix();

    // Create two authors (Z and A alphabetically) to verify sort order
    let (_, author_z_key) = create_author(&app, &s, "sort-z-author", "Zara", "Zulu").await;
    let (_, author_a_key) = create_author(&app, &s, "sort-a-author", "Adam", "Alpha").await;

    // B (junction dep, author=Zulu)
    let b_id =
        create_bibitem_with_details(&app, &s, "sort-b", "book", "Zulu Book", Some(2010)).await;
    link_author(&app, b_id, &author_z_key, "author", 1).await;

    // C (inline-cited, author=Alpha — should sort BEFORE Zulu)
    let c_id =
        create_bibitem_with_details(&app, &s, "sort-c", "book", "Alpha Book", Some(2015)).await;
    link_author(&app, c_id, &author_a_key, "author", 1).await;

    let c_key = format!("sort-c-{s}");
    let (_, author_main_key) = create_author(&app, &s, "sort-main-author", "Main", "Author").await;
    let a_payload = json!({
        "bibkey": format!("sort-a-{s}"),
        "entry_type": "article",
        "title_latex": "Main Article",
        "title_unicode": "Main Article",
        "title_simplified": "main article",
        "note_latex": format!("See \\citet{{{c_key}}}"),
        "date_year": 2020
    });
    let resp = app.post_json("/api/v1/bibitems", &a_payload).await;
    assert_eq!(resp.status(), 200);
    let a_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, a_id, &author_main_key, "author", 1).await;

    // Junction dep A→B
    let a_key = format!("sort-a-{s}");
    let b_key = format!("sort-b-{s}");
    let refs_csv = format!("source_key,target_key,ref_type\n{a_key},{b_key},further_ref");
    let refs_resp = upload_csv(&app, "/api/v1/admin/import/bibitem-refs", &refs_csv).await;
    assert_eq!(refs_resp.status(), 200);

    let rc_resp = app
        .client
        .post(app.url("/api/v1/admin/recompute-deps"))
        .header("Authorization", format!("Bearer {}", app.api_key))
        .send()
        .await
        .unwrap();
    assert_eq!(rc_resp.status(), 200);

    // Render with include_further_refs: true (both junction + inline in further refs)
    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [a_key], "include_further_refs": true }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let further = body["further_refs_html"]
        .as_str()
        .expect("further_refs_html should contain both items");

    // Alpha (C) should appear before Zulu (B) in the HTML
    let pos_alpha = further
        .find(&format!("data-bibkey=\"{c_key}\""))
        .expect("C (Alpha) should be in further refs");
    let pos_zulu = further
        .find(&format!("data-bibkey=\"{b_key}\""))
        .expect("B (Zulu) should be in further refs");
    assert!(
        pos_alpha < pos_zulu,
        "Alpha should sort before Zulu in further refs: {further}"
    );
}

// =============================================================================
// Test: external citations get year suffixes + postnotes preserved
// =============================================================================

#[tokio::test]
async fn test_render_external_citations_year_suffix_and_postnote() {
    let app = TestApp::spawn().await;
    let suffix = unique_suffix();

    // Create author (shared by all Lewis entries)
    let (_, author_key) = create_author(&app, &suffix, "suffix-author", "David", "Lewis").await;

    // Create three Lewis 1986 entries:
    //   A (incollection, in the render request) — lewis:1986i (postscript to 1973)
    //   B (book, NOT in render request) — lewis:1986a (Philosophical Papers)
    //   C (incollection, NOT in render request) — lewis:1986j (postscript to 1979)
    let a_bibkey = format!("lewis:1986i-{suffix}");
    let a_payload = json!({
        "bibkey": &a_bibkey,
        "entry_type": "incollection",
        "title_latex": "Postscript",
        "title_unicode": "Postscript",
        "title_simplified": "postscript",
        "date_year": 1986
    });
    let resp = app.post_json("/api/v1/bibitems", &a_payload).await;
    assert_eq!(resp.status(), 200);
    let a_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, a_id, &author_key, "author", 1).await;

    let b_bibkey = format!("lewis:1986a-{suffix}");
    let b_payload = json!({
        "bibkey": &b_bibkey,
        "entry_type": "book",
        "title_latex": "Philosophical Papers",
        "title_unicode": "Philosophical Papers",
        "title_simplified": "philosophical papers",
        "date_year": 1986
    });
    let resp = app.post_json("/api/v1/bibitems", &b_payload).await;
    assert_eq!(resp.status(), 200);
    let b_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, b_id, &author_key, "author", 1).await;

    let c_bibkey = format!("lewis:1986j-{suffix}");
    let c_payload = json!({
        "bibkey": &c_bibkey,
        "entry_type": "incollection",
        "title_latex": "Postscript to 1979",
        "title_unicode": "Postscript to 1979",
        "title_simplified": "postscript to 1979",
        "date_year": 1986
    });
    let resp = app.post_json("/api/v1/bibitems", &c_payload).await;
    assert_eq!(resp.status(), 200);
    let c_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, c_id, &author_key, "author", 1).await;

    // Create the citing article whose note_latex references B and C with postnotes
    let citing_bibkey = format!("lewis:1973b-{suffix}");
    let citing_payload = json!({
        "bibkey": &citing_bibkey,
        "entry_type": "article",
        "title_latex": "Causation",
        "title_unicode": "Causation",
        "title_simplified": "causation",
        "note_latex": format!("Reprinted in \\citep{{{a_bibkey}}}, \\citet[32--51]{{{b_bibkey}}}"),
        "date_year": 1973
    });
    let resp = app.post_json("/api/v1/bibitems", &citing_payload).await;
    assert_eq!(resp.status(), 200);
    let citing_id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_i64()
        .unwrap();
    link_author(&app, citing_id, &author_key, "author", 1).await;

    // Render citing article + A (two Lewis entries in main, B is external)
    let resp = app
        .post_json(
            "/api/v1/render",
            &json!({ "bibkeys": [citing_bibkey, a_bibkey] }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let html = body["main_html"].as_str().unwrap();

    // A (main, bibkey suffix "i") gets rendered suffix "a" (main items first)
    assert!(
        html.contains("1986a"),
        "main item lewis:1986i should get year suffix 'a': {html}"
    );

    // B (external, bibkey suffix "a") gets rendered suffix "b"
    // Citation with postnote: Lewis (1986b, 32–51)
    assert!(
        html.contains("1986b, 32\u{2013}51"),
        "external citation should have suffix 'b' and postnote pages: {html}"
    );

    // B should appear in further refs
    let further = body["further_refs_html"]
        .as_str()
        .expect("further_refs_html should contain externally-cited items");
    assert!(
        further.contains(&format!("data-bibkey=\"{b_bibkey}\"")),
        "B should be in further refs: {further}"
    );
}
