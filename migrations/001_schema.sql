-- Alexandria Nexus — Full Database Schema
-- Bibliography and knowledge engine for Philosophie.ch

-- =============================================================================
-- EXTENSIONS
-- =============================================================================

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- =============================================================================
-- ENUMS
-- =============================================================================

CREATE TYPE entry_type AS ENUM (
    'article', 'book', 'incollection', 'inproceedings',
    'mastersthesis', 'misc', 'phdthesis', 'proceedings',
    'techreport', 'unpublished', 'UNKNOWN'
);

CREATE TYPE pubstate AS ENUM (
    'unpub', 'forthcoming', 'inwork', 'submitted', 'published'
);

CREATE TYPE langid AS ENUM (
    'catalan', 'czech', 'danish', 'dutch', 'english', 'french',
    'greek', 'italian', 'latin', 'lithuanian', 'ngerman', 'polish',
    'portuguese', 'romanian', 'russian', 'slovak', 'spanish',
    'swedish', 'unknown'
);

CREATE TYPE epoch AS ENUM (
    'ancient-philosophy', 'ancient-scientists', 'austrian-philosophy',
    'british-idealism', 'classics', 'contemporaries', 'contemporary-scientists',
    'continental-philosophy', 'critical-theory', 'cynics', 'enlightenment',
    'existentialism', 'exotic-philosophy', 'german-idealism', 'german-rationalism',
    'gestalt-psychology', 'hermeneutics', 'islamic-philosophy', 'mathematicians',
    'medieval-philosophy', 'modern-philosophy', 'modern-scientists',
    'neo-kantianism', 'neoplatonism', 'new-realism', 'ordinary-language-philosophy',
    'phenomenology', 'polish-logic', 'pragmatism', 'presocratics', 'renaissance',
    'stoics', 'theologians', 'vienna-circle'
);

CREATE TYPE author_role AS ENUM ('author', 'editor', 'guesteditor');

CREATE TYPE ref_type AS ENUM ('further_ref', 'depends_on');

CREATE TYPE permission_level AS ENUM ('public', 'read', 'write', 'admin');

-- =============================================================================
-- AUTHENTICATION
-- =============================================================================

CREATE TABLE api_keys (
    id              BIGSERIAL PRIMARY KEY,
    key_hash        TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    permission      permission_level NOT NULL DEFAULT 'read',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at      TIMESTAMPTZ
);

CREATE INDEX idx_api_keys_active ON api_keys(key_hash) WHERE revoked_at IS NULL;

-- =============================================================================
-- ENTITY TABLES
-- =============================================================================

-- Authors
CREATE TABLE authors (
    id                      BIGSERIAL PRIMARY KEY,
    author_key              TEXT NOT NULL UNIQUE,
    -- Name components (BibStringAttr: latex, unicode, simplified)
    given_name_latex        TEXT,
    given_name_unicode      TEXT,
    given_name_simplified   TEXT,
    family_name_latex       TEXT,
    family_name_unicode     TEXT,
    family_name_simplified  TEXT,
    -- Single-name authors (Plato, Aristotle)
    mononym_latex           TEXT,
    mononym_unicode         TEXT,
    mononym_simplified      TEXT,
    -- Display shorthand
    shorthand_latex         TEXT,
    shorthand_unicode       TEXT,
    shorthand_simplified    TEXT,
    -- Famous name for profiles
    famous_name_latex       TEXT,
    famous_name_unicode     TEXT,
    famous_name_simplified  TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT author_has_name CHECK (
        family_name_latex IS NOT NULL OR mononym_latex IS NOT NULL
    )
);

CREATE INDEX idx_authors_family_name ON authors(family_name_simplified);
CREATE INDEX idx_authors_full_name ON authors(family_name_simplified, given_name_simplified);

-- Journals
CREATE TABLE journals (
    id              BIGSERIAL PRIMARY KEY,
    journal_key     TEXT NOT NULL UNIQUE,
    name_latex      TEXT NOT NULL,
    name_unicode    TEXT NOT NULL,
    name_simplified TEXT NOT NULL,
    issn_print      VARCHAR(9),
    issn_electronic VARCHAR(9),

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_journals_name ON journals(name_simplified);

-- Publishers
CREATE TABLE publishers (
    id              BIGSERIAL PRIMARY KEY,
    publisher_key   TEXT NOT NULL UNIQUE,
    name_latex      TEXT NOT NULL,
    name_unicode    TEXT NOT NULL,
    name_simplified TEXT NOT NULL,
    default_address TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_publishers_name ON publishers(name_simplified);

-- Series
CREATE TABLE series (
    id              BIGSERIAL PRIMARY KEY,
    series_key      TEXT NOT NULL UNIQUE,
    name_latex      TEXT NOT NULL,
    name_unicode    TEXT NOT NULL,
    name_simplified TEXT NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_series_name ON series(name_simplified);

-- Institutions
CREATE TABLE institutions (
    id                  BIGSERIAL PRIMARY KEY,
    institution_key     TEXT NOT NULL UNIQUE,
    name_latex          TEXT NOT NULL,
    name_unicode        TEXT NOT NULL,
    name_simplified     TEXT NOT NULL,
    default_address     TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_institutions_name ON institutions(name_simplified);

-- Schools
CREATE TABLE schools (
    id              BIGSERIAL PRIMARY KEY,
    school_key      TEXT NOT NULL UNIQUE,
    name_latex      TEXT NOT NULL,
    name_unicode    TEXT NOT NULL,
    name_simplified TEXT NOT NULL,
    default_address TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_schools_name ON schools(name_simplified);

-- Keywords (flat, level 1-3)
CREATE TABLE keywords (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    level       SMALLINT NOT NULL CHECK (level BETWEEN 1 AND 3),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(name, level)
);

CREATE INDEX idx_keywords_level ON keywords(level);
CREATE INDEX idx_keywords_name ON keywords(name);

-- =============================================================================
-- BIBITEMS (main bibliography table)
-- =============================================================================

CREATE TABLE bibitems (
    id              BIGSERIAL PRIMARY KEY,

    -- Identity
    bibkey          TEXT NOT NULL UNIQUE,
    entry_type      entry_type NOT NULL,

    -- Dates
    date_year           SMALLINT,
    date_year_2_hyphen  SMALLINT,
    date_year_2_slash   SMALLINT,
    date_month          SMALLINT,
    date_day            SMALLINT,
    date_is_no_date     BOOLEAN NOT NULL DEFAULT FALSE,
    pubstate            pubstate,

    -- Title (BibStringAttr)
    title_latex         TEXT NOT NULL,
    title_unicode       TEXT NOT NULL,
    title_simplified    TEXT NOT NULL,

    -- Booktitle (for @incollection)
    booktitle_latex     TEXT,
    booktitle_unicode   TEXT,
    booktitle_simplified TEXT,

    -- Publication info
    journal_id      BIGINT REFERENCES journals(id),
    publisher_id    BIGINT REFERENCES publishers(id),
    address         TEXT,
    volume          TEXT,
    number          TEXT,
    pages           TEXT,
    eid             TEXT,
    series_id       BIGINT REFERENCES series(id),
    edition         TEXT,

    -- Institutional
    institution_id  BIGINT REFERENCES institutions(id),
    school_id       BIGINT REFERENCES schools(id),
    type_field      TEXT,

    -- Identifiers
    doi             TEXT,
    url             TEXT,
    eprint          TEXT,
    urn             TEXT,

    -- References
    crossref_id     BIGINT REFERENCES bibitems(id),

    -- Issue/notes
    issuetitle_latex    TEXT,
    issuetitle_unicode  TEXT,
    note_latex      TEXT,
    note_unicode    TEXT,
    extra_note_latex    TEXT,
    extra_note_unicode  TEXT,

    -- Metadata
    langid          langid,
    is_translation  BOOLEAN NOT NULL DEFAULT FALSE,
    epoch           epoch,
    options         TEXT,
    shorthand       TEXT,

    -- Internal tracking
    person_id       BIGINT REFERENCES authors(id),
    has_fulltext    BOOLEAN NOT NULL DEFAULT FALSE,
    fulltext_path   TEXT,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bibitems_entry_type ON bibitems(entry_type);
CREATE INDEX idx_bibitems_year ON bibitems(date_year);
CREATE INDEX idx_bibitems_journal ON bibitems(journal_id);
CREATE INDEX idx_bibitems_publisher ON bibitems(publisher_id);
CREATE INDEX idx_bibitems_crossref ON bibitems(crossref_id);

-- =============================================================================
-- JUNCTION TABLES
-- =============================================================================

-- Bibitem-Authors (many-to-many with role and position)
CREATE TABLE bibitem_authors (
    bibitem_id  BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    author_id   BIGINT NOT NULL REFERENCES authors(id) ON DELETE RESTRICT,
    role        author_role NOT NULL DEFAULT 'author',
    position    SMALLINT NOT NULL,

    PRIMARY KEY (bibitem_id, author_id, role),
    UNIQUE (bibitem_id, role, position)
);

CREATE INDEX idx_bibitem_authors_author ON bibitem_authors(author_id);

-- Bibitem-Keywords (flat, with level)
CREATE TABLE bibitem_keywords (
    bibitem_id      BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    keyword_id      BIGINT NOT NULL REFERENCES keywords(id) ON DELETE RESTRICT,
    keyword_level   SMALLINT NOT NULL,

    PRIMARY KEY (bibitem_id, keyword_id)
);

CREATE INDEX idx_bibitem_keywords_keyword ON bibitem_keywords(keyword_id);

-- Bibitem references (further_ref, depends_on)
CREATE TABLE bibitem_refs (
    source_id   BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    target_id   BIGINT NOT NULL REFERENCES bibitems(id) ON DELETE CASCADE,
    ref_type    ref_type NOT NULL,

    PRIMARY KEY (source_id, target_id, ref_type)
);

CREATE INDEX idx_bibitem_refs_target ON bibitem_refs(target_id);

-- =============================================================================
-- WORKFLOW NOTES (JSONB for internal columns)
-- =============================================================================

CREATE TABLE bibitem_notes (
    bibitem_id  BIGINT PRIMARY KEY REFERENCES bibitems(id) ON DELETE CASCADE,
    notes       JSONB NOT NULL DEFAULT '{}'
);

-- =============================================================================
-- TRIGRAM INDEXES (for pg_trgm similarity search)
-- =============================================================================

CREATE INDEX idx_bibitems_title_trgm ON bibitems USING gin(title_simplified gin_trgm_ops);
CREATE INDEX idx_bibitems_bibkey_trgm ON bibitems USING gin(bibkey gin_trgm_ops);
CREATE INDEX idx_authors_family_trgm ON authors USING gin(family_name_simplified gin_trgm_ops);
CREATE INDEX idx_authors_given_trgm ON authors USING gin(given_name_simplified gin_trgm_ops);
CREATE INDEX idx_journals_name_trgm ON journals USING gin(name_simplified gin_trgm_ops);
CREATE INDEX idx_publishers_name_trgm ON publishers USING gin(name_simplified gin_trgm_ops);
