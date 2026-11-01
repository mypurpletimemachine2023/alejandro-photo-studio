//! Validate a browser save prompt's filename without granting filesystem paths.

/// Cancel (or a blank answer) keeps the document untouched. Browser downloads use only a file
/// name, not a folder path; reject portable-invalid names instead of silently retargeting work.
pub fn chosen(answer: Option<&str>) -> Result<Option<String>, &'static str> {
    let Some(name) = answer.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if name.len() > 255 {
        return Err("The filename is too long; use at most 255 UTF-8 bytes.");
    }
    if name == "." || name == ".." || name.ends_with('.') {
        return Err("Choose a filename with a name and a supported extension, such as guardian.pcraft.");
    }
    if name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')) {
        return Err("Choose a filename, not a folder path; omit / \\ : < > \" | ? * and control characters.");
    }
    Ok(Some(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::chosen;

    #[test]
    fn cancel_and_empty_are_not_save_acknowledgements() {
        for answer in [None, Some(""), Some("   ")] {
            assert_eq!(chosen(answer), Ok(None));
        }
    }

    #[test]
    fn names_and_explicit_format_choices_are_preserved() {
        for name in ["guardian.pcraft", "Existing.PSD", "Poster.psb", "Portrait v2.png", "Escultura José.pcraft", "version1.2"] {
            assert_eq!(chosen(Some(name)), Ok(Some(name.to_string())));
        }
        assert_eq!(chosen(Some("  guardian.pcraft  ")), Ok(Some("guardian.pcraft".into())));
    }

    #[test]
    fn paths_and_invalid_names_are_rejected() {
        for name in ["/tmp/guardian.pcraft", "C:\\art\\guardian.pcraft", "../guardian.pcraft", ".", "..", "guardian.", "bad\nname.pcraft", "bad:name.pcraft"] {
            assert!(chosen(Some(name)).is_err(), "{name}");
        }
        assert!(chosen(Some(&"x".repeat(256))).is_err());
    }
}
