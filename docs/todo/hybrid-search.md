# TODO: Hybrid Lexical + Semantic Search with ONNX

## Goal

Upgrade philosophie-bib's search from lexical-only (pg_trgm) to hybrid: combine trigram/tsvector lexical scoring with cosine similarity on vector embeddings. Final ranking: weighted blend of semantic and lexical scores.

Additionally, expose a **generic search index** so external systems (e.g., the portal) can push arbitrary searchable documents. philosophie-bib becomes a unified search-as-a-service layer — it stores text, embeds it, indexes it, and searches it, without knowing what the documents represent.

## Current State

- **Lexical search**: `POST /api/v1/search` uses `pg_trgm` similarity on bibitem fields
- **No embeddings**, no pgvector, no semantic search
- **No generic search index** — only bibitems are searchable

## Architecture

### Embedding model: in-process via ONNX Runtime

Use `multilingual-e5-small` (384 dimensions, 33M params) loaded in-process via the `ort` crate.

No Python sidecar needed. The model runs as a function call inside the Rust binary:

```
philosophie-bib (single binary)
├── Axum API handlers
├── ONNX Runtime (ort crate)
│   └── multilingual-e5-small.onnx (~200MB loaded)
├── Tokenizer (tokenizers crate, ~5MB)
└── PostgreSQL (pgvector + pg_trgm)
```

**Why not a Python sidecar?**
- Rust `ort` crate links prebuilt ONNX Runtime C++ lib at build time (auto-downloaded, ~50MB)
- No Python, no PyTorch, no extra container
- Same inference speed (~15ms/query)
- ~230MB total vs ~550MB for Python sidecar

**Model conversion (one-time):**
```bash
# One-time: convert HuggingFace model to ONNX
pip install optimum
optimum-cli export onnx --model intfloat/multilingual-e5-small e5-small-onnx/
# Output: model.onnx + tokenizer.json
```

The `.onnx` file and `tokenizer.json` ship with the Docker image or are mounted as a volume.

### Resource estimates

| Component | RAM |
|-----------|-----|
| Current philosophie-bib (binary + DB pool) | ~30-50 MB |
| + ONNX model loaded in memory | +~230 MB |
| + pgvector HNSW index (300K+ docs) | +~200-300 MB (in PostgreSQL) |
| **Total process** | **~300 MB** |
| **Total including PG index growth** | **~500-600 MB** |

### Generic search index

A `search_documents` table that holds searchable content from any source:

```sql
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE search_documents (
    id          BIGSERIAL PRIMARY KEY,
    source      TEXT NOT NULL,              -- opaque: 'bibitem', 'portal_page', etc.
    source_id   TEXT NOT NULL,              -- ID in origin system
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    url         TEXT NOT NULL,              -- link back to source
    locale      TEXT,                       -- 'en', 'de', 'fr', 'it'
    metadata    JSONB DEFAULT '{}',         -- opaque, type-specific
    embedding   vector(384),
    searchable  tsvector GENERATED ALWAYS AS (
        to_tsvector('simple', coalesce(title, '') || ' ' || coalesce(content, ''))
    ) STORED,
    updated_at  TIMESTAMPTZ DEFAULT now(),
    UNIQUE(source, source_id)
);

CREATE INDEX idx_search_docs_embedding ON search_documents
    USING hnsw (embedding vector_cosine_ops);
CREATE INDEX idx_search_docs_tsvector ON search_documents
    USING gin (searchable);
CREATE INDEX idx_search_docs_source ON search_documents (source);
```

**Key principle:** philosophie-bib never interprets `source`, `metadata`, or `url`. It's a generic text search engine. Consumers decide what to index and how to display results.

**Bibitems** are auto-indexed into this table internally (source = `"bibitem"`). Portal content arrives via API.

### API endpoints

```
# Index management (requires write permission)
PUT    /api/v1/search-index/{source}/{source_id}   → upsert document (embed + store)
DELETE /api/v1/search-index/{source}/{source_id}   → remove document

# Search (public)
GET    /api/v1/search?q=...&sources=...&locale=...  → hybrid search across all sources
```

**Search response:**
```json
{
  "results": [
    {
      "source": "bibitem",
      "source_id": "quine:1951",
      "title": "Two Dogmas of Empiricism",
      "url": "/bibitems/quine:1951",
      "score": 0.87,
      "snippet": "...highlighted match..."
    },
    {
      "source": "portal_page",
      "source_id": "42",
      "title": "Naturalism in Philosophy",
      "url": "/en/naturalism",
      "score": 0.82,
      "snippet": "..."
    }
  ]
}
```

### Hybrid scoring

```sql
SELECT *,
    (0.6 * (1 - (embedding <=> $1)) +
     0.4 * ts_rank_cd(searchable, plainto_tsquery('simple', $2))
    ) AS hybrid_score
FROM search_documents
WHERE searchable @@ plainto_tsquery('simple', $2)
   OR (embedding <=> $1) < 0.7
ORDER BY hybrid_score DESC
LIMIT $3
```

Uses `'simple'` text search config (no language-specific stemming) — works equally for DE/FR/IT/EN. The semantic embeddings carry multilingual understanding.

### Query flow

```
User query "Quine naturalisme"
  → philosophie-bib tokenizes + embeds query (~15ms, in-process)
  → hybrid SQL: cosine similarity + tsvector rank
  → returns ranked results from all sources
```

## Implementation plan

### Step 1: pgvector + search_documents table
- Migration: enable `vector` extension, create `search_documents`
- New DataSource implementation for search_documents

### Step 2: ONNX embedding module
- Add `ort` and `tokenizers` crates
- Load model at startup in AppState
- `embed(text: &str) -> Vec<f32>` function

### Step 3: Search index endpoints
- `PUT /search-index/{source}/{source_id}` — embed + upsert
- `DELETE /search-index/{source}/{source_id}` — remove
- Internal: auto-index bibitems on create/update

### Step 4: Hybrid search endpoint
- `GET /search?q=...` — embed query, run hybrid SQL, return results
- Optional filters: `sources`, `locale`

### Step 5: Portal integration
- Portal pushes searchable_records to philosophie-bib on create/update
- Portal search controller calls philosophie-bib search endpoint
- Dual-write period: keep existing portal search working alongside new one

## Dependencies

- `ort` crate (ONNX Runtime bindings)
- `tokenizers` crate (HuggingFace tokenizers in Rust)
- `pgvector` PostgreSQL extension
- `multilingual-e5-small` model converted to ONNX format

## Multilingual note

`multilingual-e5-small` handles DE/FR/IT/EN natively. The `'simple'` tsvector config avoids English-biased stemming. This matches the portal's multilingual content without per-language configuration.
