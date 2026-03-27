//! Application state — holds all data sources and shared resources.

use hexforge::DatabasePool;

/// Shared application state.
///
/// Contains database pool and will hold all data sources
/// once entities are defined.
#[derive(Clone)]
pub struct AppState {
    pub pool: DatabasePool,
}

impl AppState {
    pub fn new(pool: DatabasePool) -> Self {
        Self { pool }
    }
}
