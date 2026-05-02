use std::cmp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageEntry {
    Single(String),
    Range { start: String, end: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPages(pub Vec<PageEntry>);

pub fn parse_pages_string(text: &str) -> Result<ParsedPages, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(ParsedPages(Vec::new()));
    }

    let entries: Vec<&str> = trimmed.split(',').collect();
    let mut result = Vec::with_capacity(entries.len());

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
                result.push(PageEntry::Single(start.to_string()));
            } else {
                result.push(PageEntry::Range {
                    start: start.to_string(),
                    end: end.to_string(),
                });
            }
        } else if entry.contains('-') {
            return Err(format!(
                "single hyphen in pages (use '--' for ranges): '{entry}'"
            ));
        } else {
            result.push(PageEntry::Single(entry.to_string()));
        }
    }

    Ok(ParsedPages(result))
}

pub fn roman_to_arabic(s: &str) -> Option<i32> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }

    let value_of = |c: char| -> Option<i32> {
        match c {
            'i' => Some(1),
            'v' => Some(5),
            'x' => Some(10),
            'l' => Some(50),
            'c' => Some(100),
            'd' => Some(500),
            'm' => Some(1000),
            _ => None,
        }
    };

    let chars: Vec<char> = s.chars().collect();
    let mut total: i32 = 0;
    let mut i = 0;

    while i < chars.len() {
        let curr = value_of(chars[i])?;
        let next = if i + 1 < chars.len() {
            value_of(chars[i + 1])?
        } else {
            0
        };

        if curr < next {
            total += next - curr;
            i += 2;
        } else {
            total += curr;
            i += 1;
        }
    }

    if total == 0 { None } else { Some(total) }
}

fn page_value_to_int(s: &str) -> Option<i32> {
    let trimmed = s.trim();
    trimmed
        .parse::<i32>()
        .ok()
        .or_else(|| roman_to_arabic(trimmed))
}

pub fn compute_start_page(pages: Option<&str>) -> Option<i32> {
    let text = pages?;
    let parsed = parse_pages_string(text).ok()?;

    let mut min_page: Option<i32> = None;

    for entry in &parsed.0 {
        let value = match entry {
            PageEntry::Single(s) => page_value_to_int(s),
            PageEntry::Range { start, end } => {
                let s = page_value_to_int(start);
                let e = page_value_to_int(end);
                match (s, e) {
                    (Some(sv), Some(ev)) => Some(cmp::min(sv, ev)),
                    (Some(sv), None) => Some(sv),
                    (None, Some(ev)) => Some(ev),
                    (None, None) => None,
                }
            }
        };

        if let Some(v) = value {
            min_page = Some(min_page.map_or(v, |m| cmp::min(m, v)));
        }
    }

    min_page
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── roman_to_arabic ──────────────────────────────────────────────────

    #[test]
    fn roman_basic() {
        assert_eq!(roman_to_arabic("i"), Some(1));
        assert_eq!(roman_to_arabic("v"), Some(5));
        assert_eq!(roman_to_arabic("x"), Some(10));
        assert_eq!(roman_to_arabic("l"), Some(50));
        assert_eq!(roman_to_arabic("c"), Some(100));
        assert_eq!(roman_to_arabic("d"), Some(500));
        assert_eq!(roman_to_arabic("m"), Some(1000));
    }

    #[test]
    fn roman_subtractive() {
        assert_eq!(roman_to_arabic("iv"), Some(4));
        assert_eq!(roman_to_arabic("ix"), Some(9));
        assert_eq!(roman_to_arabic("xl"), Some(40));
        assert_eq!(roman_to_arabic("xc"), Some(90));
        assert_eq!(roman_to_arabic("cd"), Some(400));
        assert_eq!(roman_to_arabic("cm"), Some(900));
    }

    #[test]
    fn roman_compound() {
        assert_eq!(roman_to_arabic("xii"), Some(12));
        assert_eq!(roman_to_arabic("xiv"), Some(14));
        assert_eq!(roman_to_arabic("xxiii"), Some(23));
        assert_eq!(roman_to_arabic("xlii"), Some(42));
        assert_eq!(roman_to_arabic("xcix"), Some(99));
        assert_eq!(roman_to_arabic("mcmxcix"), Some(1999));
        assert_eq!(roman_to_arabic("mmxxvi"), Some(2026));
    }

    #[test]
    fn roman_case_insensitive() {
        assert_eq!(roman_to_arabic("XII"), Some(12));
        assert_eq!(roman_to_arabic("Xiv"), Some(14));
    }

    #[test]
    fn roman_invalid() {
        assert_eq!(roman_to_arabic(""), None);
        assert_eq!(roman_to_arabic("abc"), None);
        assert_eq!(roman_to_arabic("123"), None);
        assert_eq!(roman_to_arabic("xii3"), None);
    }

    // ── parse_pages_string ───────────────────────────────────────────────

    #[test]
    fn parse_single_page() {
        let p = parse_pages_string("123").unwrap();
        assert_eq!(p.0, vec![PageEntry::Single("123".to_string())]);
    }

    #[test]
    fn parse_range() {
        let p = parse_pages_string("123--456").unwrap();
        assert_eq!(
            p.0,
            vec![PageEntry::Range {
                start: "123".to_string(),
                end: "456".to_string()
            }]
        );
    }

    #[test]
    fn parse_multi_entry() {
        let p = parse_pages_string("123--456, 789").unwrap();
        assert_eq!(
            p.0,
            vec![
                PageEntry::Range {
                    start: "123".to_string(),
                    end: "456".to_string()
                },
                PageEntry::Single("789".to_string()),
            ]
        );
    }

    #[test]
    fn parse_roman_range() {
        let p = parse_pages_string("xii--xiv").unwrap();
        assert_eq!(
            p.0,
            vec![PageEntry::Range {
                start: "xii".to_string(),
                end: "xiv".to_string()
            }]
        );
    }

    #[test]
    fn parse_empty() {
        let p = parse_pages_string("").unwrap();
        assert!(p.0.is_empty());
        let p = parse_pages_string("  ").unwrap();
        assert!(p.0.is_empty());
    }

    #[test]
    fn parse_single_hyphen_error() {
        assert!(parse_pages_string("123-456").is_err());
    }

    #[test]
    fn parse_missing_start_error() {
        assert!(parse_pages_string("--456").is_err());
    }

    #[test]
    fn parse_open_ended_range() {
        let p = parse_pages_string("123--").unwrap();
        assert_eq!(p.0, vec![PageEntry::Single("123".to_string())]);
    }

    #[test]
    fn parse_too_many_hyphens() {
        assert!(parse_pages_string("1--2--3").is_err());
    }

    #[test]
    fn parse_whitespace_normalization() {
        let p = parse_pages_string(" 123 -- 456 ").unwrap();
        assert_eq!(
            p.0,
            vec![PageEntry::Range {
                start: "123".to_string(),
                end: "456".to_string()
            }]
        );
    }

    // ── compute_start_page ───────────────────────────────────────────────

    #[test]
    fn start_page_none() {
        assert_eq!(compute_start_page(None), None);
    }

    #[test]
    fn start_page_empty() {
        assert_eq!(compute_start_page(Some("")), None);
    }

    #[test]
    fn start_page_single_numeric() {
        assert_eq!(compute_start_page(Some("42")), Some(42));
    }

    #[test]
    fn start_page_range() {
        assert_eq!(compute_start_page(Some("123--456")), Some(123));
    }

    #[test]
    fn start_page_roman() {
        assert_eq!(compute_start_page(Some("xii")), Some(12));
    }

    #[test]
    fn start_page_roman_range() {
        assert_eq!(compute_start_page(Some("xii--xiv")), Some(12));
    }

    #[test]
    fn start_page_multi_entry_picks_minimum() {
        assert_eq!(compute_start_page(Some("200--300, 50")), Some(50));
    }

    #[test]
    fn start_page_mixed_roman_and_arabic() {
        assert_eq!(compute_start_page(Some("xii--xiv, 5")), Some(5));
    }

    #[test]
    fn start_page_unparseable_returns_none() {
        assert_eq!(compute_start_page(Some("frontmatter")), None);
    }

    #[test]
    fn start_page_malformed_returns_none() {
        assert_eq!(compute_start_page(Some("123-456")), None);
    }
}
