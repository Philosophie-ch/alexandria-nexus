use std::collections::HashSet;

/// Extract all unique bibkeys referenced by `\cite*{...}` commands in a LaTeX string.
///
/// Handles `\citet`, `\citep`, `\cite`, `\citeauthor`, `\citeyear` and any other
/// `\cite`-prefixed command. Multi-key arguments (`\citet{a,b}`) are split on commas.
/// Returns deduplicated keys in order of first appearance.
pub fn extract_cite_keys(latex: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut keys = Vec::new();

    let mut rest = latex;
    while let Some(pos) = rest.find("\\cite") {
        rest = &rest[pos + 1..]; // step past the backslash

        // skip the command name (letters only)
        let after_cmd = rest.trim_start_matches(|c: char| c.is_ascii_alphabetic());

        // optional star (e.g. \citet*)
        let after_cmd = after_cmd.strip_prefix('*').unwrap_or(after_cmd);

        // skip all optional [...] arguments (cite commands can have two: [prenote][postnote])
        let mut after_opt = after_cmd;
        while let Some(s) = after_opt.strip_prefix('[') {
            after_opt = s.find(']').map(|i| &s[i + 1..]).unwrap_or(after_opt);
        }

        // required {key} argument
        let Some(brace_start) = after_opt.strip_prefix('{') else {
            continue;
        };
        let Some(close) = brace_start.find('}') else {
            continue;
        };
        let arg = &brace_start[..close];

        for raw in arg.split(',') {
            let key = raw.trim().to_string();
            if !key.is_empty() && seen.insert(key.clone()) {
                keys.push(key);
            }
        }
    }

    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        assert!(extract_cite_keys("").is_empty());
    }

    #[test]
    fn no_citations() {
        assert!(extract_cite_keys("plain text without commands").is_empty());
    }

    #[test]
    fn single_citet() {
        assert_eq!(extract_cite_keys(r"\citet{smith:2000}"), vec!["smith:2000"]);
    }

    #[test]
    fn single_citep() {
        assert_eq!(extract_cite_keys(r"\citep{jones:1990}"), vec!["jones:1990"]);
    }

    #[test]
    fn plain_cite() {
        assert_eq!(extract_cite_keys(r"\cite{doe:2010}"), vec!["doe:2010"]);
    }

    #[test]
    fn citeauthor_and_citeyear() {
        let latex = r"\citeauthor{a:1} and \citeyear{b:2}";
        assert_eq!(extract_cite_keys(latex), vec!["a:1", "b:2"]);
    }

    #[test]
    fn multi_key_arg() {
        let keys = extract_cite_keys(r"\citet{a:1,b:2,c:3}");
        assert_eq!(keys, vec!["a:1", "b:2", "c:3"]);
    }

    #[test]
    fn multi_key_with_spaces() {
        let keys = extract_cite_keys(r"\citet{a:1, b:2 , c:3}");
        assert_eq!(keys, vec!["a:1", "b:2", "c:3"]);
    }

    #[test]
    fn deduplication_preserves_order() {
        let keys = extract_cite_keys(r"\citet{a:1} \citep{b:2} \cite{a:1}");
        assert_eq!(keys, vec!["a:1", "b:2"]);
    }

    #[test]
    fn cite_star_variant() {
        assert_eq!(
            extract_cite_keys(r"\citet*{smith:2000}"),
            vec!["smith:2000"]
        );
    }

    #[test]
    fn optional_argument_skipped() {
        assert_eq!(
            extract_cite_keys(r"\citep[see][p.~5]{foo:bar}"),
            vec!["foo:bar"]
        );
    }

    #[test]
    fn multiple_commands_in_prose() {
        let latex = r"As shown by \citet{a:1} and later confirmed \citep{b:2,c:3}.";
        assert_eq!(extract_cite_keys(latex), vec!["a:1", "b:2", "c:3"]);
    }
}
