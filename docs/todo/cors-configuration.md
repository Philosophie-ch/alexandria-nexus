# TODO: Configure CORS Allowed Origins

## Current State

CORS is fully permissive — `Allow-Origin: *`, `Allow-Methods: *`, `Allow-Headers: *`:

```rust
// crates/api/src/lib.rs
let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods(Any)
    .allow_headers(Any);
```

This is fine for development but should be tightened for production.

## Goal

Define allowed origins via environment configuration so CORS is restrictive in production and permissive in development.

## Implementation

### Environment variable

```env
# .env
CORS_ALLOWED_ORIGINS=https://philosophie.ch,https://www.philosophie.ch,https://alexandria.philosophie.ch
# Use "*" for development:
# CORS_ALLOWED_ORIGINS=*
```

### Code change

In `crates/api/src/lib.rs`, read origins from env and build the CORS layer accordingly:

```rust
use tower_http::cors::{CorsLayer, AllowOrigin};

fn build_cors_layer() -> CorsLayer {
    let origins_str = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "*".to_string());

    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any);

    if origins_str.trim() == "*" {
        cors.allow_origin(Any)
    } else {
        let origins: Vec<HeaderValue> = origins_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        cors.allow_origin(origins)
    }
}
```

### What to allow

| Origin | Why |
|--------|-----|
| `https://philosophie.ch` | Portal (main site) |
| `https://www.philosophie.ch` | Portal (www variant) |
| `https://alexandria.philosophie.ch` | alexandria-web frontend (see `alexandria-web.md`) |
| localhost variants | Development only (via `*`) |

Methods and headers can stay permissive (`Any`) — origin restriction is the meaningful security boundary.

## Effort

Minimal — ~15 lines of code + `.env` update.
