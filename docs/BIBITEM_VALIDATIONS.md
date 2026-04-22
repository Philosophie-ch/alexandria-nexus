# BibItem Validation Rules

Validation happens at three levels: CSV parsing (syntax), DTO validation (business rules), and database constraints (integrity). All three work together — the API never stores invalid data.

## Level 1: CSV Parsing (syntax)

Applied by `csv_parsing/` when parsing human-readable CSVs. Per-field, all errors collected in one pass.

### Bibkey

- Required (non-empty)
- Must contain exactly one colon: `author:date`
- Author part: 1-2 hyphen-separated components (e.g., `kant` or `kant-smith`)
- Date part: digits (year) with optional suffix, or `unpub`/`forthcoming` with optional `-suffix`
- Year abs <= 9999
- Negative years allowed (e.g., `plato:-380`)
- Empty suffix after `-` in pubstate is an error (e.g., `smith:forthcoming-` fails)

### Author / Editor / Guesteditor

- `" and "`-separated list (space-and-space, not bare `and`)
- Each author: 1-3 comma-separated parts
  - 1 part: mononym (e.g., `Aristotle`)
  - 2 parts: `Family, Given` (e.g., `Kant, Immanuel`)
  - 3 parts: `Family, Suffix, Given` (e.g., `Belnap, Jr., Nuel` — suffix merged into family)
  - 4+ commas: error
- Empty family name after split: error

### Date

- Empty or `"no date"` (case-insensitive): valid (NoDate)
- `YYYY`: single year
- `YYYY-YYYY`: year range (hyphen separator)
- `YYYY/YYYY`: year range (slash separator)
- `YYYY-MM-DD`: full date (month/day parts must be 1-2 digits)
- `-YYYY`: negative year (ancient dates)
- Year abs <= 9999
- Month: 1-12
- Day: 1-31

### Pages

- Comma-separated entries
- Range: `start--end` (double hyphen required)
- Single page: `123`
- Single hyphen (`123-456`): error — must use double hyphen
- Empty start or end in range: error

### Keywords

- Semicolon-separated names per level (`_kw_level1`, `_kw_level2`, `_kw_level3`)
- Whitespace trimmed, empty entries filtered

### Entry Type

- Strips `@`, `{`, `}`, lowercases
- Maps to domain enum (article, book, incollection, etc.)
- Unknown values default to `UNKNOWN` (not an error)

### Pubstate / Epoch / LangId

- Parsed as domain enum values
- Invalid values default to `None` (not an error)

### Booleans

- `_lang_der` (is_translation): non-empty = true
- `_has_link_to_full_text` (has_fulltext): non-empty = true

## Level 2: DTO Validation (business rules)

Applied by `validation/bibitem.rs` before database insertion. Both CRUD API and full CSV import.

### Create

- `bibkey`: required, non-empty (trimmed)
- `title`: at least one of `title_latex`, `title_unicode`, `title_simplified` must be non-empty
- If `date_year_2_hyphen` or `date_year_2_slash` is set, `date_year` must be set
- Cannot have both `date_year_2_hyphen` and `date_year_2_slash`
- `date_month` (if set): 1-12
- `date_day` (if set): 1-31

### Update

- `bibkey` (if provided): cannot be empty
- `date_month` (if set): 1-12
- `date_day` (if set): 1-31

## Level 3: Full CSV Validation Report

Applied by `POST /api/v1/admin/validate-full-csv`. Collects all issues in one response.

### Duplicate detection

- `duplicate_bibkeys`: bibkeys appearing more than once in the CSV, with row numbers

### Entity resolution

- `missing_authors`: author names in CSV not found in DB (by `family_name_latex + given_name_latex` or `mononym_latex`)
- `ambiguous_authors`: author names matching multiple DB records (with matching IDs listed)
- `missing_journals`: journal names not found by `name_latex`
- `missing_publishers`: publisher names not found by `name_latex`
- `missing_institutions`: institution names not found by `name_latex`
- `missing_schools`: school names not found by `name_latex`
- `missing_series`: series names not found by `name_latex`
- `missing_keywords`: keyword names not found by `(name, level)` — grouped by level

### Reference resolution

- `missing_crossrefs`: bibkeys in `crossref` column not found in DB
- `missing_further_refs`: bibkeys in `_further_refs` not found in DB or CSV
- `missing_depends_on`: bibkeys in `_depends_on` not found in DB or CSV

### Stale detection

- `stale_bibitems`: bibkeys in DB but NOT in the CSV (will be deleted on import)

## Level 4: Database Constraints (integrity)

Enforced by PostgreSQL. These catch anything that slips past application validation.

All FK columns use **business keys (TEXT)**, not surrogate integer IDs.

- `bibkey TEXT NOT NULL UNIQUE` — no duplicates, no nulls
- `entry_type entry_type NOT NULL DEFAULT 'UNKNOWN'` — valid enum value
- `title_latex TEXT NOT NULL` — required; `title_unicode TEXT` nullable (NULL when contains `\cite*{}`)
- `journal_key TEXT REFERENCES journals(journal_key)` — FK integrity
- `publisher_key TEXT REFERENCES publishers(publisher_key)`
- `institution_key TEXT REFERENCES institutions(institution_key)`
- `school_key TEXT REFERENCES schools(school_key)`
- `series_key TEXT REFERENCES series(series_key)`
- `crossref TEXT REFERENCES bibitems(bibkey)` — self-referencing FK via bibkey
- `person_key TEXT REFERENCES authors(author_key)`
- `bibitem_authors`: `PRIMARY KEY (bibkey, author_key, role)` + `ON DELETE CASCADE` (bibitem) / `ON DELETE RESTRICT` (author)
- `bibitem_keywords`: `PRIMARY KEY (bibkey, keyword_key)` + `ON DELETE CASCADE` (bibitem) / `ON DELETE RESTRICT` (keyword)
- `bibitem_refs`: `PRIMARY KEY (source_key, target_key, ref_type)` + `ON DELETE CASCADE` (source) / `ON DELETE RESTRICT` (target)
