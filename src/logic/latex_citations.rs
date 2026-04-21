use std::collections::{HashMap, HashSet};

/// Resolved data for a single bibitem used in citation substitution.
pub struct CitationData {
    /// Display name (family name or mononym of first author; editor as fallback).
    pub author: Option<String>,
    pub year: Option<i16>,
}

// =============================================================================
// substitute_citations
// =============================================================================

/// Replace `\cite*{...}` commands in a LaTeX string with human-readable text.
///
/// - `\citet{key}` → `Author (Year)` (or `A and B (Y)` for multi-key)
/// - `\citep{key}` / `\cite{key}` → `(Author, Year)`
/// - `\citeauthor{key}` → `Author`
/// - `\citeyear{key}` → `Year`
/// - Any unresolvable or incomplete cite → empty string (nothing rendered)
pub fn substitute_citations(latex: &str, resolved: &HashMap<String, CitationData>) -> String {
    do_substitute(latex, resolved).0
}

/// Like `substitute_citations` but also returns `true` if the text contained any
/// `\cite*` command at all — resolved or not.  Any citation in a title means the
/// unicode field depends on external data and should be stored as NULL.
pub fn substitute_citations_checked(
    latex: &str,
    resolved: &HashMap<String, CitationData>,
) -> (String, bool) {
    do_substitute(latex, resolved)
}

fn do_substitute(latex: &str, resolved: &HashMap<String, CitationData>) -> (String, bool) {
    let mut result = String::with_capacity(latex.len());
    let mut rest = latex;
    let mut had_unresolvable = false;

    loop {
        let Some(cmd_pos) = rest.find("\\cite") else {
            result.push_str(rest);
            break;
        };

        result.push_str(&rest[..cmd_pos]);
        let cmd_start = &rest[cmd_pos..]; // starts at '\'

        let after_bs = &cmd_start[1..]; // skip '\'
        let name_end = after_bs
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(after_bs.len());
        let cmd_name = &after_bs[..name_end];

        let mut scan = &after_bs[name_end..];

        // optional star (e.g. \citet*)
        if let Some(s) = scan.strip_prefix('*') {
            scan = s;
        }

        // skip optional [...] args
        while let Some(s) = scan.strip_prefix('[') {
            if let Some(i) = s.find(']') {
                scan = &s[i + 1..];
            } else {
                break;
            }
        }

        let Some(after_brace) = scan.strip_prefix('{') else {
            result.push_str("\\cite");
            rest = &rest[cmd_pos + 5..];
            continue;
        };
        let Some(close) = after_brace.find('}') else {
            result.push_str("\\cite");
            rest = &rest[cmd_pos + 5..];
            continue;
        };

        let arg = &after_brace[..close];
        // bytes consumed: '\' + name + optional_star_and_brackets (cmd_start.len() - scan.len() - 1)
        // + '{' + arg + '}'
        let cmd_len = (cmd_start.len() - scan.len()) + 1 + close + 1;

        let keys: Vec<&str> = arg
            .split(',')
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .collect();

        if keys.is_empty() {
            result.push_str(&cmd_start[..cmd_len]);
        } else {
            had_unresolvable = true;
            let substituted = format_cite(cmd_name, &keys, resolved);
            result.push_str(&substituted);
        }

        rest = &rest[cmd_pos + cmd_len..];
    }

    (result, had_unresolvable)
}

fn format_cite(cmd_name: &str, keys: &[&str], resolved: &HashMap<String, CitationData>) -> String {
    match cmd_name {
        "citep" | "cite" => {
            let items: Vec<String> = keys
                .iter()
                .map(|&k| {
                    fmt_author_comma_year(resolved.get(k).unwrap_or(&CitationData {
                        author: None,
                        year: None,
                    }))
                })
                .collect();
            if items.iter().any(|s| s.is_empty()) {
                String::new()
            } else {
                format!("({})", items.join("; "))
            }
        }
        "citeauthor" => {
            let items: Vec<Option<String>> = keys
                .iter()
                .map(|&k| resolved.get(k).and_then(|d| d.author.clone()))
                .collect();
            if items.iter().any(|s| s.is_none()) {
                String::new()
            } else {
                join_and(&items.into_iter().flatten().collect::<Vec<_>>())
            }
        }
        "citeyear" => {
            let items: Vec<Option<String>> = keys
                .iter()
                .map(|&k| resolved.get(k).and_then(|d| d.year.map(|y| y.to_string())))
                .collect();
            if items.iter().any(|s| s.is_none()) {
                String::new()
            } else {
                items.into_iter().flatten().collect::<Vec<_>>().join("; ")
            }
        }
        // "citet" and any unknown variant → "Author (Year)" with "and"-joining
        _ => {
            let items: Vec<String> = keys
                .iter()
                .map(|&k| {
                    fmt_author_year(resolved.get(k).unwrap_or(&CitationData {
                        author: None,
                        year: None,
                    }))
                })
                .collect();
            if items.iter().any(|s| s.is_empty()) {
                String::new()
            } else {
                join_and(&items)
            }
        }
    }
}

fn fmt_author_year(d: &CitationData) -> String {
    match (&d.author, d.year) {
        (Some(a), Some(y)) => format!("{a} ({y})"),
        _ => String::new(),
    }
}

fn fmt_author_comma_year(d: &CitationData) -> String {
    match (&d.author, d.year) {
        (Some(a), Some(y)) => format!("{a}, {y}"),
        _ => String::new(),
    }
}

/// Join a list of strings using Oxford-"and" format (matching the old Python behaviour).
fn join_and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        _ => {
            let (head, tail) = items.split_at(items.len() - 1);
            format!("{}, and {}", head.join(", "), tail[0])
        }
    }
}

// =============================================================================
// extract_cite_keys
// =============================================================================

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
        // If a closing ']' is missing (malformed input), break to avoid an infinite loop.
        let mut after_opt = after_cmd;
        while let Some(s) = after_opt.strip_prefix('[') {
            match s.find(']') {
                Some(i) => after_opt = &s[i + 1..],
                None => break,
            }
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

    #[test]
    fn unclosed_bracket_does_not_hang() {
        // \citet[215--244{key} — missing closing ']', infinite loop without the fix
        let keys = extract_cite_keys(r"Translated in \citet[215--244{schmid_hb:2009}");
        assert!(keys.is_empty());
    }

    #[test]
    fn unclosed_bracket_with_valid_cite_after() {
        let keys = extract_cite_keys(r"\citet[no-close{bad} and \citet{good:key}");
        assert_eq!(keys, vec!["good:key"]);
    }

    // --- Malformed: unclosed/missing braces ---

    #[test]
    fn unclosed_brace_returns_empty() {
        // \citet{unclosed — no closing '}'
        assert!(extract_cite_keys(r"\citet{unclosed").is_empty());
    }

    #[test]
    fn cite_with_no_args_returns_empty() {
        // \cite immediately followed by space/end-of-string
        assert!(extract_cite_keys(r"\cite ").is_empty());
        assert!(extract_cite_keys(r"\cite").is_empty());
    }

    #[test]
    fn empty_braces_returns_empty() {
        // \citet{} — empty required arg
        assert!(extract_cite_keys(r"\citet{}").is_empty());
    }

    #[test]
    fn whitespace_only_in_braces_returns_empty() {
        assert!(extract_cite_keys(r"\citet{   }").is_empty());
    }

    #[test]
    fn empty_optional_arg_with_valid_key() {
        // \citet[]{key} — empty optional arg should still work
        assert_eq!(extract_cite_keys(r"\citet[]{key:1}"), vec!["key:1"]);
    }

    // --- Malformed: second optional arg unclosed ---

    #[test]
    fn two_optional_args_second_unclosed() {
        // \citep[pre][no-close{key} — second optional arg has no ']'
        assert!(extract_cite_keys(r"\citep[pre][no-close{key}").is_empty());
    }

    #[test]
    fn two_optional_args_both_valid() {
        // \citep[pre][post]{key} — two closed optional args
        assert_eq!(
            extract_cite_keys(r"\citep[pre][post]{key:1}"),
            vec!["key:1"]
        );
    }

    // --- Malformed: star variant edge cases ---

    #[test]
    fn star_with_unclosed_bracket() {
        // \citet*[bad{key} — star + unclosed bracket
        assert!(extract_cite_keys(r"\citet*[bad{key}").is_empty());
    }

    #[test]
    fn star_with_no_args() {
        // \citet* — star but no required arg
        assert!(extract_cite_keys(r"\citet*").is_empty());
    }

    // --- Malformed: keys with commas/whitespace ---

    #[test]
    fn key_with_leading_comma_ignored() {
        // \citet{,b:2} — leading comma → empty string filtered out
        assert_eq!(extract_cite_keys(r"\citet{,b:2}"), vec!["b:2"]);
    }

    #[test]
    fn key_with_trailing_comma_ignored() {
        // \citet{a:1,} — trailing comma → empty string filtered out
        assert_eq!(extract_cite_keys(r"\citet{a:1,}"), vec!["a:1"]);
    }

    // --- Malformed: \cite at end of string ---

    #[test]
    fn cite_at_end_of_string() {
        // String ends with backslash — no hang, no keys
        assert!(extract_cite_keys(r"text \cite").is_empty());
    }

    // --- Multiple malformed cites: only valid ones extracted ---

    #[test]
    fn multiple_malformed_with_valid_interleaved() {
        // Three malformed cites and one valid cite
        let text = r"\citet[no-close \citet{} \citet{  } \citet{real:key}";
        assert_eq!(extract_cite_keys(text), vec!["real:key"]);
    }

    // =============================================================================
    // substitute_citations malformed tests
    // =============================================================================

    fn cd(author: Option<&str>, year: Option<i16>) -> CitationData {
        CitationData {
            author: author.map(str::to_string),
            year,
        }
    }

    fn map(pairs: &[(&str, CitationData)]) -> HashMap<String, CitationData> {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    k.to_string(),
                    CitationData {
                        author: v.author.clone(),
                        year: v.year,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn substitute_no_citations() {
        let r = substitute_citations("plain text", &HashMap::new());
        assert_eq!(r, "plain text");
    }

    #[test]
    fn substitute_citet_author_year() {
        let m = map(&[("smith:2000", cd(Some("Smith"), Some(2000)))]);
        assert_eq!(
            substitute_citations(r"\citet{smith:2000}", &m),
            "Smith (2000)"
        );
    }

    #[test]
    fn substitute_citet_no_year() {
        let m = map(&[("smith:2000", cd(Some("Smith"), None))]);
        assert_eq!(substitute_citations(r"\citet{smith:2000}", &m), "");
    }

    #[test]
    fn substitute_citet_no_author() {
        let m = map(&[("x:1999", cd(None, Some(1999)))]);
        assert_eq!(substitute_citations(r"\citet{x:1999}", &m), "");
    }

    #[test]
    fn substitute_citep() {
        let m = map(&[("jones:2010", cd(Some("Jones"), Some(2010)))]);
        assert_eq!(
            substitute_citations(r"\citep{jones:2010}", &m),
            "(Jones, 2010)"
        );
    }

    #[test]
    fn substitute_cite_same_as_citep() {
        let m = map(&[("jones:2010", cd(Some("Jones"), Some(2010)))]);
        assert_eq!(
            substitute_citations(r"\cite{jones:2010}", &m),
            "(Jones, 2010)"
        );
    }

    #[test]
    fn substitute_citeauthor() {
        let m = map(&[("a:1", cd(Some("Doe"), Some(2001)))]);
        assert_eq!(substitute_citations(r"\citeauthor{a:1}", &m), "Doe");
    }

    #[test]
    fn substitute_citeyear() {
        let m = map(&[("a:1", cd(Some("Doe"), Some(2001)))]);
        assert_eq!(substitute_citations(r"\citeyear{a:1}", &m), "2001");
    }

    #[test]
    fn substitute_unknown_key_empty() {
        assert_eq!(
            substitute_citations(r"\citet{missing:key}", &HashMap::new()),
            ""
        );
    }

    #[test]
    fn substitute_multi_key_two_authors() {
        let m = map(&[
            ("a:1", cd(Some("Alpha"), Some(2000))),
            ("b:2", cd(Some("Beta"), Some(2001))),
        ]);
        assert_eq!(
            substitute_citations(r"\citet{a:1,b:2}", &m),
            "Alpha (2000) and Beta (2001)"
        );
    }

    #[test]
    fn substitute_multi_key_three_authors() {
        let m = map(&[
            ("a:1", cd(Some("A"), Some(2000))),
            ("b:2", cd(Some("B"), Some(2001))),
            ("c:3", cd(Some("C"), Some(2002))),
        ]);
        assert_eq!(
            substitute_citations(r"\citet{a:1,b:2,c:3}", &m),
            "A (2000), B (2001), and C (2002)"
        );
    }

    #[test]
    fn substitute_star_variant() {
        let m = map(&[("s:1", cd(Some("Star"), Some(1990)))]);
        assert_eq!(substitute_citations(r"\citet*{s:1}", &m), "Star (1990)");
    }

    #[test]
    fn substitute_optional_args_skipped() {
        let m = map(&[("f:1", cd(Some("Foo"), Some(2005)))]);
        assert_eq!(
            substitute_citations(r"\citep[see][p.5]{f:1}", &m),
            "(Foo, 2005)"
        );
    }

    #[test]
    fn substitute_prose_multiple_commands() {
        let m = map(&[
            ("a:1", cd(Some("Alpha"), Some(2000))),
            ("b:2", cd(Some("Beta"), Some(2001))),
        ]);
        let latex = r"See \citet{a:1} and also \citep{b:2}.";
        let result = substitute_citations(latex, &m);
        assert_eq!(result, "See Alpha (2000) and also (Beta, 2001).");
    }

    #[test]
    fn substitute_citeyear_no_year_empty() {
        let m = map(&[("a:1", cd(Some("Doe"), None))]);
        assert_eq!(substitute_citations(r"\citeyear{a:1}", &m), "");
    }

    #[test]
    fn substitute_citeauthor_no_author_empty() {
        let m = map(&[("a:1", cd(None, Some(2000)))]);
        assert_eq!(substitute_citations(r"\citeauthor{a:1}", &m), "");
    }

    #[test]
    fn substitute_citep_multi_key() {
        let m = map(&[
            ("a:1", cd(Some("Alpha"), Some(2000))),
            ("b:2", cd(Some("Beta"), Some(2001))),
        ]);
        assert_eq!(
            substitute_citations(r"\citep{a:1,b:2}", &m),
            "(Alpha, 2000; Beta, 2001)"
        );
    }

    #[test]
    fn substitute_multi_key_partial_unknown() {
        // Any missing key in a multi-key cite → entire cite renders nothing
        let m = map(&[("known:1", cd(Some("Known"), Some(2000)))]);
        assert_eq!(substitute_citations(r"\citet{known:1,missing:x}", &m), "");
    }

    #[test]
    fn substitute_citep_no_author_no_year() {
        let m = map(&[("x:1", cd(None, None))]);
        assert_eq!(substitute_citations(r"\citep{x:1}", &m), "");
    }

    #[test]
    fn substitute_unclosed_brace_left_verbatim() {
        // No closing '}' — no valid command, whole string passes through unchanged
        let result = substitute_citations(r"\citet{unclosed", &HashMap::new());
        assert_eq!(result, r"\citet{unclosed");
    }

    #[test]
    fn substitute_non_cite_command_untouched() {
        // \citealp is not a standard variant we handle specially, but must not crash
        let m = map(&[("a:1", cd(Some("A"), Some(2000)))]);
        let result = substitute_citations(r"\citealp{a:1}", &m);
        // Falls into the `_` arm (treated like \citet)
        assert_eq!(result, "A (2000)");
    }

    #[test]
    fn substitute_text_before_and_after_preserved() {
        let m = map(&[("k:1", cd(Some("K"), Some(1)))]);
        let result = substitute_citations(r"Before \citet{k:1} after.", &m);
        assert_eq!(result, "Before K (1) after.");
    }

    // --- missing data: nothing renders ---

    #[test]
    fn substitute_citet_no_author_in_prose_leaves_no_placeholder() {
        // The exact scenario: a title like "Review of \citet{gaskin_r:2008}"
        // where the cited bibitem has no author — must not leave "[key] (year)" in output
        let m = map(&[("gaskin_r:2008", cd(None, Some(2008)))]);
        let result = substitute_citations(r"Review of \citet{gaskin_r:2008}", &m);
        assert_eq!(result, "Review of ");
        assert!(!result.contains('['));
    }

    #[test]
    fn substitute_citep_no_author_empty_not_parens() {
        // Missing author in \citep → empty string, not "()"
        let m = map(&[("x:1", cd(None, Some(2001)))]);
        assert_eq!(substitute_citations(r"\citep{x:1}", &m), "");
    }

    #[test]
    fn substitute_citep_no_year_empty_not_parens() {
        let m = map(&[("x:1", cd(Some("Doe"), None))]);
        assert_eq!(substitute_citations(r"\citep{x:1}", &m), "");
    }

    #[test]
    fn substitute_multi_key_all_missing_empty() {
        // All keys in a multi-key cite have incomplete data → empty, not "(; )" or "and"
        let m = map(&[
            ("a:1", cd(None, Some(2000))),
            ("b:2", cd(Some("Beta"), None)),
        ]);
        assert_eq!(substitute_citations(r"\citet{a:1,b:2}", &m), "");
        assert_eq!(substitute_citations(r"\citep{a:1,b:2}", &m), "");
    }

    #[test]
    fn substitute_multi_key_one_complete_one_missing_author() {
        // Any key missing data → entire multi-key cite renders nothing
        let m = map(&[
            ("a:1", cd(Some("Alpha"), Some(2000))),
            ("b:2", cd(None, Some(2001))), // missing author
        ]);
        assert_eq!(substitute_citations(r"\citet{a:1,b:2}", &m), "");
    }

    #[test]
    fn substitute_citeauthor_unknown_key_empty() {
        assert_eq!(
            substitute_citations(r"\citeauthor{missing:key}", &HashMap::new()),
            ""
        );
    }

    #[test]
    fn substitute_citeyear_unknown_key_empty() {
        assert_eq!(
            substitute_citations(r"\citeyear{missing:key}", &HashMap::new()),
            ""
        );
    }

    // --- substitute_citations malformed input ---

    #[test]
    fn substitute_unclosed_bracket_does_not_hang() {
        // \citet[215--244{key} — unclosed '[', must not hang
        let result = substitute_citations(
            r"Translated in \citet[215--244{schmid_hb:2009}",
            &HashMap::new(),
        );
        // Malformed command: \cite verbatim + rest of string unchanged
        assert!(result.contains("\\cite"));
    }

    #[test]
    fn substitute_unclosed_bracket_with_valid_cite_after() {
        let m = map(&[("good:1", cd(Some("Good"), Some(2000)))]);
        let result = substitute_citations(r"\citet[no-close{bad} text \citet{good:1}", &m);
        assert!(result.contains("Good (2000)"));
    }

    #[test]
    fn substitute_empty_braces_left_verbatim() {
        // \citet{} — empty key, no substitution
        let result = substitute_citations(r"\citet{}", &HashMap::new());
        assert_eq!(result, r"\citet{}");
    }

    #[test]
    fn substitute_cite_no_args_left_verbatim() {
        // \cite with no braces — left as \cite
        let result = substitute_citations(r"text \cite more", &HashMap::new());
        assert_eq!(result, r"text \cite more");
    }

    #[test]
    fn substitute_multiple_malformed_with_valid() {
        let m = map(&[("real:1", cd(Some("Real"), Some(2001)))]);
        let text = r"\citet{} \citet[no-close \citet{real:1}";
        let result = substitute_citations(text, &m);
        assert!(result.contains("Real (2001)"));
    }
}
