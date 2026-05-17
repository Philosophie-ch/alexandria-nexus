//! Composition layer — wires adapters, domain, and logic into the application.

pub mod state;
mod wiring;

use hexforge::{
    Api, CorsConfig, CrudPermissions, CrudResourceConfig, Endpoint, JunctionConfig, OpenApiConfig,
    Permission, Resource, SchemaInfo,
    axum_exports::{DefaultBodyLimit, Router, get},
};

pub use state::AppState;

use crate::adapters::auth::ApiKeyValidator;
use crate::adapters::db::queries::{
    AuthorQuery, BibItemQuery, DataVersionQuery, InstitutionQuery, JournalQuery, KeywordQuery,
    PublisherQuery, SchoolQuery, SeriesQuery,
};
use crate::adapters::handlers::{
    BulkImportResponse, KeywordTreeResponse, LatexConvertItem, LatexConvertRequest,
    LatexConvertResponse, RenderRequest, RenderResponseBody, WipeResponse, bulk_import_table,
    compute_numeric_fields_handler, convert_latex_columns, convert_latex_to_unicode,
    export_authors, export_bibitems, export_full_csv, export_institutions, export_journals,
    export_keywords, export_publishers, export_schools, export_series, get_keyword_tree,
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
use crate::logic::export::{BibitemExportRequest, EntityExportRequest, ExportFormat};
use crate::logic::full_import::{
    AmbiguousAuthor, ColumnConvertResult, DuplicateBibkey, EntityImportError, EntityImportReport,
    FieldError, FullImportReport, LatexConvertError, LatexConvertReport, MissingKeywords,
    NamedEntityKind, RowError, ValidationReport,
};
use crate::logic::import::{ImportResponse, ImportRowError, MissingReferencesError};
use crate::logic::pages::{compute_start_page, extract_leading_integer};
use crate::logic::search::{SearchRequest, SearchResponse};
use crate::logic::validation::{
    validate_create_author, validate_create_bibitem, validate_create_institution,
    validate_create_journal, validate_create_keyword, validate_create_publisher,
    validate_create_school, validate_create_series, validate_update_author,
    validate_update_bibitem, validate_update_institution, validate_update_journal,
    validate_update_keyword, validate_update_publisher, validate_update_school,
    validate_update_series,
};
use crate::process::compute_numeric_fields::ComputeNumericFieldsReport;

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

fn create_bibitem_computed(input: CreateBibItem) -> BibItem {
    let mut bib = create_bib_item_transform(input);
    bib.start_page = compute_start_page(bib.pages.as_deref());
    bib.volume_numeric = extract_leading_integer(bib.volume.as_deref());
    bib.number_numeric = extract_leading_integer(bib.number.as_deref());
    bib
}

fn update_bibitem_computed(input: UpdateBibItem, existing: BibItem) -> BibItem {
    let mut bib = update_bib_item_transform(input, existing);
    bib.start_page = compute_start_page(bib.pages.as_deref());
    bib.volume_numeric = extract_leading_integer(bib.volume.as_deref());
    bib.number_numeric = extract_leading_integer(bib.number.as_deref());
    bib
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
        .extra_schemas(vec![
            SchemaInfo::from_type::<FieldError>(),
            SchemaInfo::from_type::<RowError>(),
            SchemaInfo::from_type::<AmbiguousAuthor>(),
            SchemaInfo::from_type::<DuplicateBibkey>(),
            SchemaInfo::from_type::<MissingKeywords>(),
            SchemaInfo::from_type::<EntityImportError>(),
            SchemaInfo::from_type::<NamedEntityKind>(),
            SchemaInfo::from_type::<ColumnConvertResult>(),
            SchemaInfo::from_type::<LatexConvertError>(),
            SchemaInfo::from_type::<ImportRowError>(),
            SchemaInfo::from_type::<ExportFormat>(),
            SchemaInfo::from_type::<LatexConvertItem>(),
        ])
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
            .lookup_by("author_key")
            .sortable_columns(&[
                "author_key",
                "family_name_unicode",
                "given_name_unicode",
                "famous",
                "mononym_unicode",
                "shorthand_unicode",
                "created_at",
                "updated_at",
            ]),
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
            .lookup_by("journal_key")
            .sortable_columns(&[
                "journal_key",
                "name_unicode",
                "name_latex",
                "created_at",
                "updated_at",
            ]),
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
            .lookup_by("publisher_key")
            .sortable_columns(&[
                "publisher_key",
                "name_unicode",
                "name_latex",
                "default_address",
                "created_at",
                "updated_at",
            ]),
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
            .lookup_by("institution_key")
            .sortable_columns(&[
                "institution_key",
                "name_unicode",
                "name_latex",
                "default_address",
                "created_at",
                "updated_at",
            ]),
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
            .lookup_by("school_key")
            .sortable_columns(&[
                "school_key",
                "name_unicode",
                "name_latex",
                "created_at",
                "updated_at",
            ]),
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
            .lookup_by("series_key")
            .sortable_columns(&[
                "series_key",
                "name_unicode",
                "name_latex",
                "created_at",
                "updated_at",
            ]),
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
            .lookup_by("keyword_key")
            .sortable_columns(&[
                "keyword_key",
                "name",
                "level",
                "created_at",
                "updated_at",
            ]),
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
            .create_transform(create_bibitem_computed)
            .update_transform(update_bibitem_computed)
            .lookup_by("bibkey")
            .sortable_columns(&[
                "bibkey",
                "title_unicode",
                "entry_type",
                "date_year",
                "volume",
                "volume_numeric",
                "number",
                "number_numeric",
                "start_page",
                "journal_key",
                "publisher_key",
                "pubstate",
                "langid",
                "epoch",
                "created_at",
                "updated_at",
            ])
            .extra_schemas(vec![
                SchemaInfo::from_type::<crate::domain::EntryType>(),
                SchemaInfo::from_type::<crate::domain::Epoch>(),
                SchemaInfo::from_type::<crate::domain::LangId>(),
                SchemaInfo::from_type::<crate::domain::PubState>(),
            ])
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
                .operation(
                    Endpoint::get("/tree")
                        .response_schema::<KeywordTreeResponse>()
                        .tag("Keywords")
                        .description("Hierarchical keyword tree (levels 1–3)"),
                    get_keyword_tree,
                )
                .with_state(state.clone()),
        )
        // Search
        .resource(
            Resource::<AppState>::new("/api/v1")
                .operation(
                    Endpoint::post("/search")
                        .request_schema::<SearchRequest>()
                        .response_schema::<SearchResponse>()
                        .tag("Search")
                        .description("Full-text search with filters"),
                    search_bibitems,
                )
                .with_state(state.clone()),
        )
        // Render (HTML bibliography)
        .resource(
            Resource::<AppState>::new("/api/v1")
                .operation(
                    Endpoint::post("/render")
                        .request_schema::<RenderRequest>()
                        .response_schema::<RenderResponseBody>()
                        .tag("Render")
                        .description("Render bibliography items as HTML"),
                    render_bibitems,
                )
                .with_state(state.clone()),
        )
        // Admin: Export endpoints (return CSV streams — request schemas only)
        .resource(
            Resource::<AppState>::new("/api/v1/admin/export")
                .require_permission(Permission::Admin)
                .operation(
                    Endpoint::post("/bibitems")
                        .request_schema::<BibitemExportRequest>()
                        .tag("Admin")
                        .description("Export bibitems as CSV"),
                    export_bibitems,
                )
                .operation(
                    Endpoint::post("/authors")
                        .request_schema::<EntityExportRequest>()
                        .tag("Admin")
                        .description("Export authors as CSV"),
                    export_authors,
                )
                .operation(
                    Endpoint::post("/journals")
                        .request_schema::<EntityExportRequest>()
                        .tag("Admin")
                        .description("Export journals as CSV"),
                    export_journals,
                )
                .operation(
                    Endpoint::post("/publishers")
                        .request_schema::<EntityExportRequest>()
                        .tag("Admin")
                        .description("Export publishers as CSV"),
                    export_publishers,
                )
                .operation(
                    Endpoint::post("/institutions")
                        .request_schema::<EntityExportRequest>()
                        .tag("Admin")
                        .description("Export institutions as CSV"),
                    export_institutions,
                )
                .operation(
                    Endpoint::post("/schools")
                        .request_schema::<EntityExportRequest>()
                        .tag("Admin")
                        .description("Export schools as CSV"),
                    export_schools,
                )
                .operation(
                    Endpoint::post("/series")
                        .request_schema::<EntityExportRequest>()
                        .tag("Admin")
                        .description("Export series as CSV"),
                    export_series,
                )
                .operation(
                    Endpoint::post("/keywords")
                        .request_schema::<EntityExportRequest>()
                        .tag("Admin")
                        .description("Export keywords as CSV"),
                    export_keywords,
                )
                .with_state(state.clone()),
        )
        // Admin: Import endpoints (multipart CSV uploads — response schemas only)
        .resource(
            Resource::<AppState>::new("/api/v1/admin/import")
                .require_permission(Permission::Admin)
                .operation(
                    Endpoint::post("/bibitems")
                        .response_schema::<ImportResponse>()
                        .error_schema::<MissingReferencesError>("422")
                        .tag("Admin")
                        .description("Import bibitems from IDs-format CSV"),
                    import_bibitems,
                )
                .operation(
                    Endpoint::post("/authors")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import authors from CSV"),
                    import_authors,
                )
                .operation(
                    Endpoint::post("/journals")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import journals from CSV"),
                    import_journals,
                )
                .operation(
                    Endpoint::post("/publishers")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import publishers from CSV"),
                    import_publishers,
                )
                .operation(
                    Endpoint::post("/institutions")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import institutions from CSV"),
                    import_institutions,
                )
                .operation(
                    Endpoint::post("/schools")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import schools from CSV"),
                    import_schools,
                )
                .operation(
                    Endpoint::post("/series")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import series from CSV"),
                    import_series,
                )
                .operation(
                    Endpoint::post("/keywords")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import keywords from CSV"),
                    import_keywords,
                )
                .operation(
                    Endpoint::post("/author-name-variants")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import author name variants from CSV"),
                    import_author_name_variants,
                )
                .operation(
                    Endpoint::post("/bibitem-refs")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import bibitem references from CSV"),
                    import_bibitem_refs,
                )
                .operation(
                    Endpoint::post("/bibitem-notes")
                        .response_schema::<ImportResponse>()
                        .tag("Admin")
                        .description("Import bibitem notes from CSV"),
                    import_bibitem_notes,
                )
                .with_state(state.clone()),
        )
        // Admin: Full CSV, utility, and bulk endpoints
        .resource(
            Resource::<AppState>::new("/api/v1/admin")
                .require_permission(Permission::Admin)
                .operation(
                    Endpoint::post("/validate-full-csv")
                        .response_schema::<ValidationReport>()
                        .tag("Admin")
                        .description("Validate a human-readable CSV without importing"),
                    validate_full_csv,
                )
                .operation(
                    Endpoint::post("/import-entities-from-full-csv")
                        .response_schema::<EntityImportReport>()
                        .tag("Admin")
                        .description("Import missing entities from a human-readable CSV"),
                    import_entities_from_full_csv,
                )
                .operation(
                    Endpoint::post("/import-full-csv")
                        .response_schema::<FullImportReport>()
                        .error_schema::<ValidationReport>("422")
                        .tag("Admin")
                        .description("Import bibitems from human-readable CSV; upserts and optionally deletes stale"),
                    import_full_csv,
                )
                .operation(
                    Endpoint::post("/export-full-csv")
                        .tag("Admin")
                        .description("Export all bibitems as human-readable CSV"),
                    export_full_csv,
                )
                .operation(
                    Endpoint::post("/recompute-deps")
                        .tag("Admin")
                        .description("Recompute transitive further-ref dependencies"),
                    recompute_deps,
                )
                .operation(
                    Endpoint::post("/latex-to-unicode")
                        .request_schema::<LatexConvertRequest>()
                        .response_schema::<LatexConvertResponse>()
                        .tag("Admin")
                        .description("Convert a batch of LaTeX strings to Unicode"),
                    convert_latex_to_unicode,
                )
                .operation(
                    Endpoint::post("/compute-numeric-fields")
                        .response_schema::<ComputeNumericFieldsReport>()
                        .tag("Admin")
                        .description("Compute and persist numeric fields (start_page, volume_numeric, number_numeric) for all bibitems"),
                    compute_numeric_fields_handler,
                )
                .operation(
                    Endpoint::post("/convert-latex-columns")
                        .response_schema::<LatexConvertReport>()
                        .tag("Admin")
                        .description("Convert all LaTeX columns to Unicode across all entity tables"),
                    convert_latex_columns,
                )
                .operation(
                    Endpoint::post("/bulk-import/{table}")
                        .response_schema::<BulkImportResponse>()
                        .tag("Admin")
                        .description("High-throughput bulk import (~50× faster than upsert); use only after wipe"),
                    bulk_import_table,
                )
                .operation(
                    Endpoint::post("/wipe")
                        .response_schema::<WipeResponse>()
                        .tag("Admin")
                        .description("Truncate all data tables (pass ?confirm=true)"),
                    wipe_data,
                )
                .operation(
                    Endpoint::post("/snapshot")
                        .tag("Admin")
                        .description("Generate and download a ZIP snapshot of all data tables"),
                    snapshot_data,
                )
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
