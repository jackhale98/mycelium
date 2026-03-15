use crate::cst::Block;

/// Parse a source/quote/example block from lines.
/// Returns the block and number of lines consumed.
pub fn parse_block(lines: &[&str], start: usize) -> Option<(Block, usize)> {
    let line = lines.get(start)?;
    let trimmed = line.trim();
    let upper = trimmed.to_uppercase();

    if !upper.starts_with("#+BEGIN_") {
        return None;
    }

    let after_begin = &trimmed[8..]; // after "#+BEGIN_"
    let (block_type, parameters) = match after_begin.find(' ') {
        Some(idx) => (&after_begin[..idx], after_begin[idx + 1..].to_string()),
        None => (after_begin, String::new()),
    };

    let end_marker = format!("#+END_{}", block_type);
    let block_type = block_type.to_string();

    let mut raw = String::new();
    raw.push_str(line);
    raw.push('\n');

    let mut contents = String::new();
    let mut i = start + 1;

    while i < lines.len() {
        let current = lines[i];
        raw.push_str(current);

        if current.trim().eq_ignore_ascii_case(&end_marker) {
            raw.push('\n');
            return Some((
                Block {
                    block_type,
                    parameters,
                    contents,
                    raw,
                },
                i - start + 1,
            ));
        }

        contents.push_str(current);
        contents.push('\n');
        raw.push('\n');
        i += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_src_block() {
        let lines = vec![
            "#+BEGIN_SRC python",
            "print(\"hello\")",
            "#+END_SRC",
        ];
        let (block, count) = parse_block(&lines, 0).unwrap();
        assert_eq!(count, 3);
        assert_eq!(block.block_type, "SRC");
        assert_eq!(block.parameters, "python");
        assert_eq!(block.contents, "print(\"hello\")\n");
    }

    #[test]
    fn test_quote_block() {
        let lines = vec![
            "#+BEGIN_QUOTE",
            "Some wise words",
            "on two lines",
            "#+END_QUOTE",
        ];
        let (block, count) = parse_block(&lines, 0).unwrap();
        assert_eq!(count, 4);
        assert_eq!(block.block_type, "QUOTE");
        assert!(block.parameters.is_empty());
        assert_eq!(block.contents, "Some wise words\non two lines\n");
    }
}
