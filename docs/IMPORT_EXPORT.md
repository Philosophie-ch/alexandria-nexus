# Import / Export

All import and export endpoints require Admin authentication (`Authorization: Bearer <key>`).

## Workflow

1. Import reference entities (authors, journals, publishers, etc.)
2. Import bibitems (referencing entity keys from step 1)
3. Export as needed

## Bulk Import (corpus release — post-wipe only)

`POST /api/v1/admin/bulk-import/{table}` uses PostgreSQL `COPY FROM STDIN` rather than row-by-row upsert. **~50× faster** for large datasets (full corpus: ~36 seconds vs ~30 minutes).

**Use this for:** post-wipe corpus releases via `scripts/release.sh` or the GitHub Actions workflow.  
**Do NOT use for:** incremental updates — no `ON CONFLICT` support, tables must be empty.

Supported tables: `authors`, `journals`, `publishers`, `institutions`, `schools`, `series`, `keywords`, `bibitems`, `bibitem_authors`, `bibitem_keywords`, `bibitem_refs`, `bibitem_notes`.

The CSV must include all required columns for the table. Extra columns (e.g. `author_keys` embedded in the bibitem snapshot CSV) are silently dropped — no pre-filtering needed.

```bash
curl -X POST http://localhost:8080/api/v1/admin/bulk-import/bibitems \
  -H "Authorization: Bearer <key>" \
  -F "file=@bibitems.csv"
# → {"table": "bibitems", "rows": 186497}
```

## Import

All imports accept a CSV file via multipart form upload. Uses upsert (`ON CONFLICT DO UPDATE`) — safe for incremental updates on live data.

### Reference entities

```bash
curl -X POST http://localhost:8080/api/v1/admin/import/authors \
  -H "Authorization: Bearer <key>" \
  -F "file=@authors.csv"
```

Same pattern for `/import/journals`, `/import/publishers`, `/import/institutions`, `/import/schools`, `/import/series`, `/import/keywords`.

### CSV columns per entity

**Authors:**
```
author_key,family_name_latex,family_name_unicode,given_name_latex,given_name_unicode,mononym_latex,mononym_unicode,shorthand_latex,shorthand_unicode,famous_name_latex,famous_name_unicode,name_variants_latex,name_variants_unicode
```
Required: `author_key` + at least one of `family_name_*` or `mononym_*`.

**Journals:**
```
journal_key,name_latex,name_unicode,issn_print,issn_electronic
```
Required: `journal_key`, `name_latex`, `name_unicode`.

**Publishers / Institutions / Schools:**
```
{entity}_key,name_latex,name_unicode,default_address
```
Required: `{entity}_key`, `name_latex`, `name_unicode`.

**Series:**
```
series_key,name_latex,name_unicode
```
Required: `series_key`, `name_latex`, `name_unicode`.

**Keywords:**
```
name,level
```
Required: both. Level is 1, 2, or 3.

### Bibitems

```bash
curl -X POST http://localhost:8080/api/v1/admin/import/bibitems \
  -H "Authorization: Bearer <key>" \
  -F "file=@bibitems.csv"
```

**Columns:**
```
id,entry_type,bibkey,options,shorthand,date_year,pubstate,title_latex,title_unicode,booktitle_latex,booktitle_unicode,crossref,journal_key,volume,number,pages,eid,series_key,address,institution_key,school_key,publisher_key,type_field,edition,note_latex,note_unicode,issuetitle_latex,issuetitle_unicode,extra_note_latex,extra_note_unicode,urn,eprint,doi,url,langid,is_translation,epoch,author_keys,editor_keys,guesteditor_keys,keyword_keys
```

Required: `entry_type`, `bibkey`.

**Relation columns (all use business keys, not integer IDs):**
- `author_keys`: semicolon-separated author keys (e.g., `kant_i;smith_j`)
- `editor_keys`: semicolon-separated author keys (role = editor)
- `guesteditor_keys`: semicolon-separated author keys (role = guesteditor)
- `keyword_keys`: semicolon-separated `{level}:{name}` entries (e.g., `1:epistemology;2:knowledge`)
- `journal_key`, `publisher_key`, `institution_key`, `school_key`, `series_key`: single key
- `crossref`: bibkey of the parent entry

All referenced keys are validated before any insert. If any don't exist, a 422 is returned with the full list of missing keys grouped by entity type. Nothing is inserted.

**`title_unicode` nullability:** If `title_latex` contains `\cite*{...}` commands, `title_unicode` must be NULL (not a partially-rendered string). `POST /api/v1/admin/convert-latex-columns` enforces this invariant.

### Import response

```json
{
  "imported": 150,
  "updated": 12,
  "failed": 2,
  "errors": [
    { "row": 42, "identifier": "bad:entry", "error": "validation failed: ..." }
  ]
}
```

## Export

All exports are POST with a JSON body. Returns CSV.

### Selection modes

```bash
# Export everything
curl -X POST http://localhost:8080/api/v1/admin/export/authors \
  -H "Authorization: Bearer <key>" \
  -H "Content-Type: application/json" \
  -d '{"all": true}'

# By IDs
curl -X POST ... -d '{"ids": [1, 2, 3]}'

# By keys
curl -X POST ... -d '{"keys": ["kant_i", "plato"]}'
```

Same pattern for `/export/journals`, `/export/publishers`, `/export/institutions`, `/export/schools`, `/export/series`, `/export/keywords`.

If any requested IDs or keys don't exist, a 422 is returned with the full list of missing ones. No partial export.

### Bibitem export

```bash
# Human-readable (expanded names)
curl -X POST http://localhost:8080/api/v1/admin/export/bibitems \
  -H "Authorization: Bearer <key>" \
  -H "Content-Type: application/json" \
  -d '{"all": true, "format": "expanded"}'

# Machine-readable (IDs, for re-import)
curl -X POST ... -d '{"all": true, "format": "ids"}'

# By bibkeys
curl -X POST ... -d '{"format": "expanded", "bibkeys": ["kant:1781"]}'
```

**Expanded format:** Relations resolved to names:
- `author`: "LastName, FirstName and LastName2, FirstName2"
- `editor`, `guesteditor`: same format
- `journal`, `publisher`, `institution`, `school`, `series`: entity name
- `crossref`: bibkey of the referenced item
- `kw_level1`, `kw_level2`, `kw_level3`: semicolon-separated keyword names

**IDs format:** Relations as numeric IDs (same format accepted by import).

## Full CSV Import (Human-Readable)

### Quick start (shell scripts)

All scripts in `tools/` output raw JSON to stdout. Pipe to a file, then use the `-format` scripts to split into a human-readable summary (.txt) and per-row errors (.csv).

With the app running (`make dev-start`):

```bash
# 1. Validate
tools/validate-csv bibliography.csv > report.json
tools/validate-csv-format report.json summary.txt errors.csv

# 2. Create missing entities
tools/import-entities bibliography.csv > entities.json
tools/import-entities-format entities.json summary.txt errors.csv

# 3. Import bibitems (upsert only — safe, doesn't delete)
tools/import-bibitems bibliography.csv > import.json
tools/import-bibitems-format import.json summary.txt errors.csv

# 3b. Source-of-truth mode (also deletes bibitems not in the CSV)
tools/import-bibitems bibliography.csv --delete-stale > import.json

# 4. Export
tools/export-csv > exported.csv
```

The `-format` scripts take a JSON file and produce two files:
- **summary.txt** — counts, missing entities listed by name, stale bibkeys
- **errors.csv** — one row per error with columns: `row, bibkey, field, error` (open in a spreadsheet)

### API endpoints

For teams working with ODS/CSV spreadsheets that use human-readable names instead of database IDs. Three-step pipeline:

### Workflow

1. **Validate** -- check CSV for parse errors, missing entities, ambiguous authors, stale bibitems
2. **Import entities** -- create missing authors, journals, publishers, etc. from the CSV
3. **Import bibitems** -- resolve names to IDs, upsert bibitems + junctions, delete stale entries

### Step 1: Validate

```bash
curl -X POST http://localhost:8080/api/v1/admin/validate-full-csv \
  -H "Authorization: Bearer <key>" \
  -F "file=@bibliography.csv"
```

Response:
```json
{
  "total_rows": 1000,
  "valid_rows": 998,
  "errors": [{"row": 42, "bibkey": "bad:2024", "errors": [{"field": "date", "error": "..."}]}],
  "missing_authors": ["Unknown, Person"],
  "ambiguous_authors": [{"name": "Smith, John", "matching_ids": [5, 12]}],
  "missing_journals": ["Nonexistent Review"],
  "missing_publishers": [],
  "missing_institutions": [],
  "missing_schools": [],
  "missing_series": [],
  "missing_keywords": {"level_1": ["new-keyword"], "level_2": [], "level_3": []},
  "missing_crossrefs": [],
  "missing_further_refs": [],
  "missing_depends_on": [],
  "stale_bibitems": ["old:2020"]
}
```

### Step 2: Import entities

```bash
curl -X POST http://localhost:8080/api/v1/admin/import-entities-from-full-csv \
  -H "Authorization: Bearer <key>" \
  -F "file=@bibliography.csv"
```

Creates authors, journals, publishers, institutions, schools, series, and keywords that are referenced in the CSV but don't exist in the database. Auto-generates keys from names. Does NOT create duplicate authors with the same name.

### Step 3: Import bibitems

```bash
curl -X POST http://localhost:8080/api/v1/admin/import-full-csv \
  -H "Authorization: Bearer <key>" \
  -F "file=@bibliography.csv"
```

The CSV is the **source of truth**:
- New bibitems are inserted
- Existing bibitems (same bibkey) are updated
- Bibitems in the DB but NOT in the CSV are **deleted**

Rows whose entity references (author, journal, publisher, etc.) cannot be resolved are **skipped individually** — the rest of the import continues. Skipped rows are reported in `errors`. The only hard failure is duplicate bibkeys within the CSV itself (returns 422 with no changes).

Response:
```json
{
  "imported": 1200,
  "updated": 50,
  "deleted": 3,
  "failed": 0,
  "skipped": 12,
  "errors": [
    { "row": 42, "identifier": "bad:2024", "error": "unresolvable entity: journal 'Nonexistent Review'" }
  ]
}
```

### CSV format

The CSV follows the ODS spreadsheet structure. Column names use hyphens (normalized to underscores internally).

**Key columns:**
- `entry_type` -- BibTeX type (`@article{` or `article`)
- `bibkey` -- unique identifier in `author:year` format (e.g., `kant:1781a`)
- `title` -- single column, stored in all three variants (latex/unicode/simplified)
- `author` -- `" and "`-separated: `"Kant, Immanuel and Aristotle"`
- `editor`, `_guesteditor` -- same format as author
- `date` -- year (`2024`), range (`2021-2022`, `2021/2022`), full (`2024-01-15`), or `no date`
- `journal`, `publisher`, `institution`, `school`, `series` -- human-readable names (looked up by `name_latex`)
- `pages` -- double-hyphen ranges: `123--456, 789`
- `_kw-level1`, `_kw-level2`, `_kw-level3` -- semicolon-separated keyword names
- `crossref` -- bibkey of parent entry
- `_further_refs`, `_depends_on` -- comma-separated bibkeys
- `_person` -- philosopher mononym with optional trailing semicolon (e.g., `Kierkegaard;`)
- `pubstate`, `_epoch`, `_langid` -- enum values (default to None on invalid)
- `_lang-der` -- non-empty = is_translation
- `_has-link-to-full-text` -- non-empty = has_fulltext
