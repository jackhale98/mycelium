use crate::cst::MetadataEntry;

/// Parse a file-level metadata line like "#+TITLE: My Title"
pub fn parse_metadata_line(line: &str) -> Option<MetadataEntry> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#+") {
        return None;
    }

    // Find the colon after #+KEY
    let after_hash = &trimmed[2..];
    let colon_pos = after_hash.find(':')?;

    let key = &after_hash[..colon_pos];

    // Skip known block markers
    let upper_key = key.to_uppercase();
    if upper_key.starts_with("BEGIN_") || upper_key.starts_with("END_") {
        return None;
    }

    let value = after_hash[colon_pos + 1..].trim().to_string();

    Some(MetadataEntry {
        key: key.to_string(),
        value,
        raw: line.to_string(),
    })
}

/// Extract TITLE from metadata entries
pub fn get_title(entries: &[MetadataEntry]) -> Option<&str> {
    entries
        .iter()
        .find(|e| e.key.eq_ignore_ascii_case("TITLE"))
        .map(|e| e.value.as_str())
}

/// Extract FILETAGS from metadata entries
pub fn get_filetags(entries: &[MetadataEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|e| e.key.eq_ignore_ascii_case("FILETAGS"))
        .flat_map(|e| {
            e.value
                .split(':')
                .filter(|t| !t.is_empty())
                .map(|t| t.to_string())
        })
        .collect()
}

/// Extract TODO keywords declared in the file via `#+TODO:`, `#+SEQ_TODO:` or `#+TYP_TODO:`.
/// Fast-access keys and the `|` separator are stripped, e.g. `TODO(t) | DONE(d)` -> [TODO, DONE].
pub fn get_todo_keywords(entries: &[MetadataEntry]) -> Vec<String> {
    let mut keywords = Vec::new();

    for entry in entries.iter().filter(|e| {
        e.key.eq_ignore_ascii_case("TODO")
            || e.key.eq_ignore_ascii_case("SEQ_TODO")
            || e.key.eq_ignore_ascii_case("TYP_TODO")
    }) {
        for token in entry.value.split_whitespace() {
            if token == "|" {
                continue;
            }
            let name = match token.find('(') {
                Some(idx) => &token[..idx],
                None => token,
            };
            let name = name.trim_end_matches('|');
            if name.is_empty() {
                continue;
            }
            if !keywords.iter().any(|k: &String| k == name) {
                keywords.push(name.to_string());
            }
        }
    }

    keywords
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_keywords_from_metadata() {
        let entries = vec![
            parse_metadata_line("#+TODO: SPEC(s) IMPL(i) | SHIPPED(d)").unwrap(),
            parse_metadata_line("#+SEQ_TODO: REPORT BUG | FIXED").unwrap(),
        ];
        assert_eq!(
            get_todo_keywords(&entries),
            vec!["SPEC", "IMPL", "SHIPPED", "REPORT", "BUG", "FIXED"]
        );
    }

    #[test]
    fn test_no_todo_keywords() {
        let entries = vec![parse_metadata_line("#+TITLE: Nothing").unwrap()];
        assert!(get_todo_keywords(&entries).is_empty());
    }

    #[test]
    fn test_title() {
        let entry = parse_metadata_line("#+TITLE: My Great Note").unwrap();
        assert_eq!(entry.key, "TITLE");
        assert_eq!(entry.value, "My Great Note");
    }

    #[test]
    fn test_filetags() {
        let entry = parse_metadata_line("#+FILETAGS: :rust:programming:").unwrap();
        let tags = get_filetags(&[entry]);
        assert_eq!(tags, vec!["rust", "programming"]);
    }

    #[test]
    fn test_not_metadata() {
        assert!(parse_metadata_line("Not metadata").is_none());
        assert!(parse_metadata_line("#+BEGIN_SRC python").is_none());
    }
}
