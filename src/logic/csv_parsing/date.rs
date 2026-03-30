use super::types::{DateRangeSeparator, ParsedDate};

/// Parse a date string from ODS/CSV format.
///
/// Supported formats:
/// - `""` or `"no date"` (case-insensitive) → [`ParsedDate::NoDate`]
/// - `"YYYY"` → [`ParsedDate::Year`]
/// - `"YYYY-YYYY"` → [`ParsedDate::YearRange`] with hyphen separator
/// - `"YYYY/YYYY"` → [`ParsedDate::YearRange`] with slash separator
/// - `"YYYY-MM-DD"` → [`ParsedDate::FullDate`]
///
/// Validates: year abs ≤ 9999, month 1–12, day 1–31.
pub fn parse_date(text: &str) -> Result<ParsedDate, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(ParsedDate::NoDate);
    }

    if trimmed.eq_ignore_ascii_case("no date") {
        return Ok(ParsedDate::NoDate);
    }

    // Slash-separated range: "YYYY/YYYY"
    if trimmed.contains('/') {
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() != 2 {
            return Err(format!("invalid date format: '{trimmed}'"));
        }
        let year = parse_year(parts[0])?;
        let year2 = parse_year(parts[1])?;
        return Ok(ParsedDate::YearRange {
            year,
            year2,
            separator: DateRangeSeparator::Slash,
        });
    }

    // Check if it contains hyphens (but first handle negative years)
    if trimmed.contains('-') {
        return parse_hyphenated(trimmed);
    }

    // Single year
    let year = parse_year(trimmed)?;
    Ok(ParsedDate::Year(year))
}

fn parse_hyphenated(text: &str) -> Result<ParsedDate, String> {
    // Split carefully: the first character might be '-' for negative years.
    // Strategy: if starts with '-', strip it, split on '-', then negate first part.
    let (is_negative, rest) = if let Some(stripped) = text.strip_prefix('-') {
        (true, stripped)
    } else {
        (false, text)
    };

    let parts: Vec<&str> = rest.split('-').collect();

    match parts.len() {
        1 => {
            // Negative year only: "-380"
            if is_negative {
                let year = negate_year(parse_year_raw(parts[0])?)?;
                Ok(ParsedDate::Year(year))
            } else {
                Err(format!("invalid date format: '{text}'"))
            }
        }
        2 => {
            if is_negative {
                let year = negate_year(parse_year_raw(parts[0])?)?;
                let year2 = parse_year(parts[1])?;
                Ok(ParsedDate::YearRange {
                    year,
                    year2,
                    separator: DateRangeSeparator::Hyphen,
                })
            } else {
                // "YYYY-YYYY" range
                let year = parse_year(parts[0])?;
                let year2 = parse_year(parts[1])?;
                Ok(ParsedDate::YearRange {
                    year,
                    year2,
                    separator: DateRangeSeparator::Hyphen,
                })
            }
        }
        3 => {
            // Could be "YYYY-MM-DD" (with or without negative year prefix)
            let year_str = if is_negative {
                format!("-{}", parts[0])
            } else {
                parts[0].to_string()
            };
            let year = parse_year(&year_str)?;

            // Month and day parts should be short (1-2 chars)
            if parts[1].len() <= 2 && parts[2].len() <= 2 {
                let month: i16 = parts[1]
                    .parse()
                    .map_err(|_| format!("invalid month: '{}'", parts[1]))?;
                let day: i16 = parts[2]
                    .parse()
                    .map_err(|_| format!("invalid day: '{}'", parts[2]))?;
                validate_month(month)?;
                validate_day(day)?;
                Ok(ParsedDate::FullDate { year, month, day })
            } else {
                Err(format!(
                    "invalid date format: '{}'",
                    if is_negative {
                        format!("-{}", parts.join("-"))
                    } else {
                        parts.join("-")
                    }
                ))
            }
        }
        _ => Err(format!("invalid date format: '{text}'")),
    }
}

fn parse_year(s: &str) -> Result<i16, String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err("empty year".to_string());
    }
    let val: i64 = trimmed
        .parse()
        .map_err(|_| format!("invalid year: '{trimmed}'"))?;
    if val.unsigned_abs() > 9999 {
        return Err(format!("year out of range: {val}"));
    }
    i16::try_from(val).map_err(|_| format!("year out of range: {val}"))
}

fn parse_year_raw(s: &str) -> Result<i16, String> {
    let val: i64 = s.parse().map_err(|_| format!("invalid year: '{s}'"))?;
    if val.unsigned_abs() > 9999 {
        return Err(format!("year out of range: {val}"));
    }
    i16::try_from(val).map_err(|_| format!("year out of range: {val}"))
}

fn negate_year(year: i16) -> Result<i16, String> {
    year.checked_neg()
        .ok_or_else(|| format!("year out of range: -{year}"))
}

fn validate_month(month: i16) -> Result<(), String> {
    if !(1..=12).contains(&month) {
        Err(format!("month out of range (1-12): {month}"))
    } else {
        Ok(())
    }
}

fn validate_day(day: i16) -> Result<(), String> {
    if !(1..=31).contains(&day) {
        Err(format!("day out of range (1-31): {day}"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_only() {
        assert_eq!(parse_date("2024").unwrap(), ParsedDate::Year(2024));
        assert_eq!(parse_date("1781").unwrap(), ParsedDate::Year(1781));
    }

    #[test]
    fn no_date() {
        assert_eq!(parse_date("no date").unwrap(), ParsedDate::NoDate);
        assert_eq!(parse_date("NO DATE").unwrap(), ParsedDate::NoDate);
        assert_eq!(parse_date("No Date").unwrap(), ParsedDate::NoDate);
    }

    #[test]
    fn empty_string() {
        assert_eq!(parse_date("").unwrap(), ParsedDate::NoDate);
        assert_eq!(parse_date("  ").unwrap(), ParsedDate::NoDate);
    }

    #[test]
    fn year_range_hyphen() {
        assert_eq!(
            parse_date("2021-2022").unwrap(),
            ParsedDate::YearRange {
                year: 2021,
                year2: 2022,
                separator: DateRangeSeparator::Hyphen,
            }
        );
    }

    #[test]
    fn year_range_slash() {
        assert_eq!(
            parse_date("2021/2022").unwrap(),
            ParsedDate::YearRange {
                year: 2021,
                year2: 2022,
                separator: DateRangeSeparator::Slash,
            }
        );
    }

    #[test]
    fn full_date() {
        assert_eq!(
            parse_date("2021-01-15").unwrap(),
            ParsedDate::FullDate {
                year: 2021,
                month: 1,
                day: 15,
            }
        );
    }

    #[test]
    fn negative_year() {
        assert_eq!(parse_date("-380").unwrap(), ParsedDate::Year(-380));
    }

    #[test]
    fn invalid_format() {
        assert!(parse_date("202x").is_err());
        assert!(parse_date("not-a-date").is_err());
    }

    #[test]
    fn invalid_month() {
        assert!(parse_date("2024-13-01").is_err());
        assert!(parse_date("2024-0-01").is_err());
    }

    #[test]
    fn invalid_day() {
        assert!(parse_date("2024-01-32").is_err());
        assert!(parse_date("2024-01-0").is_err());
    }

    #[test]
    fn year_out_of_range() {
        assert!(parse_date("10000").is_err());
        assert!(parse_date("-10000").is_err());
    }

    #[test]
    fn whitespace_handling() {
        assert_eq!(parse_date("  2024  ").unwrap(), ParsedDate::Year(2024));
    }
}
