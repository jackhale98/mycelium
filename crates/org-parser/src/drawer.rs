use crate::cst::Drawer;

/// Parse a drawer (e.g. :LOGBOOK: ... :END:) from lines.
/// Note: Property drawers are handled separately in property.rs.
/// Returns the drawer and number of lines consumed.
pub fn parse_drawer(lines: &[&str], start: usize) -> Option<(Drawer, usize)> {
    let line = lines.get(start)?;
    let trimmed = line.trim();

    if !trimmed.starts_with(':') || !trimmed.ends_with(':') || trimmed.len() < 3 {
        return None;
    }

    let name = &trimmed[1..trimmed.len() - 1];

    // Skip PROPERTIES (handled elsewhere) and END
    if name.eq_ignore_ascii_case("PROPERTIES") || name.eq_ignore_ascii_case("END") {
        return None;
    }

    // Must be a valid drawer name (alphanumeric + hyphens + underscores)
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    let mut raw = String::new();
    raw.push_str(line);
    raw.push('\n');

    let mut contents = String::new();
    let mut i = start + 1;

    while i < lines.len() {
        let current = lines[i];
        raw.push_str(current);

        if current.trim() == ":END:" {
            raw.push('\n');
            return Some((
                Drawer {
                    name: name.to_string(),
                    contents,
                    raw,
                },
                i - start + 1,
            ));
        }

        contents.push_str(current);
        contents.push('\n');
        raw.push('\n');
        i += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logbook_drawer() {
        let lines = vec![
            ":LOGBOOK:",
            "- State \"DONE\" from \"TODO\" [2024-01-15 Mon 10:00]",
            ":END:",
        ];
        let (drawer, count) = parse_drawer(&lines, 0).unwrap();
        assert_eq!(count, 3);
        assert_eq!(drawer.name, "LOGBOOK");
        assert!(drawer.contents.contains("DONE"));
    }

    #[test]
    fn test_not_a_drawer() {
        assert!(parse_drawer(&[":PROPERTIES:"], 0).is_none());
        assert!(parse_drawer(&["Not a drawer"], 0).is_none());
    }
}
