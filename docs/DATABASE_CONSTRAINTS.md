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
| keywords | `keyword_key` | Business key (`{level}:{name}`) |
| keywords | `(name, level)` | Unique per name+level combination |
| bibitems | `bibkey` | Bibliography key (e.g., `kant:1781a`) |
| bibitem_notes | `bibkey` | One notes record per bibitem |

## Composite Primary Keys (junction tables)

| Table | Primary Key | Notes |
|-------|-------------|-------|
| bibitem_authors | `(bibkey, author_key, role)` | Same author can be both author and editor |
| bibitem_keywords | `(bibkey, keyword_key)` | |
| bibitem_refs | `(source_key, target_key, ref_type)` | Same pair can have further_ref and depends_on |
| bibitem_further_refs | `(source_key, dep_key)` | |
| bibitem_depends_on | `(source_key, dep_key)` | |

## CHECK Constraints

| Table | Constraint | Rule |
|-------|-----------|------|
| authors | `author_has_name` | `family_name_latex IS NOT NULL OR mononym_latex IS NOT NULL` |
| keywords | inline | `level BETWEEN 1 AND 3` |

## Foreign Keys

All FK columns use **business keys (TEXT)**, not surrogate integer IDs.

| Table | Column | References | On Delete |
|-------|--------|------------|-----------|
| bibitems | `journal_key` | journals(journal_key) | — |
| bibitems | `publisher_key` | publishers(publisher_key) | — |
| bibitems | `institution_key` | institutions(institution_key) | — |
| bibitems | `school_key` | schools(school_key) | — |
| bibitems | `series_key` | series(series_key) | — |
| bibitems | `crossref` | bibitems(bibkey) | — |
| bibitems | `person_key` | authors(author_key) | — |
| bibitem_notes | `bibkey` | bibitems(bibkey) | CASCADE |
| bibitem_authors | `bibkey` | bibitems(bibkey) | CASCADE |
| bibitem_authors | `author_key` | authors(author_key) | RESTRICT |
| bibitem_keywords | `bibkey` | bibitems(bibkey) | CASCADE |
| bibitem_keywords | `keyword_key` | keywords(keyword_key) | RESTRICT |
| bibitem_refs | `source_key` | bibitems(bibkey) | CASCADE |
| bibitem_refs | `target_key` | bibitems(bibkey) | RESTRICT |
| bibitem_further_refs | `source_key` | bibitems(bibkey) | CASCADE |
| bibitem_further_refs | `dep_key` | bibitems(bibkey) | RESTRICT |
| bibitem_depends_on | `source_key` | bibitems(bibkey) | CASCADE |
| bibitem_depends_on | `dep_key` | bibitems(bibkey) | RESTRICT |

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
| bibitems | `journal_key` | FK lookup |
| bibitems | `publisher_key` | FK lookup |
| bibitems | `crossref` | FK lookup |
| institutions | `name_simplified` | Name lookup |
| journals | `name_simplified` | Name lookup |
| keywords | `level` | Filter by level |
| keywords | `name` | Name lookup |
| publishers | `name_simplified` | Name lookup |
| schools | `name_simplified` | Name lookup |
| series | `name_simplified` | Name lookup |
| bibitem_authors | `author_key` | Reverse junction lookup |
| bibitem_keywords | `keyword_key` | Reverse junction lookup |
| bibitem_refs | `target_key` | Reverse ref lookup |
| bibitem_further_refs | `dep_key` | Reverse ref lookup |
| bibitem_depends_on | `dep_key` | Reverse ref lookup |

### GIN trigram (fuzzy text search)

| Table | Column | Purpose |
|-------|--------|---------|
| authors | `family_name_simplified` | Fuzzy author search |
| authors | `given_name_simplified` | Fuzzy author search |
| bibitems | `title_simplified` | Fuzzy title search |
| bibitems | `title_unicode` | Fuzzy title search (unicode variant) |
| bibitems | `bibkey` | Fuzzy bibkey search |
| institutions | `name_simplified` | Fuzzy name search |
| journals | `name_simplified` | Fuzzy name search |
| publishers | `name_simplified` | Fuzzy name search |
| schools | `name_simplified` | Fuzzy name search |
| series | `name_simplified` | Fuzzy name search |
