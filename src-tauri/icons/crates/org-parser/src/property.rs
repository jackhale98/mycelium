use crate::cst::{Property, PropertyDrawer};

/// Parse a property drawer from lines starting at ":PROPERTIES:"
/// Returns the drawer and number of lines consumed.
pub fn parse_property_drawer(lines: &[&str]) -> Option<(PropertyDrawer, usize)> {
    if lines.is_empty() || lines[0].trim() != ":PROPERTIES:" {
        return None;
    }

    let mut properties = Vec::new();
    let mut raw = String::new();
    raw.push_str(lines[0]);
    raw.push('\n');

    let mut i = 1;
    while i < lines.len() {
        let line = lines[i];
        raw.push_str(line);

        if line.trim() == ":END:" {
            raw.push('\n');
            return Some((
                PropertyDrawer {
                    properties,
                    raw,
                },
                i + 1,
            ));
        }

        if let Some(prop) = parse_property_line(line) {
            properties.push(prop);
        }

        raw.push('\n');
        i += 1;
    }

    None
}

/// Parse a single property line like ":KEY: value"
fn parse_property_line(line: &str) -> Option<Property> {
    let trimmed = line.trim();
    if !trimmed.starts_with(':') {
        return None;
    }

    let after_colon = &trimmed[1..];
    let end = after_colon.find(':')?;
    let key = &after_colon[..end];

    if key.is_empty() || key.contains(' ') {
        return None;
    }

    let value = after_colon[end + 1..].trim().to_string();

    Some(Property {
        key: key.to_string(),
        value,
        raw: line.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_drawer() {
        let lines = vec![
            ":PROPERTIES:",
            ":ID: abc-123",
            ":ROAM_ALIASES: \"Alias One\" \"Alias Two\"",
            ":END:",
        ];
        let (drawer, count) = parse_property_drawer(&lines).unwrap();
        assert_eq!(count, 4);
        assert_eq!(drawer.properties.len(), 2);
        assert_eq!(drawer.properties[0].key, "ID");
        assert_eq!(drawer.properties[0].value, "abc-123");
        assert_eq!(drawer.properties[1].key, "ROAM_ALIASES");
    }

    #[test]
    fn test_no_properties() {
        let lines = vec!["Some text"];
        assert!(parse_property_drawer(&lines).is_none());
    }

    #[test]
    fn test_property_line() {
        let prop = parse_property_line(":ID: my-uuid-here").unwrap();
        assert_eq!(prop.key, "ID");
        assert_eq!(prop.value, "my-uuid-here");
    }
}
