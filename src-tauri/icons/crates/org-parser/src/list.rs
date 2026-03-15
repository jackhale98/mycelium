use crate::cst::{Checkbox, List, ListItem};
use crate::link::parse_inline_content;

/// Parse a list from consecutive list-item lines.
/// Returns the list and number of lines consumed.
pub fn parse_list(lines: &[&str], start: usize) -> Option<(List, usize)> {
    let mut items = Vec::new();
    let mut raw = String::new();
    let mut i = start;

    while i < lines.len() {
        if let Some(item) = parse_list_item(lines[i]) {
            raw.push_str(lines[i]);
            raw.push('\n');
            items.push(item);
            i += 1;
        } else if i > start && !lines[i].trim().is_empty() && lines[i].starts_with(' ') {
            // Continuation line
            raw.push_str(lines[i]);
            raw.push('\n');
            i += 1;
        } else {
            break;
        }
    }

    if items.is_empty() {
        return None;
    }

    Some((List { items, raw }, i - start))
}

/// Parse a single list item line
fn parse_list_item(line: &str) -> Option<ListItem> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();

    let (bullet, rest) = parse_bullet(trimmed)?;

    let (checkbox, rest) = parse_checkbox(rest);
    let (tag, rest) = parse_description_tag(rest);

    let content = parse_inline_content(rest);

    Some(ListItem {
        indent,
        bullet,
        checkbox,
        tag,
        content,
        raw: line.to_string(),
    })
}

fn parse_bullet(s: &str) -> Option<(String, &str)> {
    // Unordered: "- ", "+ ", "* " (only at list level, not headline)
    for prefix in &["- ", "+ "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return Some((prefix.trim().to_string(), rest));
        }
    }

    // Ordered: "1. ", "1) "
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')') {
        if i + 1 < bytes.len() && bytes[i + 1] == b' ' {
            let bullet = &s[..=i];
            let rest = &s[i + 2..];
            return Some((bullet.to_string(), rest));
        }
    }

    None
}

fn parse_checkbox(s: &str) -> (Option<Checkbox>, &str) {
    if let Some(rest) = s.strip_prefix("[ ] ") {
        (Some(Checkbox::Unchecked), rest)
    } else if let Some(rest) = s.strip_prefix("[X] ") {
        (Some(Checkbox::Checked), rest)
    } else if let Some(rest) = s.strip_prefix("[x] ") {
        (Some(Checkbox::Checked), rest)
    } else if let Some(rest) = s.strip_prefix("[-] ") {
        (Some(Checkbox::Partial), rest)
    } else {
        (None, s)
    }
}

fn parse_description_tag(s: &str) -> (Option<String>, &str) {
    if let Some(idx) = s.find(" :: ") {
        let tag = s[..idx].to_string();
        let rest = &s[idx + 4..];
        (Some(tag), rest)
    } else {
        (None, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unordered_list() {
        let lines = vec!["- Item one", "- Item two", "- Item three"];
        let (list, count) = parse_list(&lines, 0).unwrap();
        assert_eq!(count, 3);
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.items[0].bullet, "-");
    }

    #[test]
    fn test_ordered_list() {
        let lines = vec!["1. First", "2. Second"];
        let (list, count) = parse_list(&lines, 0).unwrap();
        assert_eq!(count, 2);
        assert_eq!(list.items[0].bullet, "1.");
    }

    #[test]
    fn test_checkbox_list() {
        let lines = vec!["- [ ] Todo", "- [X] Done"];
        let (list, _) = parse_list(&lines, 0).unwrap();
        assert_eq!(list.items[0].checkbox, Some(Checkbox::Unchecked));
        assert_eq!(list.items[1].checkbox, Some(Checkbox::Checked));
    }

    #[test]
    fn test_description_list() {
        let lines = vec!["- Term :: Definition here"];
        let (list, _) = parse_list(&lines, 0).unwrap();
        assert_eq!(list.items[0].tag.as_deref(), Some("Term"));
    }
}
