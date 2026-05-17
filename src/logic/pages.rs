use std::cmp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageEntry {
    Single(String),
    Range { start: String, end: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPages(pub Vec<PageEntry>);

fn collapse_hyphens(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut consecutive = 0u32;
    for c in s.chars() {
        if c == '-' {
            consecutive += 1;
        } else {
            if consecutive >= 2 {
                result.push_str("--");
            } else if consecutive == 1 {
                result.push('-');
            }
            consecutive = 0;
            result.push(c);
        }
    }
    if consecutive >= 2 {
        result.push_str("--");
    } else if consecutive == 1 {
        result.push('-');
    }
    result
}

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

        let normalized;
        let entry = if entry.contains("---") {
            normalized = collapse_hyphens(entry);
            normalized.as_str()
        } else {
            entry
        };

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
        .or_else(|| {
            let bytes = trimmed.as_bytes();
            if bytes.len() >= 2 && bytes[0].is_ascii_uppercase() {
                trimmed[1..].parse::<i32>().ok()
            } else {
                None
            }
        })
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

// Philosophia Mathematica uses "sN-M" notation: series N, volume M.
// Encoded as series * 1000 + volume to preserve ordering (e.g. s2-4 = 2004).
fn parse_series_volume(text: &str) -> Option<i32> {
    let rest = text.strip_prefix('s')?;
    let (series_str, after_series) = rest.split_once('-')?;
    let series = series_str.parse::<i32>().ok()?;
    let volume_str = match after_series.split_once('-') {
        Some((first, _)) => first,
        None => after_series,
    };
    let volume = volume_str.parse::<i32>().ok()?;
    Some(series * 1000 + volume)
}

pub fn extract_leading_integer(text: Option<&str>) -> Option<i32> {
    let raw = text?.trim();
    if raw.is_empty() {
        return None;
    }

    if let Ok(n) = raw.parse::<i32>() {
        return Some(n);
    }

    if let Some(n) = roman_to_arabic(raw) {
        return Some(n);
    }

    if raw.starts_with('s')
        && raw.contains('-')
        && let Some(n) = parse_series_volume(raw)
    {
        return Some(n);
    }

    let stripped = raw
        .strip_prefix("suppl.")
        .map(|s| s.trim_start_matches(',').trim());
    if let Some(remainder) = stripped {
        if let Ok(n) = remainder.parse::<i32>() {
            return Some(n);
        }
        return None;
    }

    for sep in ["--", "/", ",", "."] {
        if let Some((before, _)) = raw.split_once(sep) {
            let before = before.trim();
            if let Some(n) = page_value_to_int(before) {
                return Some(n);
            }
        }
    }

    None
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
    fn parse_excess_hyphens_normalized_to_range() {
        for input in ["90---103", "90----103", "90------103"] {
            let p = parse_pages_string(input).unwrap();
            assert_eq!(
                p.0,
                vec![PageEntry::Range {
                    start: "90".to_string(),
                    end: "103".to_string()
                }],
                "Failed for input: {input}"
            );
        }
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
    fn start_page_uppercase_letter_prefix() {
        assert_eq!(compute_start_page(Some("S339--353")), Some(339));
        assert_eq!(compute_start_page(Some("C2--41")), Some(2));
    }

    #[test]
    fn start_page_lowercase_elocator_returns_none() {
        assert_eq!(compute_start_page(Some("e12936")), None);
    }

    #[test]
    fn start_page_excess_hyphens() {
        assert_eq!(compute_start_page(Some("90---103")), Some(90));
    }

    #[test]
    fn start_page_unparseable_returns_none() {
        assert_eq!(compute_start_page(Some("frontmatter")), None);
    }

    #[test]
    fn start_page_malformed_returns_none() {
        assert_eq!(compute_start_page(Some("123-456")), None);
    }

    // ── extract_leading_integer ─────────────────────────────────────────

    #[test]
    fn leading_int_none() {
        assert_eq!(extract_leading_integer(None), None);
    }

    #[test]
    fn leading_int_empty() {
        assert_eq!(extract_leading_integer(Some("")), None);
        assert_eq!(extract_leading_integer(Some("  ")), None);
    }

    #[test]
    fn leading_int_plain_integer() {
        assert_eq!(extract_leading_integer(Some("42")), Some(42));
        assert_eq!(extract_leading_integer(Some("1")), Some(1));
        assert_eq!(extract_leading_integer(Some("100")), Some(100));
    }

    #[test]
    fn leading_int_roman_numeral() {
        assert_eq!(extract_leading_integer(Some("XII")), Some(12));
        assert_eq!(extract_leading_integer(Some("III")), Some(3));
        assert_eq!(extract_leading_integer(Some("V")), Some(5));
        assert_eq!(extract_leading_integer(Some("IX")), Some(9));
    }

    #[test]
    fn leading_int_combined_slash() {
        assert_eq!(extract_leading_integer(Some("3/4")), Some(3));
        assert_eq!(extract_leading_integer(Some("38/39")), Some(38));
        assert_eq!(extract_leading_integer(Some("7/8")), Some(7));
        assert_eq!(extract_leading_integer(Some("11/12")), Some(11));
    }

    #[test]
    fn leading_int_range_double_hyphen() {
        assert_eq!(extract_leading_integer(Some("1--3")), Some(1));
        assert_eq!(extract_leading_integer(Some("21--24")), Some(21));
        assert_eq!(extract_leading_integer(Some("185--188")), Some(185));
    }

    #[test]
    fn leading_int_series_volume() {
        assert_eq!(extract_leading_integer(Some("s2-4")), Some(2004));
        assert_eq!(extract_leading_integer(Some("s1-11")), Some(1011));
        assert_eq!(extract_leading_integer(Some("s1-1")), Some(1001));
        assert_eq!(extract_leading_integer(Some("s2-6")), Some(2006));
    }

    #[test]
    fn leading_int_series_double_volume() {
        assert_eq!(extract_leading_integer(Some("s1-13-14")), Some(1013));
        assert_eq!(extract_leading_integer(Some("s1-15-16")), Some(1015));
        assert_eq!(extract_leading_integer(Some("s1-17-18")), Some(1017));
    }

    #[test]
    fn leading_int_number_with_trailing_text() {
        assert_eq!(
            extract_leading_integer(Some("10, Erg{\"a}nzungsband")),
            Some(10)
        );
    }

    #[test]
    fn leading_int_latex_prefix() {
        assert_eq!(
            extract_leading_integer(Some("1.~Foundations of the Unity of Science")),
            Some(1)
        );
    }

    #[test]
    fn leading_int_supplement_with_number() {
        assert_eq!(extract_leading_integer(Some("suppl., 2")), Some(2));
        assert_eq!(extract_leading_integer(Some("suppl., 1")), Some(1));
        assert_eq!(extract_leading_integer(Some("suppl., 3")), Some(3));
    }

    #[test]
    fn leading_int_supplement_bare() {
        assert_eq!(extract_leading_integer(Some("suppl.")), None);
    }

    #[test]
    fn leading_int_pure_text() {
        assert_eq!(extract_leading_integer(Some("special issue")), None);
        assert_eq!(extract_leading_integer(Some("s/n")), None);
    }
}
