//! Adapter factory methods for AppState.
//!
//! Composition makes wiring decisions: which concrete adapter types to use
//! and how they are configured. Handlers call these methods to obtain
//! pre-configured adapters without importing concrete adapter types.

use hexforge::HexforgeError;

use super::AppState;

use crate::adapters::compute_start_pages::PgStartPageComputer;
use crate::adapters::export::{
    PgBibitemFetcher, PgEntityBatchFetcher, PgExportJunctionFetcher, PgKeyedEntityFetcher,
    PgKeywordFetcher as PgKeywordExportFetcher,
};
use crate::adapters::field_parsing::parse_variant_to_keys;
use crate::adapters::full_import::PgFullImportStore;
use crate::adapters::import::{
    PgBibitemJunctionStore, PgBibitemNotesStore, PgBibitemRefsStore, PgEntityBatchLookup,
    PgNameVariantStore, PgReferenceStore, PgSequenceSyncer,
};
use crate::adapters::keyword_tree::PgKeywordFetcher;
use crate::adapters::latex_citations::PgCitationResolver;
use crate::adapters::latex_columns::PgLatexColumnConverter;
use crate::adapters::latex_to_unicode::PyLatexConverter;
use crate::adapters::render::{
    PgBibitemResolver, PgRenderAuthorFetcher, PgRenderEntityFetcher, PgTransitiveDepsResolver,
};
use crate::adapters::search::PgBibitemSearcher;
use crate::adapters::snapshot::PgSnapshotFetcher;
use crate::adapters::wipe::wipe_tables;

use crate::adapters::db::queries::{
    AuthorQuery, BibItemQuery, InstitutionQuery, JournalQuery, KeywordQuery, PublisherQuery,
    SchoolQuery, SeriesQuery,
};
use crate::domain::{Author, BibItem, Institution, Journal, Keyword, Publisher, School, Series};

// =============================================================================
// Search
// =============================================================================

impl AppState {
    pub fn searcher(&self) -> PgBibitemSearcher<'_> {
        PgBibitemSearcher::new(self.pool.pool())
    }
}

// =============================================================================
// Keyword tree
// =============================================================================

impl AppState {
    pub fn keyword_fetcher(&self) -> PgKeywordFetcher<'_> {
        PgKeywordFetcher::new(self.pool.pool())
    }
}

// =============================================================================
// Snapshot
// =============================================================================

impl AppState {
    pub fn snapshot_fetcher(&self) -> PgSnapshotFetcher {
        PgSnapshotFetcher::new(self.pool.pool().clone())
    }
}

// =============================================================================
// Render
// =============================================================================

impl AppState {
    pub fn bibitem_resolver(&self) -> PgBibitemResolver<'_, BibItemQuery> {
        PgBibitemResolver::new(&self.bibitem_ds)
    }

    pub fn render_entity_fetcher(&self) -> PgRenderEntityFetcher<'_> {
        PgRenderEntityFetcher::new(self.pool.pool())
    }

    pub fn render_author_fetcher(&self) -> PgRenderAuthorFetcher<'_> {
        PgRenderAuthorFetcher::new(self.pool.pool())
    }

    pub fn transitive_deps_resolver(&self) -> PgTransitiveDepsResolver<'_> {
        PgTransitiveDepsResolver::new(self.pool.pool())
    }
}

// =============================================================================
// Compute start pages
// =============================================================================

impl AppState {
    pub fn start_page_fetcher(&self) -> PgStartPageComputer<'_> {
        PgStartPageComputer::new(self.pool.pool())
    }

    pub fn start_page_writer(&self) -> PgStartPageComputer<'_> {
        PgStartPageComputer::new(self.pool.pool())
    }
}

// =============================================================================
// LaTeX conversion
// =============================================================================

impl AppState {
    pub fn latex_column_fetcher(&self) -> PgLatexColumnConverter<'_> {
        PgLatexColumnConverter::new(self.pool.pool())
    }

    pub fn latex_column_writer(&self) -> PgLatexColumnConverter<'_> {
        PgLatexColumnConverter::new(self.pool.pool())
    }

    pub fn citation_resolver(&self) -> PgCitationResolver<'_> {
        PgCitationResolver::new(self.pool.pool())
    }

    pub fn latex_converter(&self) -> PyLatexConverter {
        PyLatexConverter
    }
}

// =============================================================================
// Export — keyed entity fetchers
// =============================================================================

impl AppState {
    pub fn author_export_fetcher(&self) -> PgKeyedEntityFetcher<'_, Author, AuthorQuery> {
        PgKeyedEntityFetcher::new(&self.author_ds, "author_key", "authors", self.pool.pool())
    }

    pub fn journal_export_fetcher(&self) -> PgKeyedEntityFetcher<'_, Journal, JournalQuery> {
        PgKeyedEntityFetcher::new(
            &self.journal_ds,
            "journal_key",
            "journals",
            self.pool.pool(),
        )
    }

    pub fn publisher_export_fetcher(&self) -> PgKeyedEntityFetcher<'_, Publisher, PublisherQuery> {
        PgKeyedEntityFetcher::new(
            &self.publisher_ds,
            "publisher_key",
            "publishers",
            self.pool.pool(),
        )
    }

    pub fn institution_export_fetcher(
        &self,
    ) -> PgKeyedEntityFetcher<'_, Institution, InstitutionQuery> {
        PgKeyedEntityFetcher::new(
            &self.institution_ds,
            "institution_key",
            "institutions",
            self.pool.pool(),
        )
    }

    pub fn school_export_fetcher(&self) -> PgKeyedEntityFetcher<'_, School, SchoolQuery> {
        PgKeyedEntityFetcher::new(&self.school_ds, "school_key", "schools", self.pool.pool())
    }

    pub fn series_export_fetcher(&self) -> PgKeyedEntityFetcher<'_, Series, SeriesQuery> {
        PgKeyedEntityFetcher::new(&self.series_ds, "series_key", "series", self.pool.pool())
    }

    pub fn keyword_export_fetcher(&self) -> PgKeywordExportFetcher<'_, KeywordQuery> {
        PgKeywordExportFetcher::new(&self.keyword_ds, self.pool.pool())
    }

    pub fn bibitem_export_fetcher(&self) -> PgBibitemFetcher<'_, BibItemQuery> {
        PgBibitemFetcher::new(&self.bibitem_ds, self.pool.pool())
    }

    pub fn export_junction_fetcher(&self) -> PgExportJunctionFetcher<'_> {
        PgExportJunctionFetcher::new(self.pool.pool())
    }
}

// =============================================================================
// Export — batch entity fetchers (for expanded bibitem export)
// =============================================================================

impl AppState {
    pub fn author_batch_export_fetcher(&self) -> PgEntityBatchFetcher<'_, Author, AuthorQuery> {
        PgEntityBatchFetcher::new(&self.author_ds, "author_key", "authors", self.pool.pool())
    }

    pub fn journal_batch_export_fetcher(&self) -> PgEntityBatchFetcher<'_, Journal, JournalQuery> {
        PgEntityBatchFetcher::new(
            &self.journal_ds,
            "journal_key",
            "journals",
            self.pool.pool(),
        )
    }

    pub fn publisher_batch_export_fetcher(
        &self,
    ) -> PgEntityBatchFetcher<'_, Publisher, PublisherQuery> {
        PgEntityBatchFetcher::new(
            &self.publisher_ds,
            "publisher_key",
            "publishers",
            self.pool.pool(),
        )
    }

    pub fn institution_batch_export_fetcher(
        &self,
    ) -> PgEntityBatchFetcher<'_, Institution, InstitutionQuery> {
        PgEntityBatchFetcher::new(
            &self.institution_ds,
            "institution_key",
            "institutions",
            self.pool.pool(),
        )
    }

    pub fn school_batch_export_fetcher(&self) -> PgEntityBatchFetcher<'_, School, SchoolQuery> {
        PgEntityBatchFetcher::new(&self.school_ds, "school_key", "schools", self.pool.pool())
    }

    pub fn series_batch_export_fetcher(&self) -> PgEntityBatchFetcher<'_, Series, SeriesQuery> {
        PgEntityBatchFetcher::new(&self.series_ds, "series_key", "series", self.pool.pool())
    }

    pub fn bibitem_batch_export_fetcher(&self) -> PgEntityBatchFetcher<'_, BibItem, BibItemQuery> {
        PgEntityBatchFetcher::new(&self.bibitem_ds, "bibkey", "bibitems", self.pool.pool())
    }

    pub fn keyword_batch_export_fetcher(&self) -> PgEntityBatchFetcher<'_, Keyword, KeywordQuery> {
        PgEntityBatchFetcher::new(
            &self.keyword_ds,
            "keyword_key",
            "keywords",
            self.pool.pool(),
        )
    }
}

// =============================================================================
// Import — entity batch lookups
// =============================================================================

impl AppState {
    pub fn author_batch_lookup(&self) -> PgEntityBatchLookup<'_, Author, AuthorQuery> {
        PgEntityBatchLookup::new(&self.author_ds)
    }

    pub fn journal_batch_lookup(&self) -> PgEntityBatchLookup<'_, Journal, JournalQuery> {
        PgEntityBatchLookup::new(&self.journal_ds)
    }

    pub fn publisher_batch_lookup(&self) -> PgEntityBatchLookup<'_, Publisher, PublisherQuery> {
        PgEntityBatchLookup::new(&self.publisher_ds)
    }

    pub fn institution_batch_lookup(
        &self,
    ) -> PgEntityBatchLookup<'_, Institution, InstitutionQuery> {
        PgEntityBatchLookup::new(&self.institution_ds)
    }

    pub fn school_batch_lookup(&self) -> PgEntityBatchLookup<'_, School, SchoolQuery> {
        PgEntityBatchLookup::new(&self.school_ds)
    }

    pub fn series_batch_lookup(&self) -> PgEntityBatchLookup<'_, Series, SeriesQuery> {
        PgEntityBatchLookup::new(&self.series_ds)
    }

    pub fn keyword_batch_lookup(&self) -> PgEntityBatchLookup<'_, Keyword, KeywordQuery> {
        PgEntityBatchLookup::new(&self.keyword_ds)
    }
}

// =============================================================================
// Import — stores
// =============================================================================

impl AppState {
    pub fn sequence_syncer(&self) -> PgSequenceSyncer<'_> {
        PgSequenceSyncer::new(self.pool.pool())
    }

    pub fn name_variant_store(&self) -> PgNameVariantStore<'_> {
        PgNameVariantStore::new(self.pool.pool())
    }

    pub fn bibitem_junction_store(&self) -> PgBibitemJunctionStore<'_> {
        PgBibitemJunctionStore::new(self.pool.pool())
    }

    pub fn reference_store(&self) -> PgReferenceStore<'_> {
        PgReferenceStore::new(self.pool.pool())
    }

    pub fn bibitem_refs_store(&self) -> PgBibitemRefsStore<'_> {
        PgBibitemRefsStore::new(self.pool.pool())
    }

    pub fn bibitem_notes_store(&self) -> PgBibitemNotesStore<'_> {
        PgBibitemNotesStore::new(self.pool.pool())
    }
}

// =============================================================================
// Full import
// =============================================================================

impl AppState {
    pub fn full_import_store(&self) -> PgFullImportStore<'_> {
        PgFullImportStore::new(self.pool.pool(), parse_variant_to_keys)
    }
}

// =============================================================================
// Wipe
// =============================================================================

impl AppState {
    pub async fn wipe(&self) -> Result<(), HexforgeError> {
        wipe_tables(self.pool.pool()).await
    }
}
