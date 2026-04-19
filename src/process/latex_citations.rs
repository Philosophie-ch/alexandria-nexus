//! Citation pre-compilation — process layer.
//!
//! Defines `CitationResolver` and `pre_compile_citations`, which resolve `\cite*{...}`
//! commands in LaTeX strings before they are handed to `pylatexenc`.
//!
//! Must run BEFORE the LaTeX → Unicode conversion step so that citation commands
//! become plain text rather than being mis-parsed by `pylatexenc`.

use std::collections::{HashMap, HashSet};
use std::future::Future;

use hexforge::HexforgeError;

use crate::logic::latex_citations::{CitationData, extract_cite_keys, substitute_citations};

/// Contract for batch-resolving bibkeys to `CitationData`.
pub trait CitationResolver: Send + Sync {
    fn resolve_bibkeys(
        &self,
        keys: &[String],
    ) -> impl Future<Output = Result<HashMap<String, CitationData>, HexforgeError>> + Send;
}

/// Pre-compile `\cite*{...}` commands in a slice of LaTeX strings.
///
/// Collects all unique cite keys across all texts, resolves them in one DB call,
/// then substitutes in every text. Returns `(pre_processed_texts, missing_bibkeys)`.
///
/// Missing bibkeys are rendered as `[bibkey]` placeholders in the output and returned
/// as the second element so the caller can surface them in the conversion report.
pub async fn pre_compile_citations(
    texts: &[String],
    resolver: &impl CitationResolver,
) -> Result<(Vec<String>, Vec<String>), HexforgeError> {
    // Collect all unique cite keys across all texts
    let mut all_keys: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for text in texts {
        for key in extract_cite_keys(text) {
            if seen.insert(key.clone()) {
                all_keys.push(key);
            }
        }
    }

    if all_keys.is_empty() {
        return Ok((texts.to_vec(), Vec::new()));
    }

    let resolved = resolver.resolve_bibkeys(&all_keys).await?;

    let missing: Vec<String> = all_keys
        .into_iter()
        .filter(|k| !resolved.contains_key(k))
        .collect();

    let processed: Vec<String> = texts
        .iter()
        .map(|t| substitute_citations(t, &resolved))
        .collect();

    Ok((processed, missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockResolver(HashMap<String, CitationData>);

    impl CitationResolver for MockResolver {
        async fn resolve_bibkeys(
            &self,
            keys: &[String],
        ) -> Result<HashMap<String, CitationData>, HexforgeError> {
            let result = keys
                .iter()
                .filter_map(|k| {
                    self.0.get(k).map(|d| {
                        (
                            k.clone(),
                            CitationData {
                                author: d.author.clone(),
                                year: d.year,
                            },
                        )
                    })
                })
                .collect();
            Ok(result)
        }
    }

    fn cd(author: &str, year: i16) -> CitationData {
        CitationData {
            author: Some(author.to_string()),
            year: Some(year),
        }
    }

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[tokio::test]
    async fn empty_texts_no_db_call() {
        // MockResolver with no entries — if it were called it would return empty, but
        // the fast-path should skip it entirely
        let resolver = MockResolver(HashMap::new());
        let (texts, missing) = pre_compile_citations(&[], &resolver).await.unwrap();
        assert!(texts.is_empty());
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn no_cite_keys_returns_texts_unchanged() {
        let resolver = MockResolver(HashMap::new());
        let input = vec![s("plain text"), s("no citations here")];
        let (texts, missing) = pre_compile_citations(&input, &resolver).await.unwrap();
        assert_eq!(texts, input);
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn all_keys_resolved_no_missing() {
        let mut db = HashMap::new();
        db.insert(s("smith:2000"), cd("Smith", 2000));
        let resolver = MockResolver(db);
        let input = vec![s(r"\citet{smith:2000}")];
        let (texts, missing) = pre_compile_citations(&input, &resolver).await.unwrap();
        assert_eq!(texts, vec!["Smith (2000)"]);
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn missing_key_reported_and_placeholder_in_output() {
        let resolver = MockResolver(HashMap::new());
        let input = vec![s(r"See \citet{ghost:1999}.")];
        let (texts, missing) = pre_compile_citations(&input, &resolver).await.unwrap();
        assert_eq!(texts, vec!["See [ghost:1999]."]);
        assert_eq!(missing, vec!["ghost:1999"]);
    }

    #[tokio::test]
    async fn partial_missing_only_unknown_keys_reported() {
        let mut db = HashMap::new();
        db.insert(s("known:2000"), cd("Known", 2000));
        let resolver = MockResolver(db);
        let input = vec![s(r"\citet{known:2000} and \citet{missing:x}")];
        let (texts, missing) = pre_compile_citations(&input, &resolver).await.unwrap();
        assert_eq!(texts, vec!["Known (2000) and [missing:x]"]);
        assert_eq!(missing, vec!["missing:x"]);
    }

    #[tokio::test]
    async fn same_key_across_multiple_texts_resolved_consistently() {
        let mut db = HashMap::new();
        db.insert(s("a:1"), cd("A", 2001));
        let resolver = MockResolver(db);
        let input = vec![s(r"First \citet{a:1}."), s(r"Second \citet{a:1} again.")];
        let (texts, missing) = pre_compile_citations(&input, &resolver).await.unwrap();
        assert_eq!(texts[0], "First A (2001).");
        assert_eq!(texts[1], "Second A (2001) again.");
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn missing_keys_deduplicated_across_texts() {
        // Same missing key in two texts → appears only once in the missing list
        let resolver = MockResolver(HashMap::new());
        let input = vec![s(r"\citet{ghost:1}"), s(r"\citet{ghost:1} again")];
        let (_, missing) = pre_compile_citations(&input, &resolver).await.unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "ghost:1");
    }

    #[tokio::test]
    async fn multiple_missing_keys_all_reported() {
        let resolver = MockResolver(HashMap::new());
        let input = vec![s(r"\citet{a:1} \citet{b:2} \citet{c:3}")];
        let (texts, missing) = pre_compile_citations(&input, &resolver).await.unwrap();
        assert_eq!(texts, vec!["[a:1] [b:2] [c:3]"]);
        assert_eq!(missing.len(), 3);
        assert!(missing.contains(&s("a:1")));
        assert!(missing.contains(&s("b:2")));
        assert!(missing.contains(&s("c:3")));
    }
}
