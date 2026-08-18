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
    let mut raw: String = s[..=close_pos].to_string();
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
    let mut time_end = None;
    let mut repeater = None;
    let mut warning = None;

    for &part in &parts[1..] {
        if part.len() == 3 && part.chars().all(|c| c.is_alphabetic()) {
            day_name = Some(part.to_string());
        } else if let Some((start, end)) = parse_time_part(part) {
            time = Some(start);
            time_end = end;
        } else if part.starts_with('+') || part.starts_with(".+") {
            repeater = Some(part.to_string());
        } else if part.starts_with('-') && part.len() >= 2 {
            // Warning period: -3d, -1w, etc.
            warning = Some(part.to_string());
        }
    }

    // Date range: <a>--<b>
    let mut consumed = close_pos + 1;
    let mut range_end = None;
    if let Some(after) = s.get(consumed..).and_then(|r| r.strip_prefix("--")) {
        if let Some((end_ts, end_len)) = parse_timestamp(after) {
            consumed += 2 + end_len;
            raw = s[..consumed].to_string();
            range_end = Some(Box::new(end_ts));
        }
    }

    Some((
        Timestamp {
            active,
            date,
            day_name,
            time,
            repeater,
            warning,
            raw,
            time_end,
            range_end,
        },
        consumed,
    ))
}

/// Parse a clock part: `HH:MM` or a range `HH:MM-HH:MM`
fn parse_time_part(part: &str) -> Option<(String, Option<String>)> {
    let (start, end) = match part.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (part, None),
    };

    if !is_clock(start) {
        return None;
    }
    if let Some(end) = end {
        if !is_clock(end) {
            return None;
        }
    }

    Some((start.to_string(), end.map(|e| e.to_string())))
}

fn is_clock(s: &str) -> bool {
    s.len() >= 3
        && s.len() <= 5
        && s.contains(':')
        && s.chars().all(|c| c.is_ascii_digit() || c == ':')
}

/// Parse a planning line (SCHEDULED, DEADLINE, CLOSED).
/// The line must consist solely of `KEYWORD: <timestamp>` pairs.
pub fn parse_planning_line(line: &str) -> Option<Planning> {
    let mut rest = line.trim();

    let mut scheduled = None;
    let mut deadline = None;
    let mut closed = None;
    let mut found = false;

    while !rest.is_empty() {
        let (after_keyword, slot) = if let Some(r) = rest.strip_prefix("SCHEDULED:") {
            (r, 0)
        } else if let Some(r) = rest.strip_prefix("DEADLINE:") {
            (r, 1)
        } else if let Some(r) = rest.strip_prefix("CLOSED:") {
            (r, 2)
        } else {
            return None;
        };

        let after_keyword = after_keyword.trim_start();
        let (ts, len) = parse_timestamp(after_keyword)?;

        match slot {
            0 => scheduled = Some(ts),
            1 => deadline = Some(ts),
            _ => closed = Some(ts),
        }
        found = true;
        rest = after_keyword[len..].trim_start();
    }

    if !found {
        return None;
    }

    Some(Planning {
        scheduled,
        deadline,
        closed,
        raw: line.to_string(),
    })
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
    fn test_warning_period() {
        let (ts, _) = parse_timestamp("<2024-01-15 Mon +1m -3d>").unwrap();
        assert_eq!(ts.repeater.as_deref(), Some("+1m"));
        assert_eq!(ts.warning.as_deref(), Some("-3d"));
    }

    #[test]
    fn test_full_timestamp() {
        let (ts, _) = parse_timestamp("<2024-01-15 Mon 09:00 ++1w -2d>").unwrap();
        assert_eq!(ts.date, "2024-01-15");
        assert_eq!(ts.day_name.as_deref(), Some("Mon"));
        assert_eq!(ts.time.as_deref(), Some("09:00"));
        assert_eq!(ts.repeater.as_deref(), Some("++1w"));
        assert_eq!(ts.warning.as_deref(), Some("-2d"));
        assert_eq!(ts.raw, "<2024-01-15 Mon 09:00 ++1w -2d>");
    }

    #[test]
    fn test_hour_repeater() {
        let (ts, _) = parse_timestamp("<2024-01-15 Mon 09:00 +4h>").unwrap();
        assert_eq!(ts.repeater.as_deref(), Some("+4h"));
    }

    #[test]
    fn test_not_a_timestamp() {
        assert!(parse_timestamp("hello").is_none());
        assert!(parse_timestamp("<not-a-date>").is_none());
    }

    #[test]
    fn test_time_range() {
        let (ts, len) = parse_timestamp("<2024-01-15 Mon 10:00-11:30>").unwrap();
        assert_eq!(ts.time.as_deref(), Some("10:00"));
        assert_eq!(ts.time_end.as_deref(), Some("11:30"));
        assert_eq!(len, 28);
    }

    #[test]
    fn test_time_range_with_repeater() {
        let (ts, _) = parse_timestamp("<2024-01-15 Mon 09:00-10:00 +1w>").unwrap();
        assert_eq!(ts.time.as_deref(), Some("09:00"));
        assert_eq!(ts.time_end.as_deref(), Some("10:00"));
        assert_eq!(ts.repeater.as_deref(), Some("+1w"));
    }

    #[test]
    fn test_date_range() {
        let (ts, len) = parse_timestamp("<2024-01-15 Mon>--<2024-01-17 Wed>").unwrap();
        assert_eq!(ts.date, "2024-01-15");
        assert_eq!(ts.raw, "<2024-01-15 Mon>--<2024-01-17 Wed>");
        assert_eq!(len, 34);
        let end = ts.range_end.unwrap();
        assert_eq!(end.date, "2024-01-17");
        assert_eq!(end.day_name.as_deref(), Some("Wed"));
    }

    #[test]
    fn test_planning_requires_keyword_at_start() {
        assert!(parse_planning_line("We SCHEDULED: the review for later.").is_none());
        assert!(parse_planning_line("Body text mentioning DEADLINE: soon").is_none());
    }

    #[test]
    fn test_planning_closed_and_scheduled() {
        let plan =
            parse_planning_line("  CLOSED: [2024-01-15 Mon 10:00] SCHEDULED: <2024-01-16 Tue>")
                .unwrap();
        assert!(plan.closed.is_some());
        assert!(plan.scheduled.is_some());
        assert!(plan.deadline.is_none());
    }
}
