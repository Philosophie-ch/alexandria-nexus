//! Alexandria Nexus — Bibliography and knowledge engine for Philosophie.ch

use hexforge::{
    Api, CorsConfig, OpenApiConfig,
    axum_exports::{Router, get},
};

mod state;

pub use state::AppState;

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

/// Build the complete application router.
///
/// CORS configuration is passed in from main.rs where environment
/// variables are read. The library layer doesn't read env vars.
pub fn build_app(pool: hexforge::DatabasePool, cors: CorsConfig) -> Router {
    let _state = AppState::new(pool);

    let api = Api::new()
        .serve_openapi(
            "/docs/openapi.json",
            OpenApiConfig::new("Alexandria Nexus", "0.1.0")
                .description("Bibliography and knowledge engine for Philosophie.ch"),
        )
        .with_cors(cors)
        .build();

    Router::new().route("/health", get(health)).merge(api)
}
