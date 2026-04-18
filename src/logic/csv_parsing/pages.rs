/// Parse a pages string from ODS/CSV format.
///
/// Format: comma-separated entries, each either a single page or a range with `--` (double hyphen).
/// - `"123--456"` → valid range
/// - `"123"` → single page
/// - `"123--456, 789"` → two entries
/// - `"123-456"` (single hyphen) → **error**
///
/// Returns the validated, normalized pages string, or `None` if empty.
pub fn parse_pages(text: &str) -> Result<Option<String>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let entries: Vec<&str> = trimmed.split(',').collect();
    let mut normalized = Vec::with_capacity(entries.len());

    for entry in &entries {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        if entry.contains("--") {
            let parts: Vec<&str> = entry.split("--").collect();
            if parts.len() != 2 {
                return Err(format!(
                    "invalid page range (expected 'start--end'): '{entry}'"
                ));
            }
            let start = parts[0].trim();
            let end = parts[1].trim();
            if start.is_empty() {
                return Err(format!("missing start in page range: '{entry}'"));
            }
            if end.is_empty() {
                // open-ended range (e.g. "215--") — store as single page
                normalized.push(start.to_string());
            } else {
                normalized.push(format!("{start}--{end}"));
            }
        } else if entry.contains('-') {
            return Err(format!(
                "single hyphen in pages (use '--' for ranges): '{entry}'"
            ));
        } else {
            normalized.push(entry.to_string());
        }
    }

    if normalized.is_empty() {
        Ok(None)
    } else {
        Ok(Some(normalized.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_range() {
        assert_eq!(
            parse_pages("123--456").unwrap().as_deref(),
            Some("123--456")
        );
    }

    #[test]
    fn single_page() {
        assert_eq!(parse_pages("123").unwrap().as_deref(), Some("123"));
    }

    #[test]
    fn multiple_entries() {
        assert_eq!(
            parse_pages("123--456, 789").unwrap().as_deref(),
            Some("123--456, 789")
        );
    }

    #[test]
    fn roman_numerals() {
        assert_eq!(
            parse_pages("xii--xiv").unwrap().as_deref(),
            Some("xii--xiv")
        );
    }

    #[test]
    fn single_hyphen_error() {
        assert!(parse_pages("123-456").is_err());
    }

    #[test]
    fn empty_string() {
        assert_eq!(parse_pages("").unwrap(), None);
        assert_eq!(parse_pages("  ").unwrap(), None);
    }

    #[test]
    fn whitespace_normalization() {
        assert_eq!(
            parse_pages(" 123 -- 456 ").unwrap().as_deref(),
            Some("123--456")
        );
    }

    #[test]
    fn multiple_ranges() {
        assert_eq!(
            parse_pages("1--2, 10--20, 50").unwrap().as_deref(),
            Some("1--2, 10--20, 50")
        );
    }

    #[test]
    fn missing_start_is_error() {
        assert!(parse_pages("--456").is_err());
    }

    #[test]
    fn open_ended_range_uses_start() {
        assert_eq!(parse_pages("123--").unwrap().as_deref(), Some("123"));
        assert_eq!(parse_pages("xii--").unwrap().as_deref(), Some("xii"));
    }

    #[test]
    fn too_many_double_hyphens() {
        assert!(parse_pages("1--2--3").is_err());
    }
}
