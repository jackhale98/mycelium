use crate::cst::Headline;
use crate::link::parse_inline_content;
use crate::property::parse_property_drawer;
use crate::timestamp::parse_planning_line;

/// Parse a headline from a line like "** TODO [#A] Title :tag1:tag2:"
pub fn parse_headline(line: &str) -> Option<Headline> {
    if !line.starts_with('*') {
        return None;
    }

    let raw = line.to_string();
    let bytes = line.as_bytes();

    // Count stars
    let level = bytes.iter().take_while(|&&b| b == b'*').count();
    if level == 0 || (bytes.len() > level && bytes[level] != b' ') {
        return None;
    }

    let rest = if level < line.len() {
        line[level..].trim_start()
    } else {
        ""
    };

    // Parse TODO keyword
    let todo_keywords = [
        "TODO", "DONE", "NEXT", "WAITING", "HOLD", "CANCELLED", "CANCELED",
    ];
    let (keyword, rest) = parse_keyword(rest, &todo_keywords);

    // Parse priority [#A]
    let (priority, rest) = parse_priority(rest);

    // Parse tags at end
    let (tags, title_str) = parse_tags(rest);

    // Parse inline content in title
    let title = parse_inline_content(title_str);

    Some(Headline {
        level,
        keyword,
        priority,
        title,
        tags,
        raw,
        planning: None,
        properties: None,
    })
}

/// Parse a TODO-style keyword at the start of the string
fn parse_keyword<'a>(s: &'a str, keywords: &[&str]) -> (Option<String>, &'a str) {
    for kw in keywords {
        if s.starts_with(kw) {
            let after = &s[kw.len()..];
            if after.is_empty() || after.starts_with(' ') {
                return (Some(kw.to_string()), after.trim_start());
            }
        }
    }
    (None, s)
}

/// Parse a priority like [#A]
fn parse_priority(s: &str) -> (Option<char>, &str) {
    if s.len() >= 4 && s.starts_with("[#") && s.as_bytes()[3] == b']' {
        let c = s.as_bytes()[2] as char;
        if c.is_ascii_uppercase() {
            return (Some(c), s[4..].trim_start());
        }
    }
    (None, s)
}

/// Parse tags at the end of a headline like " :tag1:tag2:"
fn parse_tags(s: &str) -> (Vec<String>, &str) {
    let trimmed = s.trim_end();
    if !trimmed.ends_with(':') {
        return (Vec::new(), s.trim_end());
    }

    // Find the start of tags: look for a space followed by ":"
    if let Some(tag_start) = trimmed.rfind(" :") {
        let tag_part = &trimmed[tag_start + 1..];
        // Validate: must be :word: pattern
        let tags: Vec<String> = tag_part
            .split(':')
            .filter(|t| !t.is_empty())
            .map(|t| t.to_string())
            .collect();

        if !tags.is_empty()
            && tags
                .iter()
                .all(|t| t.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '@'))
        {
            return (tags, trimmed[..tag_start].trim_end());
        }
    }

    (Vec::new(), trimmed)
}

/// Attach planning and properties to a headline by consuming lines after it
pub fn attach_headline_metadata(
    headline: &mut Headline,
    lines: &[&str],
) -> usize {
    let mut consumed = 0;

    // Check for planning line (SCHEDULED, DEADLINE, CLOSED)
    if let Some(line) = lines.first() {
        if let Some(planning) = parse_planning_line(line) {
            headline.planning = Some(planning);
            consumed += 1;
        }
    }

    // Check for property drawer
    let prop_start = consumed;
    if let Some(line) = lines.get(prop_start) {
        if line.trim() == ":PROPERTIES:" {
            if let Some((drawer, count)) = parse_property_drawer(&lines[prop_start..]) {
                headline.properties = Some(drawer);
                consumed += count;
            }
        }
    }

    consumed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_headline() {
        let h = parse_headline("* Hello World").unwrap();
        assert_eq!(h.level, 1);
        assert_eq!(h.keyword, None);
        assert_eq!(h.priority, None);
        assert!(h.tags.is_empty());
    }

    #[test]
    fn test_headline_with_todo() {
        let h = parse_headline("** TODO Fix the bug").unwrap();
        assert_eq!(h.level, 2);
        assert_eq!(h.keyword, Some("TODO".to_string()));
    }

    #[test]
    fn test_headline_with_priority() {
        let h = parse_headline("* TODO [#A] Urgent task").unwrap();
        assert_eq!(h.priority, Some('A'));
        assert_eq!(h.keyword, Some("TODO".to_string()));
    }

    #[test]
    fn test_headline_with_tags() {
        let h = parse_headline("* My heading :tag1:tag2:").unwrap();
        assert_eq!(h.tags, vec!["tag1", "tag2"]);
    }

    #[test]
    fn test_headline_full() {
        let h = parse_headline("*** DONE [#B] Complete task :work:urgent:").unwrap();
        assert_eq!(h.level, 3);
        assert_eq!(h.keyword, Some("DONE".to_string()));
        assert_eq!(h.priority, Some('B'));
        assert_eq!(h.tags, vec!["work", "urgent"]);
    }

    #[test]
    fn test_not_a_headline() {
        assert!(parse_headline("Not a headline").is_none());
        assert!(parse_headline("*bold text*").is_none());
    }
}
