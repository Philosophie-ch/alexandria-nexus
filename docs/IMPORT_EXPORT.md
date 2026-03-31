# Import / Export

All import and export endpoints require Admin authentication (`Authorization: Bearer <key>`).

## Workflow

1. Import reference entities (authors, journals, publishers, etc.)
2. Import bibitems (referencing entity IDs from step 1)
3. Export as needed

## Import

All imports accept a CSV file via multipart form upload.

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
author_key,family_name_latex,family_name_unicode,family_name_simplified,given_name_latex,given_name_unicode,given_name_simplified,mononym_latex,mononym_unicode,mononym_simplified,shorthand_latex,shorthand_unicode,shorthand_simplified,famous_name_latex,famous_name_unicode,famous_name_simplified
```
Required: `author_key` + at least one of `family_name_*` or `mononym_*`.

**Journals:**
```
journal_key,name_latex,name_unicode,name_simplified,issn_print,issn_electronic
```
Required: `journal_key`, `name_latex`, `name_unicode`, `name_simplified`.

**Publishers / Institutions / Schools:**
```
{entity}_key,name_latex,name_unicode,name_simplified,default_address
```
Required: `{entity}_key`, `name_latex`, `name_unicode`, `name_simplified`.

**Series:**
```
series_key,name_latex,name_unicode,name_simplified
```
Required: `series_key`, `name_latex`, `name_unicode`, `name_simplified`.

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
entry_type,bibkey,title_latex,title_unicode,title_simplified,date_year,date_month,date_day,pubstate,booktitle_latex,booktitle_unicode,booktitle_simplified,volume,number,pages,eid,address,type_field,edition,doi,url,eprint,urn,options,shorthand,note_latex,note_unicode,issuetitle_latex,issuetitle_unicode,extra_note_latex,extra_note_unicode,langid,is_translation,epoch,journal_id,publisher_id,institution_id,school_id,series_id,crossref_id,author_ids,editor_ids,guesteditor_ids,keyword_ids
```

Required: `entry_type`, `bibkey`, `title_latex`, `title_unicode`, `title_simplified`.

**Relation columns:**
- `author_ids`: comma-separated author IDs (e.g., `42,17,3`)
- `editor_ids`: comma-separated author IDs (role = editor)
- `guesteditor_ids`: comma-separated author IDs (role = guesteditor)
- `keyword_ids`: comma-separated keyword IDs
- `journal_id`, `publisher_id`, `institution_id`, `school_id`, `series_id`, `crossref_id`: single ID

All referenced IDs are validated before any insert. If any don't exist, a 422 is returned with the full list of missing IDs grouped by entity type. Nothing is inserted.

### Import response

```json
{
  "imported": 150,
  "failed": 2,
  "errors": [
    { "row": 42, "bibkey": "bad:entry", "error": "validation failed: ..." }
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

Returns 422 if any entities can't be resolved (missing or ambiguous). Nothing is modified until all references are validated.

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
