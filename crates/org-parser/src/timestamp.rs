use crate::cst::{Planning, Timestamp};

/// Parse a timestamp like <2024-01-15 Mon 10:00> or [2024-01-15]
pub fn parse_timestamp(s: &str) -> Option<(Timestamp, usize)> {
    let start = s.as_bytes().first()?;
    let (active, _open, close) = match start {
        b'<' => (true, '<', '>'),
        b'[' => (false, '[', ']'),
        _ => return None,
    };

    let close_pos = s.find(close)?;
    let raw: String = s[..=close_pos].to_string();
    let inner = &s[1..close_pos];

    let parts: Vec<&str> = inner.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let date = parts[0].to_string();
    // Validate date format roughly: YYYY-MM-DD
    if date.len() < 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return None;
    }

    let mut day_name = None;
    let mut time = None;
    let mut repeater = None;

    for &part in &parts[1..] {
        if part.len() == 3 && part.chars().all(|c| c.is_alphabetic()) {
            day_name = Some(part.to_string());
        } else if part.contains(':') && part.len() <= 5 && part.chars().all(|c| c.is_ascii_digit() || c == ':') {
            time = Some(part.to_string());
        } else if part.starts_with('+') || part.starts_with('.') {
            repeater = Some(part.to_string());
        }
    }

    Some((
        Timestamp {
            active,
            date,
            day_name,
            time,
            repeater,
            raw,
        },
        close_pos + 1,
    ))
}

/// Parse a planning line (SCHEDULED, DEADLINE, CLOSED)
pub fn parse_planning_line(line: &str) -> Option<Planning> {
    let trimmed = line.trim();
    let has_scheduled = trimmed.contains("SCHEDULED:");
    let has_deadline = trimmed.contains("DEADLINE:");
    let has_closed = trimmed.contains("CLOSED:");

    if !has_scheduled && !has_deadline && !has_closed {
        return None;
    }

    let scheduled = extract_planning_timestamp(trimmed, "SCHEDULED:");
    let deadline = extract_planning_timestamp(trimmed, "DEADLINE:");
    let closed = extract_planning_timestamp(trimmed, "CLOSED:");

    Some(Planning {
        scheduled,
        deadline,
        closed,
        raw: line.to_string(),
    })
}

fn extract_planning_timestamp(line: &str, keyword: &str) -> Option<Timestamp> {
    let idx = line.find(keyword)?;
    let after = &line[idx + keyword.len()..].trim_start();
    parse_timestamp(after).map(|(ts, _)| ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_timestamp() {
        let (ts, len) = parse_timestamp("<2024-01-15 Mon 10:00>").unwrap();
        assert!(ts.active);
        assert_eq!(ts.date, "2024-01-15");
        assert_eq!(ts.day_name.as_deref(), Some("Mon"));
        assert_eq!(ts.time.as_deref(), Some("10:00"));
        assert_eq!(len, 22);
    }

    #[test]
    fn test_inactive_timestamp() {
        let (ts, _) = parse_timestamp("[2024-01-15]").unwrap();
        assert!(!ts.active);
        assert_eq!(ts.date, "2024-01-15");
        assert_eq!(ts.day_name, None);
        assert_eq!(ts.time, None);
    }

    #[test]
    fn test_planning_line() {
        let plan = parse_planning_line("SCHEDULED: <2024-01-15 Mon> DEADLINE: <2024-02-01>").unwrap();
        assert!(plan.scheduled.is_some());
        assert!(plan.deadline.is_some());
        assert!(plan.closed.is_none());
    }

    #[test]
    fn test_repeater_timestamp() {
        let (ts, _) = parse_timestamp("<2024-01-15 Mon +1w>").unwrap();
        assert_eq!(ts.date, "2024-01-15");
        assert_eq!(ts.repeater.as_deref(), Some("+1w"));
        assert_eq!(ts.raw, "<2024-01-15 Mon +1w>");
    }

    #[test]
    fn test_double_plus_repeater() {
        let (ts, _) = parse_timestamp("<2024-01-15 Mon ++1m>").unwrap();
        assert_eq!(ts.repeater.as_deref(), Some("++1m"));
    }

    #[test]
    fn test_dot_plus_repeater() {
        let (ts, _) = parse_timestamp("<2024-03-01 Fri .+2d>").unwrap();
        assert_eq!(ts.repeater.as_deref(), Some(".+2d"));
    }

    #[test]
    fn test_timestamp_with_time_and_repeater() {
        let (ts, _) = parse_timestamp("<2024-01-15 Mon 09:00 +1w>").unwrap();
        assert_eq!(ts.time.as_deref(), Some("09:00"));
        assert_eq!(ts.repeater.as_deref(), Some("+1w"));
    }

    #[test]
    fn test_not_a_timestamp() {
        assert!(parse_timestamp("hello").is_none());
        assert!(parse_timestamp("<not-a-date>").is_none());
    }
}
