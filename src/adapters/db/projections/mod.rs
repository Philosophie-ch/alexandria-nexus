//! Projections — typed column subsets for list and search views.
//!
//! Each projection uses `#[derive(Projection)]` with compile-time validation
//! that every field exists in the parent entity with the same type.

mod bibitem;

pub use bibitem::BibItemSummary;
