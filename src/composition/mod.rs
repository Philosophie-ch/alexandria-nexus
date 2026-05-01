//! Composition layer — wires adapters, domain, and logic into the application.

pub mod state;
mod wiring;

use hexforge::{
    Api, CorsConfig, CrudPermissions, CrudResourceConfig, JunctionConfig, OpenApiConfig,
    Permission, Resource,
    axum_exports::{DefaultBodyLimit, Router, get},
};

pub use state::AppState;

use crate::adapters::auth::ApiKeyValidator;
use crate::adapters::db::queries::{
    AuthorQuery, BibItemQuery, DataVersionQuery, InstitutionQuery, JournalQuery, KeywordQuery,
    PublisherQuery, SchoolQuery, SeriesQuery,
};
use crate::adapters::handlers::{
    bulk_import_table, convert_latex_columns, convert_latex_to_unicode, export_authors,
    export_bibitems, export_full_csv, export_institutions, export_journals, export_keywords,
    export_publishers, export_schools, export_series, get_keyword_tree,
    import_author_name_variants, import_authors, import_bibitem_notes, import_bibitem_refs,
    import_bibitems, import_entities_from_full_csv, import_full_csv, import_institutions,
    import_journals, import_keywords, import_publishers, import_schools, import_series,
    recompute_deps, render_bibitems, search_bibitems, snapshot_data, validate_full_csv, wipe_data,
};
use crate::domain::projections::{
    AuthorExpanded, BibItemCrossref, BibItemSummary, InstitutionExpanded, JournalExpanded,
    KeywordExpanded, PublisherExpanded, SchoolExpanded, SeriesExpanded,
};
use crate::domain::{
    Author, BibItem, CreateAuthor, CreateBibItem, CreateDataVersion, CreateInstitution,
    CreateJournal, CreateKeyword, CreatePublisher, CreateSchool, CreateSeries, DataVersion,
    Institution, Journal, Keyword, Publisher, School, Series, UpdateAuthor, UpdateBibItem,
    UpdateDataVersion, UpdateInstitution, UpdateJournal, UpdateKeyword, UpdatePublisher,
    UpdateSchool, UpdateSeries, create_author_transform, create_bib_item_transform,
    create_data_version_transform, create_institution_transform, create_journal_transform,
    create_keyword_transform, create_publisher_transform, create_school_transform,
    create_series_transform, update_author_transform, update_bib_item_transform,
    update_institution_transform, update_journal_transform, update_keyword_transform,
    update_publisher_transform, update_school_transform, update_series_transform,
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
pub fn build_app(
    pool: hexforge::DatabasePool,
    cors: CorsConfig,
    max_body_size_mb: usize,
) -> Router {
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
            .lookup_by("author_key"),
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
            .lookup_by("journal_key"),
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
            .lookup_by("publisher_key"),
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
            .lookup_by("institution_key"),
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
            .lookup_by("school_key"),
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
            .lookup_by("series_key"),
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
            .update_transform(update_keyword_transform)
            .lookup_by("keyword_key"),
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
            .update_transform(update_bib_item_transform)
            .lookup_by("bibkey")
            // Projection view for list endpoint: ?view=summary
            .view("summary", hexforge::build_projection_view::<_, _, BibItemSummary>(state.bibitem_ds.clone()))
            // Junction tables (many-to-many)
            .junction(
                JunctionConfig::new("authors", "bibitem_authors", state.pool.pool().clone())
                    .local_fk("bibkey")
                    .parent_id_to_local_fk("SELECT bibkey FROM bibitems WHERE id = $1")
                    .foreign_fk("author_key")
                    .foreign_fk_text()
                    .extra_columns_typed(&[("role", Some("author_role")), ("position", None)]),
            )
            .junction(
                JunctionConfig::new("keywords", "bibitem_keywords", state.pool.pool().clone())
                    .local_fk("bibkey")
                    .parent_id_to_local_fk("SELECT bibkey FROM bibitems WHERE id = $1")
                    .foreign_fk("keyword_key")
                    .foreign_fk_text()
                    .extra_columns(&["keyword_level"]),
            )
            // FK expansions — projected (no timestamps, only what's needed)
            .expand_fk_projected_by_key::<JournalExpanded, _, _>("journal", "journal_key", "journal_key", state.journal_ds.clone())
            .expand_fk_projected_by_key::<PublisherExpanded, _, _>("publisher", "publisher_key", "publisher_key", state.publisher_ds.clone())
            .expand_fk_projected_by_key::<InstitutionExpanded, _, _>("institution", "institution_key", "institution_key", state.institution_ds.clone())
            .expand_fk_projected_by_key::<SchoolExpanded, _, _>("school", "school_key", "school_key", state.school_ds.clone())
            .expand_fk_projected_by_key::<SeriesExpanded, _, _>("series", "series_key", "series_key", state.series_ds.clone())
            .expand_fk_projected_by_key::<BibItemCrossref, _, _>("crossref", "crossref", "bibkey", state.bibitem_ds.clone())
            // Junction expansions — projected, with role filtering
            .expand_junction_projected_by_key::<AuthorExpanded, _, _>(
                "authors",
                "bibitem_authors",
                "bibkey",
                "bibkey",
                "author_key",
                "author_key",
                Some(("role", "author")),
                state.author_ds.clone(),
            )
            .expand_junction_projected_by_key::<AuthorExpanded, _, _>(
                "editors",
                "bibitem_authors",
                "bibkey",
                "bibkey",
                "author_key",
                "author_key",
                Some(("role", "editor")),
                state.author_ds.clone(),
            )
            .expand_junction_projected_by_key::<AuthorExpanded, _, _>(
                "guesteditors",
                "bibitem_authors",
                "bibkey",
                "bibkey",
                "author_key",
                "author_key",
                Some(("role", "guesteditor")),
                state.author_ds.clone(),
            )
            .expand_junction_projected_by_key::<KeywordExpanded, _, _>(
                "keywords",
                "bibitem_keywords",
                "bibkey",
                "bibkey",
                "keyword_key",
                "keyword_key",
                None,
                state.keyword_ds.clone(),
            ),
        )
        // =====================================================================
        // Custom handlers (non-CRUD)
        // =====================================================================
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
        // Render (HTML bibliography)
        .resource(
            Resource::<AppState>::new("/api/v1")
                .post("/render", render_bibitems)
                .with_state(state.clone()),
        )
        // Admin: Export endpoints
        .resource(
            Resource::<AppState>::new("/api/v1/admin/export")
                .require_permission(Permission::Admin)
                .post("/bibitems", export_bibitems)
                .post("/authors", export_authors)
                .post("/journals", export_journals)
                .post("/publishers", export_publishers)
                .post("/institutions", export_institutions)
                .post("/schools", export_schools)
                .post("/series", export_series)
                .post("/keywords", export_keywords)
                .with_state(state.clone()),
        )
        // Admin: Import endpoints
        .resource(
            Resource::<AppState>::new("/api/v1/admin/import")
                .require_permission(Permission::Admin)
                .post("/bibitems", import_bibitems)
                .post("/authors", import_authors)
                .post("/journals", import_journals)
                .post("/publishers", import_publishers)
                .post("/institutions", import_institutions)
                .post("/schools", import_schools)
                .post("/series", import_series)
                .post("/keywords", import_keywords)
                .post("/author-name-variants", import_author_name_variants)
                .post("/bibitem-refs", import_bibitem_refs)
                .post("/bibitem-notes", import_bibitem_notes)
                .with_state(state.clone()),
        )
        // Admin: Full CSV import endpoints
        .resource(
            Resource::<AppState>::new("/api/v1/admin")
                .require_permission(Permission::Admin)
                .post("/validate-full-csv", validate_full_csv)
                .post("/import-entities-from-full-csv", import_entities_from_full_csv)
                .post("/import-full-csv", import_full_csv)
                .post("/export-full-csv", export_full_csv)
                .post("/recompute-deps", recompute_deps)
                .post("/latex-to-unicode", convert_latex_to_unicode)
                .post("/convert-latex-columns", convert_latex_columns)
                .post("/bulk-import/{table}", bulk_import_table)
                .post("/wipe", wipe_data)
                .post("/snapshot", snapshot_data)
                .with_state(state.clone()),
        )
        // DataVersion CRUD (list is public; all mutations are admin-only)
        .crud_auto(
            CrudResourceConfig::<DataVersion, _, CreateDataVersion, UpdateDataVersion, DataVersionQuery>::new(
                "/api/v1/data-version",
                state.data_version_ds.clone(),
            )
            .create_transform(create_data_version_transform)
            .tag("DataVersion")
            .description("Data version tracking")
            .list_permission(Permission::Public)
            .get_permission(Permission::Admin)
            .create_permission(Permission::Admin)
            .update_permission(Permission::Admin)
            .delete_permission(Permission::Admin),
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

    Router::new()
        .route("/health", get(health))
        .merge(api)
        .layer(DefaultBodyLimit::max(max_body_size_mb * 1024 * 1024))
}
