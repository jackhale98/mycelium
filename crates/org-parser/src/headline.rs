use crate::cst::Headline;
use crate::link::parse_inline_content;
use crate::property::parse_property_drawer;
use crate::timestamp::parse_planning_line;
use std::sync::RwLock;

/// Process-global list of recognized TODO/DONE keywords.
/// Updated via `set_todo_keywords` from the frontend's org-mode settings.
/// Defaults cover common org-mode keywords so first-run behavior matches Emacs.
static TODO_KEYWORDS: RwLock<Vec<String>> = RwLock::new(Vec::new());

fn default_keywords() -> Vec<String> {
    vec![
        "TODO".to_string(), "DONE".to_string(), "NEXT".to_string(),
        "WAITING".to_string(), "HOLD".to_string(),
        "CANCELLED".to_string(), "CANCELED".to_string(),
    ]
}

/// Replace the set of recognized TODO/DONE keywords used by the parser.
/// Pass the combined list of active + done states.
pub fn set_todo_keywords(keywords: Vec<String>) {
    if let Ok(mut guard) = TODO_KEYWORDS.write() {
        *guard = keywords;
    }
}

/// Snapshot of the currently configured keywords (falls back to defaults if none set).
pub fn todo_keywords() -> Vec<String> {
    match TODO_KEYWORDS.read() {
        Ok(guard) if !guard.is_empty() => guard.clone(),
        _ => default_keywords(),
    }
}

/// Whether a line is a valid headline: column-0 stars followed by a space.
pub fn is_headline(line: &str) -> bool {
    let bytes = line.as_bytes();
    let level = bytes.iter().take_while(|&&b| b == b'*').count();
    level > 0 && bytes.len() > level && bytes[level] == b' '
}

/// Parse a headline from a line like "** TODO [#A] Title :tag1:tag2:"
pub fn parse_headline(line: &str) -> Option<Headline> {
    parse_headline_with_keywords(line, &todo_keywords())
}

/// Parse a headline using an explicit TODO keyword set instead of the process-global one.
pub fn parse_headline_with_keywords(line: &str, keywords: &[String]) -> Option<Headline> {
    if !is_headline(line) {
        return None;
    }

    let raw = line.to_string();
    let level = line.bytes().take_while(|&b| b == b'*').count();
    let rest = line[level..].trim_start();

    // Parse TODO keyword using the supplied keyword set
    let kw_refs: Vec<&str> = keywords.iter().map(|s| s.as_str()).collect();
    let (keyword, rest) = parse_keyword(rest, &kw_refs);

    // Parse priority [#A]
    let (priority, rest) = parse_priority(rest);

    // Parse COMMENT marker
    let (is_comment, rest) = parse_comment(rest);

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
        pos: 0,
        is_comment,
    })
}

/// Detect the COMMENT marker. The marker stays in the title text; `is_comment` reports it.
fn parse_comment(s: &str) -> (bool, &str) {
    if let Some(after) = s.strip_prefix("COMMENT") {
        if after.is_empty() || after.starts_with(' ') {
            return (true, s);
        }
    }
    (false, s)
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

/// Parse a priority like [#A] or [#1]
fn parse_priority(s: &str) -> (Option<char>, &str) {
    if s.len() >= 4 && s.starts_with("[#") && s.as_bytes()[3] == b']' {
        let c = s.as_bytes()[2] as char;
        if c.is_ascii_alphanumeric() {
            return (Some(c), s[4..].trim_start());
        }
    }
    (None, s)
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '@' || c == '#' || c == '%'
}

/// Parse tags at the end of a headline like " :tag1:tag2:"
fn parse_tags(s: &str) -> (Vec<String>, &str) {
    let trimmed = s.trim_end();
    if !trimmed.ends_with(':') {
        return (Vec::new(), trimmed);
    }

    // Walk backwards over the trailing run of tag characters and colons
    let mut tag_start = trimmed.len();
    for (idx, c) in trimmed.char_indices().rev() {
        if c == ':' || is_tag_char(c) {
            tag_start = idx;
        } else {
            break;
        }
    }

    // The run must begin with ':' and sit at the start of the line or after whitespace
    if !trimmed[tag_start..].starts_with(':') {
        return (Vec::new(), trimmed);
    }
    if tag_start > 0 && !trimmed[..tag_start].ends_with(char::is_whitespace) {
        return (Vec::new(), trimmed);
    }

    let tags: Vec<String> = trimmed[tag_start..]
        .split(':')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect();

    if tags.is_empty() {
        return (Vec::new(), trimmed);
    }

    (tags, trimmed[..tag_start].trim_end())
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

    #[test]
    fn test_bare_stars_are_not_headlines() {
        assert!(parse_headline("*").is_none());
        assert!(parse_headline("**").is_none());
        assert!(parse_headline("***").is_none());
        assert!(!is_headline("**"));
        assert!(is_headline("** "));
    }

    #[test]
    fn test_tag_only_headline() {
        let h = parse_headline("* :tagonly:").unwrap();
        assert_eq!(h.tags, vec!["tagonly"]);
        assert!(crate::title_to_text(&h.title).is_empty());
    }

    #[test]
    fn test_tags_with_special_chars() {
        let h = parse_headline("* Title :c#:99%:@home:with_under:").unwrap();
        assert_eq!(h.tags, vec!["c#", "99%", "@home", "with_under"]);
        assert_eq!(crate::title_to_text(&h.title), "Title");
    }

    #[test]
    fn test_colon_in_title_is_not_a_tag() {
        let h = parse_headline("* Note: something:").unwrap();
        assert!(h.tags.is_empty());
        assert_eq!(crate::title_to_text(&h.title), "Note: something:");
    }

    #[test]
    fn test_numeric_priority() {
        let h = parse_headline("* TODO [#1] Numeric priority").unwrap();
        assert_eq!(h.priority, Some('1'));
        assert_eq!(crate::title_to_text(&h.title), "Numeric priority");
    }

    #[test]
    fn test_comment_headline() {
        let h = parse_headline("* COMMENT Draft notes").unwrap();
        assert!(h.is_comment);
        let h = parse_headline("* TODO COMMENT Draft").unwrap();
        assert!(h.is_comment);
        let h = parse_headline("* COMMENTARY on things").unwrap();
        assert!(!h.is_comment);
    }

    #[test]
    fn test_archive_tag() {
        let h = parse_headline("* Old stuff :ARCHIVE:").unwrap();
        assert!(h.is_archived());
        let h = parse_headline("* Fresh stuff :work:").unwrap();
        assert!(!h.is_archived());
    }

    #[test]
    fn test_explicit_keywords() {
        let kws = vec!["SPEC".to_string(), "SHIPPED".to_string()];
        let h = parse_headline_with_keywords("* SPEC Write it down", &kws).unwrap();
        assert_eq!(h.keyword, Some("SPEC".to_string()));
        let h = parse_headline_with_keywords("* TODO Write it down", &kws).unwrap();
        assert_eq!(h.keyword, None);
        assert_eq!(crate::title_to_text(&h.title), "TODO Write it down");
    }
}
