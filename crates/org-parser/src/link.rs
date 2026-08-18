use crate::cst::{InlineContent, Link, LinkType};
use crate::timestamp::parse_timestamp;

/// Recognized protocols for bare (unbracketed) links in body text
const PLAIN_LINK_PROTOCOLS: [&str; 2] = ["https://", "http://"];

/// Parse all inline content from a string, recognizing links, markup, etc.
pub fn parse_inline_content(s: &str) -> Vec<InlineContent> {
    let mut result = Vec::new();
    let mut current_text = String::new();
    let chars: Vec<char> = s.chars().collect();
    let offsets: Vec<usize> = s.char_indices().map(|(idx, _)| idx).collect();
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

        // Check for <<target>> / <<<radio target>>> and <protocol:...> angle links
        if chars[i] == '<' {
            if let Some((link, consumed)) = parse_angle_construct(&s[offsets[i]..]) {
                if !current_text.is_empty() {
                    result.push(InlineContent::Text(std::mem::take(&mut current_text)));
                }
                result.push(InlineContent::Link(link));
                i += consumed;
                continue;
            }
        }

        // Check for timestamps: <2024-01-15 Mon> / [2024-01-15]
        if chars[i] == '<' || chars[i] == '[' {
            if let Some((ts, consumed)) = parse_timestamp(&s[offsets[i]..]) {
                if !current_text.is_empty() {
                    result.push(InlineContent::Text(std::mem::take(&mut current_text)));
                }
                let char_len = s[offsets[i]..offsets[i] + consumed].chars().count();
                result.push(InlineContent::Timestamp(ts));
                i += char_len;
                continue;
            }
        }

        // Check for a bare URL in body text
        if (i == 0 || !is_link_body_char(chars[i - 1])) && chars[i] == 'h' {
            if let Some((link, consumed)) = parse_plain_link(&s[offsets[i]..]) {
                if !current_text.is_empty() {
                    result.push(InlineContent::Text(std::mem::take(&mut current_text)));
                }
                result.push(InlineContent::Link(link));
                i += consumed;
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

fn is_link_body_char(c: char) -> bool {
    c.is_alphanumeric() || c == ':' || c == '/' || c == '[' || c == '<'
}

/// Parse a bare URL such as `https://example.com/page`.
/// Returns the link and the number of chars consumed.
fn parse_plain_link(s: &str) -> Option<(Link, usize)> {
    let protocol = PLAIN_LINK_PROTOCOLS
        .iter()
        .find(|p| s.starts_with(**p))?;

    let mut end = s.len();
    for (idx, c) in s.char_indices() {
        if c.is_whitespace() || matches!(c, '<' | '>' | '[' | ']' | '{' | '}' | '"' | '\'') {
            end = idx;
            break;
        }
    }

    let mut url = &s[..end];
    while let Some(last) = url.chars().last() {
        if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | ')') {
            url = &url[..url.len() - last.len_utf8()];
        } else {
            break;
        }
    }

    if url.len() <= protocol.len() {
        return None;
    }

    let link_type = if url.starts_with("https://") {
        LinkType::Https
    } else {
        LinkType::Http
    };

    Some((
        Link {
            link_type,
            path: url.to_string(),
            description: None,
            raw: url.to_string(),
        },
        url.chars().count(),
    ))
}

/// Parse `<<target>>`, `<<<radio target>>>` or an angle link `<https://example.com>`.
/// Returns the link and the number of chars consumed.
fn parse_angle_construct(s: &str) -> Option<(Link, usize)> {
    if s.starts_with("<<") {
        let radio = s.starts_with("<<<");
        let open = if radio { 3 } else { 2 };
        let close_marker = if radio { ">>>" } else { ">>" };
        let inner_end = s[open..].find(close_marker)? + open;
        let inner = &s[open..inner_end];
        if inner.is_empty() || inner.contains('<') || inner.contains('>') {
            return None;
        }
        let raw = &s[..inner_end + close_marker.len()];
        return Some((
            Link {
                link_type: LinkType::Target,
                path: inner.to_string(),
                description: None,
                raw: raw.to_string(),
            },
            raw.chars().count(),
        ));
    }

    let close = s.find('>')?;
    let inner = &s[1..close];
    if inner.is_empty() || inner.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    let colon = inner.find(':')?;
    if colon == 0 || !inner[..colon].chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    let (link_type, path) = parse_link_type(inner);
    let raw = &s[..=close];

    Some((
        Link {
            link_type,
            path,
            description: None,
            raw: raw.to_string(),
        },
        raw.chars().count(),
    ))
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
    if let Some(rest) = path.strip_prefix('*') {
        (LinkType::Heading, rest.to_string())
    } else if let Some(rest) = path.strip_prefix('#') {
        (LinkType::CustomId, rest.to_string())
    } else if let Some(rest) = path.strip_prefix("id:") {
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

    #[test]
    fn test_plain_url() {
        let content = parse_inline_content("See https://example.com/page for details.");
        let links = extract_links_from_content(&content);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, LinkType::Https);
        assert_eq!(links[0].path, "https://example.com/page");
    }

    #[test]
    fn test_plain_url_trailing_punctuation() {
        let content = parse_inline_content("Go to http://example.com.");
        let links = extract_links_from_content(&content);
        assert_eq!(links[0].path, "http://example.com");
        assert_eq!(links[0].link_type, LinkType::Http);
    }

    #[test]
    fn test_angle_link() {
        let content = parse_inline_content("Read <https://example.com/x> now");
        let links = extract_links_from_content(&content);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, LinkType::Https);
        assert_eq!(links[0].path, "https://example.com/x");
        assert_eq!(links[0].raw, "<https://example.com/x>");
    }

    #[test]
    fn test_bracket_link_not_double_counted() {
        let content = parse_inline_content("[[https://example.com][Site]]");
        let links = extract_links_from_content(&content);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].description.as_deref(), Some("Site"));
    }

    #[test]
    fn test_internal_link_types() {
        let content = parse_inline_content("[[*Some Headline]] and [[#custom-id]]");
        let links = extract_links_from_content(&content);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].link_type, LinkType::Heading);
        assert_eq!(links[0].path, "Some Headline");
        assert_eq!(links[1].link_type, LinkType::CustomId);
        assert_eq!(links[1].path, "custom-id");
    }

    #[test]
    fn test_radio_target() {
        let content = parse_inline_content("A <<my target>> here");
        let links = extract_links_from_content(&content);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, LinkType::Target);
        assert_eq!(links[0].path, "my target");
    }

    #[test]
    fn test_inline_timestamp() {
        let content = parse_inline_content("Meeting on <2024-01-15 Mon 10:00> ok");
        assert_eq!(content.len(), 3);
        assert!(
            matches!(&content[1], InlineContent::Timestamp(ts) if ts.raw == "<2024-01-15 Mon 10:00>")
        );
    }

    #[test]
    fn test_inline_inactive_timestamp() {
        let content = parse_inline_content("Logged [2024-01-15 Mon]");
        assert!(matches!(&content[1], InlineContent::Timestamp(ts) if !ts.active));
    }

    #[test]
    fn test_checkbox_not_a_timestamp() {
        let content = parse_inline_content("[ ] not a timestamp");
        assert_eq!(content.len(), 1);
        assert!(matches!(&content[0], InlineContent::Text(_)));
    }
}
