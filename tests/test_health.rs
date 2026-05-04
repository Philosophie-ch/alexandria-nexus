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
async fn test_openapi_returns_json() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    // OpenAPI spec should have an "openapi" version field and "info"
    assert!(
        body.get("openapi").is_some(),
        "Missing openapi version field"
    );
    assert!(body.get("info").is_some(), "Missing info field");
}

#[tokio::test]
async fn test_openapi_includes_enum_schemas() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let schemas = body
        .pointer("/components/schemas")
        .expect("OpenAPI spec must have components.schemas");

    for name in &["EntryType", "Epoch", "LangId", "PubState"] {
        assert!(
            schemas.get(name).is_some(),
            "Missing enum schema in components.schemas: {name}"
        );
    }
}

#[tokio::test]
async fn test_openapi_custom_endpoint_schemas_in_components() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let schemas = body
        .pointer("/components/schemas")
        .expect("OpenAPI spec must have components.schemas");

    // Frontend-critical schemas
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

    // Admin endpoint schemas
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
}

#[tokio::test]
async fn test_openapi_auxiliary_schemas_in_components() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let schemas = body
        .pointer("/components/schemas")
        .expect("OpenAPI spec must have components.schemas");

    // Nested types registered via Api::extra_schemas() — not auto-discovered by impl_to_schema!
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
}

#[tokio::test]
async fn test_openapi_search_endpoint_has_request_and_response_schemas() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let request_ref = body
        .pointer("/paths/~1api~1v1~1search/post/requestBody/content/application~1json/schema/$ref");
    assert!(
        request_ref.is_some(),
        "POST /api/v1/search must have a requestBody schema $ref"
    );

    let response_ref = body.pointer(
        "/paths/~1api~1v1~1search/post/responses/200/content/application~1json/schema/$ref",
    );
    assert!(
        response_ref.is_some(),
        "POST /api/v1/search must have a 200 response schema $ref"
    );
}

#[tokio::test]
async fn test_openapi_render_endpoint_has_request_and_response_schemas() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let request_ref = body
        .pointer("/paths/~1api~1v1~1render/post/requestBody/content/application~1json/schema/$ref");
    assert!(
        request_ref.is_some(),
        "POST /api/v1/render must have a requestBody schema $ref"
    );

    let response_ref = body.pointer(
        "/paths/~1api~1v1~1render/post/responses/200/content/application~1json/schema/$ref",
    );
    assert!(
        response_ref.is_some(),
        "POST /api/v1/render must have a 200 response schema $ref"
    );
}

#[tokio::test]
async fn test_openapi_keyword_tree_has_response_schema() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let response_ref = body.pointer(
        "/paths/~1api~1v1~1keywords~1tree/get/responses/200/content/application~1json/schema/$ref",
    );
    assert!(
        response_ref.is_some(),
        "GET /api/v1/keywords/tree must have a 200 response schema $ref"
    );
}

#[tokio::test]
async fn test_openapi_admin_import_bibitems_has_response_and_error_schemas() {
    let app = TestApp::spawn().await;

    let resp = app.get_no_auth("/docs/openapi.json").await;
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();

    let success_ref = body.pointer("/paths/~1api~1v1~1admin~1import~1bibitems/post/responses/200/content/application~1json/schema/$ref");
    assert!(
        success_ref.is_some(),
        "POST /api/v1/admin/import/bibitems must have a 200 response schema"
    );

    let error_ref = body.pointer("/paths/~1api~1v1~1admin~1import~1bibitems/post/responses/422/content/application~1json/schema/$ref");
    assert!(
        error_ref.is_some(),
        "POST /api/v1/admin/import/bibitems must have a 422 error schema"
    );
}
