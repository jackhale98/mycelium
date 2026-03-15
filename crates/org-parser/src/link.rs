use crate::cst::{InlineContent, Link, LinkType};

/// Parse all inline content from a string, recognizing links, markup, etc.
pub fn parse_inline_content(s: &str) -> Vec<InlineContent> {
    let mut result = Vec::new();
    let mut current_text = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Check for org link: [[...]]
        if i + 1 < len && chars[i] == '[' && chars[i + 1] == '[' {
            if let Some((link, end)) = parse_org_link(&chars, i) {
                if !current_text.is_empty() {
                    result.push(InlineContent::Text(std::mem::take(&mut current_text)));
                }
                result.push(InlineContent::Link(link));
                i = end;
                continue;
            }
        }

        // Check for markup: *bold*, /italic/, _underline_, +strikethrough+, ~code~, =verbatim=
        if let Some((content, end)) = try_parse_markup(&chars, i) {
            if !current_text.is_empty() {
                result.push(InlineContent::Text(std::mem::take(&mut current_text)));
            }
            result.push(content);
            i = end;
            continue;
        }

        current_text.push(chars[i]);
        i += 1;
    }

    if !current_text.is_empty() {
        result.push(InlineContent::Text(current_text));
    }

    // If empty input, return empty vec
    result
}

/// Try to parse an org-mode link starting at position i: [[path][description]] or [[path]]
fn parse_org_link(chars: &[char], start: usize) -> Option<(Link, usize)> {
    if start + 1 >= chars.len() || chars[start] != '[' || chars[start + 1] != '[' {
        return None;
    }

    let mut i = start + 2;
    let mut path = String::new();
    // Read path until ] or end of link
    while i < chars.len() {
        if chars[i] == ']' {
            if i + 1 < chars.len() && chars[i + 1] == '[' {
                // Description follows
                i += 2;
                let mut desc = String::new();
                while i < chars.len() {
                    if chars[i] == ']' && i + 1 < chars.len() && chars[i + 1] == ']' {
                        let description = Some(desc);
                        i += 2;
                        let raw: String = chars[start..i].iter().collect();
                        let (link_type, clean_path) = parse_link_type(&path);
                        return Some((
                            Link {
                                link_type,
                                path: clean_path,
                                description,
                                raw,
                            },
                            i,
                        ));
                    }
                    desc.push(chars[i]);
                    i += 1;
                }
                return None;
            } else if i + 1 < chars.len() && chars[i + 1] == ']' {
                // No description
                i += 2;
                let raw: String = chars[start..i].iter().collect();
                let (link_type, clean_path) = parse_link_type(&path);
                return Some((
                    Link {
                        link_type,
                        path: clean_path,
                        description: None,
                        raw,
                    },
                    i,
                ));
            } else {
                return None;
            }
        }
        path.push(chars[i]);
        i += 1;
    }

    None
}

fn parse_link_type(path: &str) -> (LinkType, String) {
    if let Some(rest) = path.strip_prefix("id:") {
        (LinkType::Id, rest.to_string())
    } else if let Some(rest) = path.strip_prefix("file:") {
        (LinkType::File, rest.to_string())
    } else if let Some(rest) = path.strip_prefix("http:") {
        (LinkType::Http, format!("http:{rest}"))
    } else if let Some(rest) = path.strip_prefix("https:") {
        (LinkType::Https, format!("https:{rest}"))
    } else if let Some(idx) = path.find(':') {
        let protocol = &path[..idx];
        let rest = &path[idx + 1..];
        (LinkType::Custom(protocol.to_string()), rest.to_string())
    } else {
        // Default to file link for bare paths
        (LinkType::File, path.to_string())
    }
}

/// Try to parse inline markup at position i
fn try_parse_markup(chars: &[char], i: usize) -> Option<(InlineContent, usize)> {
    let c = chars[i];

    // Code and verbatim are special: they don't nest
    if c == '~' || c == '=' {
        return try_parse_code_verbatim(chars, i);
    }

    let marker = match c {
        '*' => Some('*'),
        '/' => Some('/'),
        '_' => Some('_'),
        '+' => Some('+'),
        _ => None,
    };

    let marker = marker?;

    // Must be preceded by start of string, whitespace, or punctuation
    if i > 0 && chars[i - 1].is_alphanumeric() {
        return None;
    }

    // Must be followed by a non-space character
    if i + 1 >= chars.len() || chars[i + 1] == ' ' {
        return None;
    }

    // Find closing marker
    let mut j = i + 1;
    while j < chars.len() {
        if chars[j] == marker && (j + 1 >= chars.len() || !chars[j + 1].is_alphanumeric()) {
            // Must be preceded by non-space
            if chars[j - 1] != ' ' {
                let inner: String = chars[i + 1..j].iter().collect();
                let end = j + 1;
                let content = match marker {
                    '*' => InlineContent::Bold(vec![InlineContent::Text(inner)]),
                    '/' => InlineContent::Italic(vec![InlineContent::Text(inner)]),
                    '_' => InlineContent::Underline(vec![InlineContent::Text(inner)]),
                    '+' => InlineContent::StrikeThrough(vec![InlineContent::Text(inner)]),
                    _ => unreachable!(),
                };
                return Some((content, end));
            }
        }
        j += 1;
    }

    None
}

fn try_parse_code_verbatim(chars: &[char], i: usize) -> Option<(InlineContent, usize)> {
    let marker = chars[i];

    if i > 0 && chars[i - 1].is_alphanumeric() {
        return None;
    }

    if i + 1 >= chars.len() || chars[i + 1] == ' ' {
        return None;
    }

    let mut j = i + 1;
    while j < chars.len() {
        if chars[j] == marker && (j + 1 >= chars.len() || !chars[j + 1].is_alphanumeric()) {
            if chars[j - 1] != ' ' {
                let inner: String = chars[i + 1..j].iter().collect();
                let end = j + 1;
                let content = if marker == '~' {
                    InlineContent::Code(inner)
                } else {
                    InlineContent::Verbatim(inner)
                };
                return Some((content, end));
            }
        }
        j += 1;
    }

    None
}

/// Extract all links from a document section's inline content
pub fn extract_links_from_content(content: &[InlineContent]) -> Vec<&Link> {
    let mut links = Vec::new();
    for item in content {
        match item {
            InlineContent::Link(link) => links.push(link),
            InlineContent::Bold(inner)
            | InlineContent::Italic(inner)
            | InlineContent::Underline(inner)
            | InlineContent::StrikeThrough(inner) => {
                links.extend(extract_links_from_content(inner));
            }
            _ => {}
        }
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_link() {
        let content = parse_inline_content("[[id:abc-123][My Node]]");
        assert_eq!(content.len(), 1);
        if let InlineContent::Link(link) = &content[0] {
            assert_eq!(link.link_type, LinkType::Id);
            assert_eq!(link.path, "abc-123");
            assert_eq!(link.description.as_deref(), Some("My Node"));
        } else {
            panic!("Expected link");
        }
    }

    #[test]
    fn test_bare_link() {
        let content = parse_inline_content("[[some/path]]");
        assert_eq!(content.len(), 1);
        if let InlineContent::Link(link) = &content[0] {
            assert_eq!(link.link_type, LinkType::File);
            assert_eq!(link.path, "some/path");
            assert_eq!(link.description, None);
        } else {
            panic!("Expected link");
        }
    }

    #[test]
    fn test_text_with_link() {
        let content = parse_inline_content("Hello [[id:abc][world]] end");
        assert_eq!(content.len(), 3);
        assert!(matches!(&content[0], InlineContent::Text(t) if t == "Hello "));
        assert!(matches!(&content[1], InlineContent::Link(_)));
        assert!(matches!(&content[2], InlineContent::Text(t) if t == " end"));
    }

    #[test]
    fn test_bold() {
        let content = parse_inline_content("some *bold* text");
        assert_eq!(content.len(), 3);
        assert!(matches!(&content[1], InlineContent::Bold(_)));
    }

    #[test]
    fn test_italic() {
        let content = parse_inline_content("some /italic/ text");
        assert_eq!(content.len(), 3);
        assert!(matches!(&content[1], InlineContent::Italic(_)));
    }

    #[test]
    fn test_code() {
        let content = parse_inline_content("some ~code~ text");
        assert_eq!(content.len(), 3);
        assert!(matches!(&content[1], InlineContent::Code(s) if s == "code"));
    }

    #[test]
    fn test_verbatim() {
        let content = parse_inline_content("some =verb= text");
        assert_eq!(content.len(), 3);
        assert!(matches!(&content[1], InlineContent::Verbatim(s) if s == "verb"));
    }
}
