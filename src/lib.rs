//! Alexandria Nexus — Bibliography and knowledge engine for Philosophie.ch

use hexforge::{
    Api, CorsConfig, CrudOpenApiMeta, CrudResourceConfig, OpenApiConfig, Permission, Resource,
    axum_exports::{Router, get},
};

pub mod auth;
pub mod domain;
pub mod dto;
pub mod entities;
pub mod handlers;
pub mod projections;
pub mod queries;
mod state;
pub mod transform;
pub mod validation;

pub use state::AppState;

use auth::ApiKeyValidator;
use dto::{
    CreateAuthor, CreateBibItem, CreateInstitution, CreateJournal, CreateKeyword, CreatePublisher,
    CreateSchool, CreateSeries, UpdateAuthor, UpdateBibItem, UpdateInstitution, UpdateJournal,
    UpdateKeyword, UpdatePublisher, UpdateSchool, UpdateSeries,
};
use entities::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};
use handlers::{
    add_author_to_bibitem, export_bibitems, get_author_by_key, get_bibitem_authors,
    get_bibitem_keywords, get_by_bibkey, get_institution_by_key, get_journal_by_key,
    get_keyword_tree, get_publisher_by_key, get_school_by_key, get_series_by_key, import_bibitems,
    import_file, remove_author_from_bibitem, replace_bibitem_authors, search_bibitems,
    set_bibitem_keywords,
};
use queries::{
    AuthorQuery, BibItemQuery, InstitutionQuery, JournalQuery, KeywordQuery, PublisherQuery,
    SchoolQuery, SeriesQuery,
};
use transform::{
    create_author_transform, create_bibitem_transform, create_institution_transform,
    create_journal_transform, create_keyword_transform, create_publisher_transform,
    create_school_transform, create_series_transform, update_author_transform,
    update_bibitem_transform, update_institution_transform, update_journal_transform,
    update_keyword_transform, update_publisher_transform, update_school_transform,
    update_series_transform,
};
use validation::{
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
        .crud_with_meta(
            CrudResourceConfig::<Author, _, CreateAuthor, UpdateAuthor, AuthorQuery>::new(
                "/api/v1/authors",
                state.author_ds.clone(),
            )
            .create_validator(validate_create_author)
            .update_validator(validate_update_author)
            .create_transform(create_author_transform)
            .update_transform(update_author_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/authors", "Authors")
                .with_schemas::<Author, CreateAuthor, UpdateAuthor>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("Author management"),
        )
        // Journals CRUD
        .crud_with_meta(
            CrudResourceConfig::<Journal, _, CreateJournal, UpdateJournal, JournalQuery>::new(
                "/api/v1/journals",
                state.journal_ds.clone(),
            )
            .create_validator(validate_create_journal)
            .update_validator(validate_update_journal)
            .create_transform(create_journal_transform)
            .update_transform(update_journal_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/journals", "Journals")
                .with_schemas::<Journal, CreateJournal, UpdateJournal>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("Journal management"),
        )
        // Publishers CRUD
        .crud_with_meta(
            CrudResourceConfig::<Publisher, _, CreatePublisher, UpdatePublisher, PublisherQuery>::new(
                "/api/v1/publishers",
                state.publisher_ds.clone(),
            )
            .create_validator(validate_create_publisher)
            .update_validator(validate_update_publisher)
            .create_transform(create_publisher_transform)
            .update_transform(update_publisher_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/publishers", "Publishers")
                .with_schemas::<Publisher, CreatePublisher, UpdatePublisher>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("Publisher management"),
        )
        // Institutions CRUD
        .crud_with_meta(
            CrudResourceConfig::<Institution, _, CreateInstitution, UpdateInstitution, InstitutionQuery>::new(
                "/api/v1/institutions",
                state.institution_ds.clone(),
            )
            .create_validator(validate_create_institution)
            .update_validator(validate_update_institution)
            .create_transform(create_institution_transform)
            .update_transform(update_institution_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/institutions", "Institutions")
                .with_schemas::<Institution, CreateInstitution, UpdateInstitution>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("Institution management"),
        )
        // Schools CRUD
        .crud_with_meta(
            CrudResourceConfig::<School, _, CreateSchool, UpdateSchool, SchoolQuery>::new(
                "/api/v1/schools",
                state.school_ds.clone(),
            )
            .create_validator(validate_create_school)
            .update_validator(validate_update_school)
            .create_transform(create_school_transform)
            .update_transform(update_school_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/schools", "Schools")
                .with_schemas::<School, CreateSchool, UpdateSchool>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("School management"),
        )
        // Series CRUD
        .crud_with_meta(
            CrudResourceConfig::<Series, _, CreateSeries, UpdateSeries, SeriesQuery>::new(
                "/api/v1/series",
                state.series_ds.clone(),
            )
            .create_validator(validate_create_series)
            .update_validator(validate_update_series)
            .create_transform(create_series_transform)
            .update_transform(update_series_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/series", "Series")
                .with_schemas::<Series, CreateSeries, UpdateSeries>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("Series management"),
        )
        // Keywords CRUD
        .crud_with_meta(
            CrudResourceConfig::<Keyword, _, CreateKeyword, UpdateKeyword, KeywordQuery>::new(
                "/api/v1/keywords",
                state.keyword_ds.clone(),
            )
            .create_validator(validate_create_keyword)
            .update_validator(validate_update_keyword)
            .create_transform(create_keyword_transform)
            .update_transform(update_keyword_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/keywords", "Keywords")
                .with_schemas::<Keyword, CreateKeyword, UpdateKeyword>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("Keyword management"),
        )
        // BibItems CRUD
        .crud_with_meta(
            CrudResourceConfig::<BibItem, _, CreateBibItem, UpdateBibItem, BibItemQuery>::new(
                "/api/v1/bibitems",
                state.bibitem_ds.clone(),
            )
            .create_validator(validate_create_bibitem)
            .update_validator(validate_update_bibitem)
            .create_transform(create_bibitem_transform)
            .update_transform(update_bibitem_transform)
            .read_permission(Permission::Read)
            .create_permission(Permission::Write)
            .update_permission(Permission::Write)
            .delete_permission(Permission::Admin),
            CrudOpenApiMeta::new("/api/v1/bibitems", "BibItems")
                .with_schemas::<BibItem, CreateBibItem, UpdateBibItem>()
                .with_permissions(
                    Permission::Read,
                    Permission::Read,
                    Permission::Write,
                    Permission::Write,
                    Permission::Admin,
                )
                .description("Bibliography item management"),
        )
        // =====================================================================
        // Custom handlers (non-CRUD)
        // =====================================================================
        // By-key lookups
        .resource(
            Resource::<AppState>::new("/api/v1/authors")
                .get("/by-key/{key}", get_author_by_key)
                .with_state(state.clone()),
        )
        .resource(
            Resource::<AppState>::new("/api/v1/journals")
                .get("/by-key/{key}", get_journal_by_key)
                .with_state(state.clone()),
        )
        .resource(
            Resource::<AppState>::new("/api/v1/publishers")
                .get("/by-key/{key}", get_publisher_by_key)
                .with_state(state.clone()),
        )
        .resource(
            Resource::<AppState>::new("/api/v1/institutions")
                .get("/by-key/{key}", get_institution_by_key)
                .with_state(state.clone()),
        )
        .resource(
            Resource::<AppState>::new("/api/v1/schools")
                .get("/by-key/{key}", get_school_by_key)
                .with_state(state.clone()),
        )
        .resource(
            Resource::<AppState>::new("/api/v1/series")
                .get("/by-key/{key}", get_series_by_key)
                .with_state(state.clone()),
        )
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
        // OpenAPI spec
        .serve_openapi(
            "/docs/openapi.json",
            OpenApiConfig::new("Alexandria Nexus", "0.1.0")
                .description("Bibliography and knowledge engine for Philosophie.ch"),
        )
        .with_cors(cors)
        .build();

    Router::new().route("/health", get(health)).merge(api)
}
