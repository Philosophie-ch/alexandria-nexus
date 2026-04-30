#![forbid(unsafe_code)]
//! Alexandria Nexus — Bibliography and knowledge engine for Philosophie.ch

pub mod adapters;
pub mod composition;
pub mod domain;
pub mod logic;
pub mod process;

// Re-export key types for external use (tests, main.rs)
pub use composition::{AppState, build_app};

// Convenience re-exports so internal crate paths and downstream code have short paths
pub use adapters::auth;
pub use adapters::db::queries;
pub use adapters::handlers;
pub use composition::state;
pub use logic::validation;
