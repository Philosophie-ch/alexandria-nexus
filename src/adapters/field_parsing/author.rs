use crate::logic::full_import::{AuthorNameKey, ParsedAuthor};

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse an ` and `-separated list of authors into [`ParsedAuthor`] structs.
///
/// Each author is in one of three formats:
/// - `"Mononym"` (no comma) — e.g., "Aristotle"
/// - `"Family, Given"` (one comma) — e.g., "Kant, Immanuel"
/// - `"Family, Suffix, Given"` (two commas) — e.g., "Belnap, Jr., Nuel"
///   Suffix is merged into family name: "Belnap Jr."
pub fn parse_authors(text: &str) -> Result<Vec<ParsedAuthor>, String> {
    let trimmed = normalize_whitespace(text);
    if trimmed.is_empty() {
        return Ok(vec![]);
    }

    let parts: Vec<&str> = trimmed.split(" and ").collect();
    let mut authors = Vec::with_capacity(parts.len());

    for part in parts {
        let name = normalize_whitespace(part);
        if name.is_empty() {
            continue;
        }

        let comma_parts: Vec<&str> = name.splitn(4, ',').collect();
        let author = match comma_parts.len() {
            1 => ParsedAuthor::Mononym(name.clone()),
            2 => {
                let family = normalize_whitespace(comma_parts[0]);
                let given = normalize_whitespace(comma_parts[1]);
                if family.is_empty() {
                    return Err(format!("empty family name in author: '{name}'"));
                }
                ParsedAuthor::Named {
                    family_name: family,
                    given_name: if given.is_empty() { None } else { Some(given) },
                }
            }
            3 => {
                let family = normalize_whitespace(comma_parts[0]);
                let suffix = normalize_whitespace(comma_parts[1]);
                let given = normalize_whitespace(comma_parts[2]);
                if family.is_empty() {
                    return Err(format!("empty family name in author: '{name}'"));
                }
                let combined_family = if suffix.is_empty() {
                    family
                } else {
                    format!("{family} {suffix}")
                };
                ParsedAuthor::Named {
                    family_name: combined_family,
                    given_name: if given.is_empty() { None } else { Some(given) },
                }
            }
            _ => {
                return Err(format!(
                    "too many commas in author name (max 2 commas allowed): '{name}'"
                ));
            }
        };
        authors.push(author);
    }

    Ok(authors)
}

/// Parse the `_person` column: a single philosopher mononym, optionally with trailing `;`.
pub fn parse_person(text: &str) -> Result<Option<ParsedAuthor>, String> {
    let cleaned = normalize_whitespace(text)
        .trim_end_matches(';')
        .trim()
        .to_string();
    if cleaned.is_empty() {
        return Ok(None);
    }

    Ok(Some(ParsedAuthor::Mononym(cleaned)))
}

/// Parse a name variant string into lookup keys.
pub fn parse_variant_to_keys(variant: &str) -> Vec<AuthorNameKey> {
    if let Ok(parsed) = parse_authors(variant) {
        parsed.iter().map(AuthorNameKey::from_parsed).collect()
    } else {
        vec![AuthorNameKey::Mononym(variant.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        assert_eq!(parse_authors("").unwrap(), vec![]);
        assert_eq!(parse_authors("  ").unwrap(), vec![]);
    }

    #[test]
    fn single_family_given() {
        let result = parse_authors("Kant, Immanuel").unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0],
            ParsedAuthor::Named { family_name, given_name: Some(given) }
            if family_name == "Kant" && given == "Immanuel"
        ));
    }

    #[test]
    fn single_mononym() {
        let result = parse_authors("Aristotle").unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], ParsedAuthor::Mononym(m) if m == "Aristotle"));
    }

    #[test]
    fn multiple_authors() {
        let result = parse_authors("Kant, Immanuel and Smith, John").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].display_name(), "Kant, Immanuel");
        assert_eq!(result[1].display_name(), "Smith, John");
    }

    #[test]
    fn mixed_mononym_and_named() {
        let result = parse_authors("Kant, Immanuel and Aristotle").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].display_name(), "Kant, Immanuel");
        assert!(matches!(&result[1], ParsedAuthor::Mononym(m) if m == "Aristotle"));
    }

    #[test]
    fn suffix_handling() {
        let result = parse_authors("Belnap, Jr., Nuel").unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0],
            ParsedAuthor::Named { family_name, given_name: Some(given) }
            if family_name == "Belnap Jr." && given == "Nuel"
        ));
    }

    #[test]
    fn whitespace_normalization() {
        let result = parse_authors("  Kant ,  Immanuel  ").unwrap();
        assert_eq!(result[0].display_name(), "Kant, Immanuel");
    }

    #[test]
    fn sanderson_not_split() {
        let result = parse_authors("Sanderson, Brandon").unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].display_name(), "Sanderson, Brandon");
    }

    #[test]
    fn too_many_commas() {
        let result = parse_authors("One, Two, Three, Four");
        assert!(result.is_err());
    }

    #[test]
    fn complex_multi_author() {
        let result =
            parse_authors("Aristotle and de las Casas, Bartolomé and Tarski, Alfred and Plato")
                .unwrap();
        assert_eq!(result.len(), 4);
        assert!(matches!(&result[0], ParsedAuthor::Mononym(m) if m == "Aristotle"));
        assert_eq!(result[1].display_name(), "de las Casas, Bartolomé");
        assert_eq!(result[2].display_name(), "Tarski, Alfred");
        assert!(matches!(&result[3], ParsedAuthor::Mononym(m) if m == "Plato"));
    }

    #[test]
    fn person_with_semicolon() {
        let result = parse_person("Kierkegaard;").unwrap();
        assert_eq!(
            result,
            Some(ParsedAuthor::Mononym("Kierkegaard".to_string()))
        );
    }

    #[test]
    fn person_without_semicolon() {
        let result = parse_person("Locke").unwrap();
        assert!(matches!(result, Some(ParsedAuthor::Mononym(m)) if m == "Locke"));
    }

    #[test]
    fn person_empty() {
        assert_eq!(parse_person("").unwrap(), None);
        assert_eq!(parse_person("  ").unwrap(), None);
        assert_eq!(parse_person(";").unwrap(), None);
    }

    #[test]
    fn person_hyphenated() {
        let result = parse_person("Adam-Smith;").unwrap();
        assert!(matches!(result, Some(ParsedAuthor::Mononym(m)) if m == "Adam-Smith"));
    }
}
