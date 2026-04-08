# Database Constraints & Indexes

## Unique Constraints

| Table | Column(s) | Notes |
|-------|-----------|-------|
| api_keys | `key_hash` | SHA-256 hash of the API key |
| authors | `author_key` | Business key (e.g., `kant_i`) |
| journals | `journal_key` | Business key |
| journals | `name_latex` | No duplicate journal names |
| publishers | `publisher_key` | Business key |
| publishers | `name_latex` | No duplicate publisher names |
| institutions | `institution_key` | Business key |
| institutions | `name_latex` | No duplicate institution names |
| schools | `school_key` | Business key |
| schools | `name_latex` | No duplicate school names |
| series | `series_key` | Business key |
| series | `name_latex` | No duplicate series names |
| keywords | `(name, level)` | Unique per name+level combination |
| bibitems | `bibkey` | Bibliography key (e.g., `kant:1781a`) |
| bibitem_notes | `bibitem_id` | One notes record per bibitem |

## Composite Primary Keys (junction tables)

| Table | Primary Key | Notes |
|-------|-------------|-------|
| bibitem_authors | `(bibitem_id, author_id, role)` | Same author can be both author and editor |
| bibitem_keywords | `(bibitem_id, keyword_id)` | |
| bibitem_refs | `(source_id, target_id, ref_type)` | Same pair can have further_ref and depends_on |

## CHECK Constraints

| Table | Constraint | Rule |
|-------|-----------|------|
| authors | `author_has_name` | `family_name_latex IS NOT NULL OR mononym_latex IS NOT NULL` |
| keywords | inline | `level BETWEEN 1 AND 3` |

## Foreign Keys

| Table | Column | References | On Delete |
|-------|--------|------------|-----------|
| bibitems | `journal_id` | journals(id) | — |
| bibitems | `publisher_id` | publishers(id) | — |
| bibitems | `institution_id` | institutions(id) | — |
| bibitems | `school_id` | schools(id) | — |
| bibitems | `series_id` | series(id) | — |
| bibitems | `crossref_id` | bibitems(id) | — |
| bibitems | `person_id` | authors(id) | — |
| bibitem_notes | `bibitem_id` | bibitems(id) | — |
| bibitem_authors | `bibitem_id` | bibitems(id) | CASCADE |
| bibitem_authors | `author_id` | authors(id) | RESTRICT |
| bibitem_keywords | `bibitem_id` | bibitems(id) | CASCADE |
| bibitem_keywords | `keyword_id` | keywords(id) | RESTRICT |
| bibitem_refs | `source_id` | bibitems(id) | CASCADE |
| bibitem_refs | `target_id` | bibitems(id) | RESTRICT |

## Indexes

### B-tree (exact match, range queries)

| Table | Column(s) | Purpose |
|-------|-----------|---------|
| api_keys | `key_hash` | Key lookup |
| api_keys | `key_hash WHERE revoked_at IS NULL` | Active key lookup (partial index) |
| authors | `family_name_simplified` | Name lookup |
| authors | `(family_name_simplified, given_name_simplified)` | Full name lookup |
| bibitems | `entry_type` | Filter by type |
| bibitems | `date_year` | Year range queries |
| bibitems | `journal_id` | FK lookup |
| bibitems | `publisher_id` | FK lookup |
| bibitems | `crossref_id` | FK lookup |
| institutions | `name_simplified` | Name lookup |
| journals | `name_simplified` | Name lookup |
| keywords | `level` | Filter by level |
| keywords | `name` | Name lookup |
| publishers | `name_simplified` | Name lookup |
| schools | `name_simplified` | Name lookup |
| series | `name_simplified` | Name lookup |
| bibitem_authors | `author_id` | Reverse junction lookup |
| bibitem_keywords | `keyword_id` | Reverse junction lookup |
| bibitem_refs | `target_id` | Reverse ref lookup |

### GIN trigram (fuzzy text search)

| Table | Column | Purpose |
|-------|--------|---------|
| authors | `family_name_simplified` | Fuzzy author search |
| authors | `given_name_simplified` | Fuzzy author search |
| bibitems | `title_simplified` | Fuzzy title search |
| bibitems | `bibkey` | Fuzzy bibkey search |
| institutions | `name_simplified` | Fuzzy name search |
| journals | `name_simplified` | Fuzzy name search |
| publishers | `name_simplified` | Fuzzy name search |
| schools | `name_simplified` | Fuzzy name search |
| series | `name_simplified` | Fuzzy name search |
