use super::types::{BibkeyDate, ParsedBibkey};

/// Parse and structurally validate a bibkey.
///
/// Format: `"author_part:date_part"`
/// - Author part: `"first"` or `"first-other"` (max one hyphen)
/// - Date part: digits (year) with optional suffix, or `"unpub"`/`"forthcoming"` with optional `-suffix`
pub fn parse_bibkey(text: &str) -> Result<ParsedBibkey, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("empty bibkey".to_string());
    }

    let colon_parts: Vec<&str> = trimmed.splitn(3, ':').collect();
    if colon_parts.len() != 2 {
        return Err(format!(
            "bibkey must contain exactly one colon: '{trimmed}'"
        ));
    }

    let author_part = colon_parts[0];
    let date_part = colon_parts[1];

    if author_part.is_empty() {
        return Err(format!("empty author part in bibkey: '{trimmed}'"));
    }
    if date_part.is_empty() {
        return Err(format!("empty date part in bibkey: '{trimmed}'"));
    }

    // Parse author part: split on '-' (max 2 parts)
    let (first_author, other_authors) = parse_bibkey_author(author_part, trimmed)?;

    // Parse date part
    let (date, suffix) = parse_bibkey_date(date_part)?;

    Ok(ParsedBibkey {
        full: trimmed.to_string(),
        first_author,
        other_authors,
        date,
        suffix,
    })
}

fn parse_bibkey_author(
    author_part: &str,
    full_bibkey: &str,
) -> Result<(String, Option<String>), String> {
    // Author part allows underscores and special chars but uses '-' to separate
    // first author from other authors. However, author names can contain hyphens
    // (e.g., "bordogarcia_l-olivadoti_s"), so we only split on '-' at the
    // "word boundary" level. The Python SDK splits naively on '-', max 2 parts.
    // We follow the same: split on first '-' only, yielding at most 2 parts.
    //
    // Actually, looking at test cases like "aristotle-plato:300a", the split is naive.
    // But "bordogarcia_l-olivadoti_s" also has one hyphen and works as 2-author.
    // The key constraint is: at most 2 parts after splitting on '-'.

    // Count hyphens to detect >2 parts scenario
    let hyphen_count = author_part.chars().filter(|&c| c == '-').count();

    if hyphen_count == 0 {
        Ok((author_part.to_string(), None))
    } else if hyphen_count == 1 {
        let parts: Vec<&str> = author_part.splitn(2, '-').collect();
        if parts[0].is_empty() || parts[1].is_empty() {
            return Err(format!("empty author component in bibkey: '{full_bibkey}'"));
        }
        Ok((parts[0].to_string(), Some(parts[1].to_string())))
    } else {
        // More than 1 hyphen — could still be valid if the names contain hyphens.
        // We split on the first hyphen only (same as Python SDK splitn behavior).
        let parts: Vec<&str> = author_part.splitn(2, '-').collect();
        Ok((parts[0].to_string(), Some(parts[1].to_string())))
    }
}

fn parse_bibkey_date(date_part: &str) -> Result<(BibkeyDate, String), String> {
    // Handle negative years: leading '-'
    let (is_negative, rest) = if let Some(stripped) = date_part.strip_prefix('-') {
        (true, stripped)
    } else {
        (false, date_part)
    };

    // Scan for leading digits
    let digit_end = rest
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, c)| i + c.len_utf8());

    match digit_end {
        Some(end) => {
            // Found digits: parse year and extract suffix
            let year_str = &rest[..end];
            let suffix = rest[end..].to_string();
            let year: i64 = year_str
                .parse()
                .map_err(|_| format!("invalid year in bibkey date: '{date_part}'"))?;
            let year = if is_negative { -year } else { year };
            let year_i16 =
                i16::try_from(year).map_err(|_| format!("year out of range in bibkey: {year}"))?;
            Ok((BibkeyDate::Year(year_i16), suffix))
        }
        None => {
            // No leading digits: must be "unpub" or "forthcoming"
            if is_negative {
                return Err(format!("invalid bibkey date: '{date_part}'"));
            }

            // Split on first '-' for suffix
            if let Some(hyphen_pos) = rest.find('-') {
                let word = &rest[..hyphen_pos];
                let suffix = &rest[hyphen_pos + 1..];
                if suffix.is_empty() {
                    return Err(format!(
                        "empty suffix after '-' in bibkey date: '{date_part}'"
                    ));
                }
                match word {
                    "unpub" => Ok((BibkeyDate::Unpub, suffix.to_string())),
                    "forthcoming" => Ok((BibkeyDate::Forthcoming, suffix.to_string())),
                    _ => Err(format!("invalid bibkey date: '{date_part}'")),
                }
            } else {
                match rest {
                    "unpub" => Ok((BibkeyDate::Unpub, String::new())),
                    "forthcoming" => Ok((BibkeyDate::Forthcoming, String::new())),
                    _ => Err(format!("invalid bibkey date: '{date_part}'")),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_bibkey() {
        let bk = parse_bibkey("kant:1781").unwrap();
        assert_eq!(bk.first_author, "kant");
        assert_eq!(bk.other_authors, None);
        assert_eq!(bk.date, BibkeyDate::Year(1781));
        assert_eq!(bk.suffix, "");
    }

    #[test]
    fn with_suffix() {
        let bk = parse_bibkey("kant:1781a").unwrap();
        assert_eq!(bk.date, BibkeyDate::Year(1781));
        assert_eq!(bk.suffix, "a");
    }

    #[test]
    fn two_authors() {
        let bk = parse_bibkey("kant-smith:2024").unwrap();
        assert_eq!(bk.first_author, "kant");
        assert_eq!(bk.other_authors.as_deref(), Some("smith"));
        assert_eq!(bk.date, BibkeyDate::Year(2024));
    }

    #[test]
    fn unpub() {
        let bk = parse_bibkey("smith:unpub").unwrap();
        assert_eq!(bk.date, BibkeyDate::Unpub);
        assert_eq!(bk.suffix, "");
    }

    #[test]
    fn forthcoming() {
        let bk = parse_bibkey("smith:forthcoming").unwrap();
        assert_eq!(bk.date, BibkeyDate::Forthcoming);
        assert_eq!(bk.suffix, "");
    }

    #[test]
    fn forthcoming_with_suffix() {
        let bk = parse_bibkey("smith:forthcoming-a").unwrap();
        assert_eq!(bk.date, BibkeyDate::Forthcoming);
        assert_eq!(bk.suffix, "a");
    }

    #[test]
    fn unpub_with_suffix() {
        let bk = parse_bibkey("smith:unpub-1").unwrap();
        assert_eq!(bk.date, BibkeyDate::Unpub);
        assert_eq!(bk.suffix, "1");
    }

    #[test]
    fn negative_year() {
        let bk = parse_bibkey("plato:-380").unwrap();
        assert_eq!(bk.date, BibkeyDate::Year(-380));
        assert_eq!(bk.suffix, "");
    }

    #[test]
    fn negative_year_with_suffix() {
        let bk = parse_bibkey("plato:-380a").unwrap();
        assert_eq!(bk.date, BibkeyDate::Year(-380));
        assert_eq!(bk.suffix, "a");
    }

    #[test]
    fn complex_author_names() {
        let bk = parse_bibkey("bordogarcia_l-olivadoti_s:2027z2").unwrap();
        assert_eq!(bk.first_author, "bordogarcia_l");
        assert_eq!(bk.other_authors.as_deref(), Some("olivadoti_s"));
        assert_eq!(bk.date, BibkeyDate::Year(2027));
        assert_eq!(bk.suffix, "z2");
    }

    #[test]
    fn full_bibkey_string() {
        let bk = parse_bibkey("kant:1781").unwrap();
        assert_eq!(bk.full, "kant:1781");
    }

    #[test]
    fn no_colon() {
        assert!(parse_bibkey("nocolon").is_err());
    }

    #[test]
    fn too_many_colons() {
        assert!(parse_bibkey("too:many:colons").is_err());
    }

    #[test]
    fn empty_author() {
        assert!(parse_bibkey(":1781").is_err());
    }

    #[test]
    fn empty_date() {
        assert!(parse_bibkey("kant:").is_err());
    }

    #[test]
    fn empty_string() {
        assert!(parse_bibkey("").is_err());
    }

    #[test]
    fn invalid_date_word() {
        assert!(parse_bibkey("kant:garbage").is_err());
    }

    #[test]
    fn forthcoming_empty_suffix() {
        assert!(parse_bibkey("smith:forthcoming-").is_err());
    }
}
