
-- =============================================================================
-- EXTENSIONS
-- =============================================================================

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- =============================================================================
-- ENUMS
-- =============================================================================

CREATE TYPE author_role AS ENUM ('author', 'editor', 'guesteditor');

CREATE TYPE entry_type AS ENUM ('article', 'book', 'incollection', 'inproceedings', 'mastersthesis', 'misc', 'phdthesis', 'proceedings', 'techreport', 'unpublished', 'UNKNOWN');

CREATE TYPE epoch AS ENUM ('ancient-philosophy', 'ancient-scientists', 'austrian-philosophy', 'british-idealism', 'classics', 'contemporaries', 'contemporary-scientists', 'continental-philosophy', 'critical-theory', 'cynics', 'enlightenment', 'existentialism', 'exotic-philosophy', 'german-idealism', 'german-rationalism', 'gestalt-psychology', 'hermeneutics', 'islamic-philosophy', 'mathematicians', 'medieval-philosophy', 'modern-philosophy', 'modern-scientists', 'neo-kantianism', 'neoplatonism', 'new-realism', 'ordinary-language-philosophy', 'phenomenology', 'polish-logic', 'pragmatism', 'presocratics', 'renaissance', 'stoics', 'theologians', 'vienna-circle');

CREATE TYPE langid AS ENUM ('catalan', 'czech', 'danish', 'dutch', 'english', 'french', 'greek', 'italian', 'latin', 'lithuanian', 'ngerman', 'polish', 'portuguese', 'romanian', 'russian', 'slovak', 'spanish', 'swedish', 'unknown');

CREATE TYPE permission_level AS ENUM ('public', 'read', 'write', 'admin');

CREATE TYPE pubstate AS ENUM ('unpub', 'forthcoming', 'inwork', 'submitted', 'published');

CREATE TYPE ref_type AS ENUM ('further_ref', 'depends_on');


-- =============================================================================
-- TABLES
-- =============================================================================

CREATE TABLE api_keys (
    id BIGSERIAL PRIMARY KEY,
    key_hash TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    permission permission_level NOT NULL DEFAULT 'read',
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE authors (
    id BIGSERIAL PRIMARY KEY,
    author_key TEXT NOT NULL UNIQUE,
    family_name_latex TEXT,
    family_name_unicode TEXT,
    famous_name_latex TEXT,
    famous_name_unicode TEXT,
    given_name_latex TEXT,
    given_name_unicode TEXT,
    mononym_latex TEXT,
    mononym_unicode TEXT,
    name_variants_latex TEXT[],
    name_variants_unicode TEXT[],
    shorthand_latex TEXT,
    shorthand_unicode TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT author_has_name CHECK (family_name_latex IS NOT NULL OR mononym_latex IS NOT NULL)
);

CREATE TABLE data_version (
    id BIGSERIAL PRIMARY KEY,
    description TEXT,
    imported_at TIMESTAMPTZ NOT NULL,
    version TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE institutions (
    id BIGSERIAL PRIMARY KEY,
    default_address TEXT,
    institution_key TEXT NOT NULL UNIQUE,
    name_latex TEXT NOT NULL UNIQUE,
    name_unicode TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE journals (
    id BIGSERIAL PRIMARY KEY,
    issn_electronic TEXT,
    issn_print TEXT,
    journal_key TEXT NOT NULL UNIQUE,
    name_latex TEXT NOT NULL UNIQUE,
    name_unicode TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE keywords (
    id BIGSERIAL PRIMARY KEY,
    level SMALLINT NOT NULL CHECK (level BETWEEN 1 AND 3),
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE publishers (
    id BIGSERIAL PRIMARY KEY,
    default_address TEXT,
    name_latex TEXT NOT NULL UNIQUE,
    name_unicode TEXT NOT NULL,
    publisher_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE schools (
    id BIGSERIAL PRIMARY KEY,
    name_latex TEXT NOT NULL UNIQUE,
    name_unicode TEXT NOT NULL,
    school_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE series (
    id BIGSERIAL PRIMARY KEY,
    name_latex TEXT NOT NULL UNIQUE,
    name_unicode TEXT NOT NULL,
    series_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bibitems (
    id BIGSERIAL PRIMARY KEY,
    address TEXT,
    bibkey TEXT NOT NULL UNIQUE,
    booktitle_latex TEXT,
    booktitle_unicode TEXT,
    crossref_id BIGINT REFERENCES bibitems(id),
    date_day SMALLINT,
    date_is_no_date BOOLEAN NOT NULL DEFAULT FALSE,
    date_month SMALLINT,
    date_year SMALLINT,
    date_year_2_hyphen SMALLINT,
    date_year_2_slash SMALLINT,
    doi TEXT,
    edition TEXT,
    eid TEXT,
    entry_type entry_type NOT NULL DEFAULT 'UNKNOWN',
    epoch epoch,
    eprint TEXT,
    extra_note_latex TEXT,
    extra_note_unicode TEXT,
    fulltext_path TEXT,
    has_fulltext BOOLEAN NOT NULL DEFAULT FALSE,
    institution_id BIGINT REFERENCES institutions(id),
    is_translation BOOLEAN NOT NULL DEFAULT FALSE,
    issuetitle_latex TEXT,
    issuetitle_unicode TEXT,
    journal_id BIGINT REFERENCES journals(id),
    langid langid,
    note_latex TEXT,
    note_unicode TEXT,
    number TEXT,
    options TEXT,
    pages TEXT,
    person_id BIGINT REFERENCES authors(id),
    publisher_id BIGINT REFERENCES publishers(id),
    pubstate pubstate,
    school_id BIGINT REFERENCES schools(id),
    series_id BIGINT REFERENCES series(id),
    shorthand TEXT,
    title_latex TEXT NOT NULL,
    title_unicode TEXT NOT NULL,
    type_field TEXT,
    url TEXT,
    urn TEXT,
    volume TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bibitem_notes (
    id BIGSERIAL PRIMARY KEY,
    bibitem_id BIGINT NOT NULL UNIQUE REFERENCES bibitems(id),
    notes JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);


-- =============================================================================
-- JUNCTION TABLES
-- =============================================================================

CREATE TABLE bibitem_authors (
    bibitem_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    author_id BIGINT NOT NULL REFERENCES authors(id) ON DELETE RESTRICT,
    role author_role NOT NULL DEFAULT 'author',
    position SMALLINT NOT NULL,
    name_variant_latex TEXT,
    name_variant_unicode TEXT,
    PRIMARY KEY (bibitem_id, author_id, role)
);
CREATE INDEX idx_bibitem_authors_author_id ON bibitem_authors(author_id);

CREATE TABLE bibitem_depends_on (
    source_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    dep_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE RESTRICT,
    PRIMARY KEY (source_id, dep_id)
);
CREATE INDEX idx_bibitem_depends_on_dep_id ON bibitem_depends_on(dep_id);

CREATE TABLE bibitem_further_refs (
    source_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    dep_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE RESTRICT,
    PRIMARY KEY (source_id, dep_id)
);
CREATE INDEX idx_bibitem_further_refs_dep_id ON bibitem_further_refs(dep_id);

CREATE TABLE bibitem_keywords (
    bibitem_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    keyword_id BIGINT NOT NULL REFERENCES keywords(id) ON DELETE RESTRICT,
    keyword_level SMALLINT NOT NULL,
    PRIMARY KEY (bibitem_id, keyword_id)
);
CREATE INDEX idx_bibitem_keywords_keyword_id ON bibitem_keywords(keyword_id);

CREATE TABLE bibitem_refs (
    source_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    target_id BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE RESTRICT,
    ref_type ref_type NOT NULL,
    PRIMARY KEY (source_id, target_id, ref_type)
);
CREATE INDEX idx_bibitem_refs_target_id ON bibitem_refs(target_id);


-- =============================================================================
-- INDEXES
-- =============================================================================

CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_key_hash_partial ON api_keys(key_hash) WHERE revoked_at IS NULL;
CREATE INDEX idx_authors_family_name_unicode ON authors(family_name_unicode);
CREATE INDEX idx_authors_family_name_unicode_given_name_unicode ON authors(family_name_unicode, given_name_unicode);
CREATE INDEX idx_authors_family_name_unicode_trgm ON authors USING gin(family_name_unicode gin_trgm_ops);
CREATE INDEX idx_authors_name_variants_latex_gin ON authors USING gin(name_variants_latex);
CREATE INDEX idx_authors_name_variants_unicode_gin ON authors USING gin(name_variants_unicode);
CREATE INDEX idx_authors_given_name_unicode_trgm ON authors USING gin(given_name_unicode gin_trgm_ops);
CREATE INDEX idx_bibitems_entry_type ON bibitems(entry_type);
CREATE INDEX idx_bibitems_date_year ON bibitems(date_year);
CREATE INDEX idx_bibitems_journal_id ON bibitems(journal_id);
CREATE INDEX idx_bibitems_publisher_id ON bibitems(publisher_id);
CREATE INDEX idx_bibitems_crossref_id ON bibitems(crossref_id);
CREATE INDEX idx_bibitems_title_unicode_trgm ON bibitems USING gin(title_unicode gin_trgm_ops);
CREATE INDEX idx_bibitems_bibkey_trgm ON bibitems USING gin(bibkey gin_trgm_ops);
CREATE INDEX idx_institutions_name_unicode ON institutions(name_unicode);
CREATE INDEX idx_institutions_name_unicode_trgm ON institutions USING gin(name_unicode gin_trgm_ops);
CREATE INDEX idx_journals_name_unicode ON journals(name_unicode);
CREATE INDEX idx_journals_name_unicode_trgm ON journals USING gin(name_unicode gin_trgm_ops);
CREATE INDEX idx_keywords_level ON keywords(level);
CREATE INDEX idx_keywords_name ON keywords(name);
CREATE INDEX idx_publishers_name_unicode ON publishers(name_unicode);
CREATE INDEX idx_publishers_name_unicode_trgm ON publishers USING gin(name_unicode gin_trgm_ops);
CREATE INDEX idx_schools_name_unicode ON schools(name_unicode);
CREATE INDEX idx_schools_name_unicode_trgm ON schools USING gin(name_unicode gin_trgm_ops);
CREATE INDEX idx_series_name_unicode ON series(name_unicode);
CREATE INDEX idx_series_name_unicode_trgm ON series USING gin(name_unicode gin_trgm_ops);
