//! Composition layer — wires adapters, domain, and logic into the application.

pub mod state;

use hexforge::{
    Api, CorsConfig, CrudPermissions, CrudResourceConfig, OpenApiConfig, Permission, Resource,
    axum_exports::{Router, get},
};

pub use state::AppState;

use crate::adapters::auth::ApiKeyValidator;
use crate::domain::{
    Author, BibItem, CreateAuthor, CreateBibItem, CreateInstitution, CreateJournal, CreateKeyword,
    CreatePublisher, CreateSchool, CreateSeries, Institution, Journal, Keyword, Publisher, School,
    Series, UpdateAuthor, UpdateBibItem, UpdateInstitution, UpdateJournal, UpdateKeyword,
    UpdatePublisher, UpdateSchool, UpdateSeries, create_author_transform,
    create_bib_item_transform, create_institution_transform, create_journal_transform,
    create_keyword_transform, create_publisher_transform, create_school_transform,
    create_series_transform, update_author_transform, update_bib_item_transform,
    update_institution_transform, update_journal_transform, update_keyword_transform,
    update_publisher_transform, update_school_transform, update_series_transform,
};
use crate::adapters::db::queries::{
    AuthorQuery, BibItemQuery, InstitutionQuery, JournalQuery, KeywordQuery, PublisherQuery,
    SchoolQuery, SeriesQuery,
};
use crate::adapters::handlers::{
    add_author_to_bibitem, export_bibitems, get_bibitem_authors, get_bibitem_keywords,
    get_by_bibkey, get_keyword_tree, import_bibitems, import_file, remove_author_from_bibitem,
    replace_bibitem_authors, search_bibitems, set_bibitem_keywords,
};
use crate::logic::validation::{
    validate_create_author, validate_create_bibitem, validate_create_institution,
    validate_create_journal, validate_create_keyword, validate_create_publisher,
    validate_create_school, validate_create_series, validate_update_author,
    validate_update_bibitem, validate_update_institution, validate_update_journal,
    validate_update_keyword, validate_update_publisher, validate_update_school,
    validate_update_series,
};

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// Build the complete application router.
///
/// CORS configuration is passed in from main.rs where environment
/// variables are read. The library layer doesn't read env vars.
pub fn build_app(pool: hexforge::DatabasePool, cors: CorsConfig) -> Router {
    let state = AppState::new(pool.clone());
    let validator = ApiKeyValidator::new(pool);

    let api = Api::new()
        .with_auth(validator)
        // Authors CRUD
        .crud_auto(
            CrudResourceConfig::<Author, _, CreateAuthor, UpdateAuthor, AuthorQuery>::new(
                "/api/v1/authors",
                state.author_ds.clone(),
            )
            .tag("Authors")
            .description("Author management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_author)
            .update_validator(validate_update_author)
            .create_transform(create_author_transform)
            .update_transform(update_author_transform)
            .by_key("author_key"),
        )
        // Journals CRUD
        .crud_auto(
            CrudResourceConfig::<Journal, _, CreateJournal, UpdateJournal, JournalQuery>::new(
                "/api/v1/journals",
                state.journal_ds.clone(),
            )
            .tag("Journals")
            .description("Journal management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_journal)
            .update_validator(validate_update_journal)
            .create_transform(create_journal_transform)
            .update_transform(update_journal_transform)
            .by_key("journal_key"),
        )
        // Publishers CRUD
        .crud_auto(
            CrudResourceConfig::<Publisher, _, CreatePublisher, UpdatePublisher, PublisherQuery>::new(
                "/api/v1/publishers",
                state.publisher_ds.clone(),
            )
            .tag("Publishers")
            .description("Publisher management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_publisher)
            .update_validator(validate_update_publisher)
            .create_transform(create_publisher_transform)
            .update_transform(update_publisher_transform)
            .by_key("publisher_key"),
        )
        // Institutions CRUD
        .crud_auto(
            CrudResourceConfig::<Institution, _, CreateInstitution, UpdateInstitution, InstitutionQuery>::new(
                "/api/v1/institutions",
                state.institution_ds.clone(),
            )
            .tag("Institutions")
            .description("Institution management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_institution)
            .update_validator(validate_update_institution)
            .create_transform(create_institution_transform)
            .update_transform(update_institution_transform)
            .by_key("institution_key"),
        )
        // Schools CRUD
        .crud_auto(
            CrudResourceConfig::<School, _, CreateSchool, UpdateSchool, SchoolQuery>::new(
                "/api/v1/schools",
                state.school_ds.clone(),
            )
            .tag("Schools")
            .description("School management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_school)
            .update_validator(validate_update_school)
            .create_transform(create_school_transform)
            .update_transform(update_school_transform)
            .by_key("school_key"),
        )
        // Series CRUD
        .crud_auto(
            CrudResourceConfig::<Series, _, CreateSeries, UpdateSeries, SeriesQuery>::new(
                "/api/v1/series",
                state.series_ds.clone(),
            )
            .tag("Series")
            .description("Series management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_series)
            .update_validator(validate_update_series)
            .create_transform(create_series_transform)
            .update_transform(update_series_transform)
            .by_key("series_key"),
        )
        // Keywords CRUD
        .crud_auto(
            CrudResourceConfig::<Keyword, _, CreateKeyword, UpdateKeyword, KeywordQuery>::new(
                "/api/v1/keywords",
                state.keyword_ds.clone(),
            )
            .tag("Keywords")
            .description("Keyword management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_keyword)
            .update_validator(validate_update_keyword)
            .create_transform(create_keyword_transform)
            .update_transform(update_keyword_transform),
        )
        // BibItems CRUD
        .crud_auto(
            CrudResourceConfig::<BibItem, _, CreateBibItem, UpdateBibItem, BibItemQuery>::new(
                "/api/v1/bibitems",
                state.bibitem_ds.clone(),
            )
            .tag("BibItems")
            .description("Bibliography item management")
            .permissions(CrudPermissions::standard())
            .create_validator(validate_create_bibitem)
            .update_validator(validate_update_bibitem)
            .create_transform(create_bib_item_transform)
            .update_transform(update_bib_item_transform),
        )
        // =====================================================================
        // Custom handlers (non-CRUD)
        // =====================================================================
        // Bibkey lookup
        .resource(
            Resource::<AppState>::new("/api/v1/bibitems")
                .get("/by-bibkey/{bibkey}", get_by_bibkey)
                .with_state(state.clone()),
        )
        // Junction tables
        .resource(
            Resource::<AppState>::new("/api/v1/bibitems")
                .get("/{id}/authors", get_bibitem_authors)
                .post("/{id}/authors", add_author_to_bibitem)
                .delete("/{id}/authors/{author_id}", remove_author_from_bibitem)
                .put("/{id}/authors", replace_bibitem_authors)
                .get("/{id}/keywords", get_bibitem_keywords)
                .post("/{id}/keywords", set_bibitem_keywords)
                .with_state(state.clone()),
        )
        // Keyword tree
        .resource(
            Resource::<AppState>::new("/api/v1/keywords")
                .get("/tree", get_keyword_tree)
                .with_state(state.clone()),
        )
        // Search
        .resource(
            Resource::<AppState>::new("/api/v1")
                .post("/search", search_bibitems)
                .with_state(state.clone()),
        )
        // Admin endpoints
        .resource(
            Resource::<AppState>::new("/api/v1/admin")
                .require_permission(Permission::Admin)
                .post("/import", import_bibitems)
                .post("/import/file", import_file)
                .get("/export", export_bibitems)
                .with_state(state),
        )
        // OpenAPI
        .serve_openapi(
            "/docs/openapi.json",
            OpenApiConfig::new("Alexandria Nexus", "0.1.0")
                .description("Bibliography and knowledge engine for Philosophie.ch"),
        )
        .serve_swagger_ui("/docs", "/docs/openapi.json")
        .with_cors(cors)
        .build();

    Router::new().route("/health", get(health)).merge(api)
}
