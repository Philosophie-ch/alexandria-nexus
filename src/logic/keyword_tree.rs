//! Keyword tree types — pure data structures for hierarchical keyword organization.

use crate::domain::Keyword;

/// Keyword tree organized by level.
#[derive(Debug)]
pub struct KeywordTree {
    pub level_1: Vec<Keyword>,
    pub level_2: Vec<Keyword>,
    pub level_3: Vec<Keyword>,
}
