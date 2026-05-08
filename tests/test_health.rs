//! Health check and OpenAPI endpoint tests.

mod common;

use common::TestApp;

#[tokio::test]
async fn test_health_returns_ok() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/health").await;

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "OK");
}

#[tokio::test]
async fn test_openapi_spec() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    // --- Basic structure ---
    assert!(
        body.get("openapi").is_some(),
        "Missing openapi version field"
    );
    assert!(body.get("info").is_some(), "Missing info field");

    let schemas = body
        .pointer("/components/schemas")
        .expect("OpenAPI spec must have components.schemas");

    // --- Domain enum schemas ---
    for name in &["EntryType", "Epoch", "LangId", "PubState"] {
        assert!(
            schemas.get(name).is_some(),
            "Missing enum schema in components.schemas: {name}"
        );
    }

    // --- Frontend-critical schemas ---
    for name in &[
        "SearchRequest",
        "SearchResponse",
        "RenderRequest",
        "RenderResponseBody",
        "KeywordTreeResponse",
    ] {
        assert!(
            schemas.get(name).is_some(),
            "Missing schema in components.schemas: {name}"
        );
    }

    // --- Admin endpoint schemas ---
    for name in &[
        "ImportResponse",
        "ImportRowError",
        "MissingReferencesError",
        "ValidationReport",
        "EntityImportReport",
        "FullImportReport",
        "LatexConvertReport",
        "LatexConvertRequest",
        "LatexConvertResponse",
        "ComputeStartPagesReport",
        "BulkImportResponse",
        "WipeResponse",
        "EntityExportRequest",
        "BibitemExportRequest",
        "ExportFormat",
    ] {
        assert!(
            schemas.get(name).is_some(),
            "Missing schema in components.schemas: {name}"
        );
    }

    // --- Auxiliary/nested schemas (registered via extra_schemas) ---
    for name in &[
        "FieldError",
        "RowError",
        "AmbiguousAuthor",
        "DuplicateBibkey",
        "MissingKeywords",
        "EntityImportError",
        "NamedEntityKind",
        "ColumnConvertResult",
        "LatexConvertError",
        "LatexConvertItem",
    ] {
        assert!(
            schemas.get(name).is_some(),
            "Missing auxiliary schema in components.schemas: {name}"
        );
    }

    // --- Endpoint schema $refs ---

    // POST /api/v1/search
    assert!(
        body.pointer(
            "/paths/~1api~1v1~1search/post/requestBody/content/application~1json/schema/$ref"
        )
        .is_some(),
        "POST /api/v1/search must have a requestBody schema $ref"
    );
    assert!(
        body.pointer(
            "/paths/~1api~1v1~1search/post/responses/200/content/application~1json/schema/$ref"
        )
        .is_some(),
        "POST /api/v1/search must have a 200 response schema $ref"
    );

    // POST /api/v1/render
    assert!(
        body.pointer(
            "/paths/~1api~1v1~1render/post/requestBody/content/application~1json/schema/$ref"
        )
        .is_some(),
        "POST /api/v1/render must have a requestBody schema $ref"
    );
    assert!(
        body.pointer(
            "/paths/~1api~1v1~1render/post/responses/200/content/application~1json/schema/$ref"
        )
        .is_some(),
        "POST /api/v1/render must have a 200 response schema $ref"
    );

    // GET /api/v1/keywords/tree
    assert!(
        body.pointer("/paths/~1api~1v1~1keywords~1tree/get/responses/200/content/application~1json/schema/$ref").is_some(),
        "GET /api/v1/keywords/tree must have a 200 response schema $ref"
    );

    // POST /api/v1/admin/import/bibitems
    assert!(
        body.pointer("/paths/~1api~1v1~1admin~1import~1bibitems/post/responses/200/content/application~1json/schema/$ref").is_some(),
        "POST /api/v1/admin/import/bibitems must have a 200 response schema"
    );
    assert!(
        body.pointer("/paths/~1api~1v1~1admin~1import~1bibitems/post/responses/422/content/application~1json/schema/$ref").is_some(),
        "POST /api/v1/admin/import/bibitems must have a 422 error schema"
    );
}
