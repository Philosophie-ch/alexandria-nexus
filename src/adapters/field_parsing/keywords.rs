/// Parse a semicolon-separated list of keyword names.
///
/// Trims whitespace and filters empty entries.
/// `"epistemology; metaphysics; "` → `["epistemology", "metaphysics"]`
pub fn parse_keyword_list(text: &str) -> Vec<String> {
    text.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        assert_eq!(
            parse_keyword_list("epistemology; metaphysics"),
            vec!["epistemology", "metaphysics"]
        );
    }

    #[test]
    fn trailing_semicolon() {
        assert_eq!(
            parse_keyword_list("epistemology; metaphysics; "),
            vec!["epistemology", "metaphysics"]
        );
    }

    #[test]
    fn single() {
        assert_eq!(parse_keyword_list("single"), vec!["single"]);
    }

    #[test]
    fn empty_string() {
        let result: Vec<String> = parse_keyword_list("");
        assert!(result.is_empty());
    }

    #[test]
    fn only_semicolons() {
        let result: Vec<String> = parse_keyword_list("; ; ;");
        assert!(result.is_empty());
    }

    #[test]
    fn whitespace_only() {
        let result: Vec<String> = parse_keyword_list("  ;  ;  ");
        assert!(result.is_empty());
    }

    #[test]
    fn preserves_internal_spaces() {
        assert_eq!(
            parse_keyword_list("philosophy of mind; philosophy of language"),
            vec!["philosophy of mind", "philosophy of language"]
        );
    }
}
