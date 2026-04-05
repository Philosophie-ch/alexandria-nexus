# Streaming Bulk Operations

## Problem

The admin bulk endpoints (`export-full-csv`, `validate-full-csv`, `import-full-csv`, and the existing `export/*`) load all data into memory before responding. With 213K bibitems, a single export request allocates ~500-600 MB.

This is fine for single-admin usage but won't scale to concurrent bulk operations or larger datasets.

## Current behavior

All bulk operations use `fetch_all()` → process in memory → return buffered response:

- `export_full_csv`: loads all bibitems + all junction data + all entity lookup maps → builds entire CSV in `Vec<u8>`
- `validate_full_csv`: loads all entity lookup maps (authors, journals, keywords, bibkeys) into HashMaps
- `import_full_csv`: same lookup maps + parses full CSV into `Vec<ParsedBibRow>`
- `export.rs` (existing): same `fetch_all` → buffer pattern

## Proposed fix: streaming CSV export

Replace buffered export with streaming using `axum::body::Body::from_stream`:

1. Open a database cursor (sqlx `fetch` returns a `Stream`)
2. For each bibitem row from the cursor, resolve its junction data and write one CSV row
3. Yield each row to the HTTP response body as a stream chunk
4. Junction data can be pre-loaded (entity lookup maps are small ~50MB) but bibitems stream row-by-row

```rust
// Sketch
pub async fn export_full_csv_stream(state: &AppState) -> impl IntoResponse {
    let pool = state.pool.pool();

    // Pre-load small lookup maps (authors, journals, keywords — these are small)
    let author_names = ...;
    let journal_names = ...;

    // Pre-load junction data indexed by bibitem_id
    let authors_by_bib = ...;
    let keywords_by_bib = ...;

    // Stream bibitems from DB cursor
    let stream = query_as::<_, BibItem>("SELECT * FROM bibitems ORDER BY bibkey")
        .fetch(pool)  // Returns Stream, not Vec
        .map(|row| {
            // Format one CSV row per bibitem
            format_csv_row(&row, &author_names, &journal_names, &authors_by_bib, ...)
        });

    // Return as streaming response
    let body = Body::from_stream(stream);
    Response::builder()
        .header("content-type", "text/csv")
        .body(body)
}
```

## Memory impact

| Component | Current | After streaming |
|-----------|---------|----------------|
| Entity lookup maps | ~50 MB | ~50 MB (same, pre-loaded) |
| Junction index maps | ~150 MB | ~150 MB (same, pre-loaded) |
| Bibitems | ~170 MB (all in Vec) | ~1 KB (one row at a time) |
| CSV output buffer | ~170 MB (full string) | ~0 (streamed) |
| **Total peak** | **~500-600 MB** | **~200 MB** |

The junction maps are the remaining bottleneck. To eliminate those too, use per-bibitem junction queries (N+1) or batch-fetch in chunks. For 213K entries the pre-loaded approach is fine.

## Streaming import/validate

Import and validate are harder to stream because they need all parsed rows before they can do batch validation. Possible approach:

1. **Two-pass import**: first pass counts and collects unique names (streaming), second pass resolves and inserts (streaming)
2. **Chunked validation**: validate in chunks of 10K rows, merge reports

Lower priority than export streaming since imports are less frequent.

## When to implement

Not needed for single-admin usage. Implement when:
- Multiple concurrent admins need bulk operations
- Dataset grows significantly beyond 213K
- Memory-constrained deployment (< 1GB)
