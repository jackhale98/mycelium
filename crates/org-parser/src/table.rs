use crate::cst::{Table, TableRow};

/// Parse a table from consecutive lines starting with |.
/// Returns the table and number of lines consumed.
pub fn parse_table(lines: &[&str], start: usize) -> Option<(Table, usize)> {
    if !lines.get(start)?.trim_start().starts_with('|') {
        return None;
    }

    let mut rows = Vec::new();
    let mut raw = String::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        if !trimmed.starts_with('|') {
            break;
        }

        raw.push_str(line);
        raw.push('\n');

        // Check if this is a horizontal rule: |---+---|
        if trimmed.starts_with("|-") {
            rows.push(TableRow::Rule(line.to_string()));
        } else {
            let cells: Vec<String> = trimmed
                .trim_start_matches('|')
                .trim_end_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect();
            rows.push(TableRow::Data {
                cells,
                raw: line.to_string(),
            });
        }

        i += 1;
    }

    if rows.is_empty() {
        return None;
    }

    Some((Table { rows, raw }, i - start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_table() {
        let lines = vec![
            "| Name  | Age |",
            "|-------+-----|",
            "| Alice | 30  |",
            "| Bob   | 25  |",
        ];
        let (table, count) = parse_table(&lines, 0).unwrap();
        assert_eq!(count, 4);
        assert_eq!(table.rows.len(), 4);
        assert!(matches!(&table.rows[1], TableRow::Rule(_)));
    }

    #[test]
    fn test_not_a_table() {
        let lines = vec!["Not a table"];
        assert!(parse_table(&lines, 0).is_none());
    }
}
