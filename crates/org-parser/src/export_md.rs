use crate::cst::*;

/// Export an OrgDocument to Markdown format
pub fn to_markdown(doc: &OrgDocument) -> String {
    let mut out = String::new();

    // Title as H1
    if let Some(title) = doc.metadata.iter().find(|m| m.key.eq_ignore_ascii_case("TITLE")) {
        out.push_str(&format!("# {}\n\n", title.value));
    }

    // Preamble
    if !doc.preamble.is_empty() {
        let trimmed = doc.preamble.trim();
        if !trimmed.is_empty() {
            out.push_str(trimmed);
            out.push_str("\n\n");
        }
    }

    for section in &doc.sections {
        export_section_md(&mut out, section);
    }

    out
}

fn export_section_md(out: &mut String, section: &Section) {
    let hl = &section.headline;

    // Heading: ## level
    // Markdown heading level = org level + 1 (since title is H1)
    let md_level = hl.level + 1;
    let hashes: String = "#".repeat(md_level.min(6));

    out.push_str(&hashes);
    out.push(' ');

    if let Some(ref kw) = hl.keyword {
        out.push_str(&format!("**{kw}** "));
    }

    out.push_str(&inline_to_md(&hl.title));

    if !hl.tags.is_empty() {
        out.push_str(&format!(" `{}`", hl.tags.join("` `")));
    }

    out.push_str("\n\n");

    // Planning
    if let Some(ref plan) = hl.planning {
        if let Some(ref s) = plan.scheduled {
            out.push_str(&format!("> SCHEDULED: {}\n", s.raw));
        }
        if let Some(ref d) = plan.deadline {
            out.push_str(&format!("> DEADLINE: {}\n", d.raw));
        }
        out.push('\n');
    }

    // Body
    for element in &section.body {
        export_element_md(out, element);
    }

    for child in &section.children {
        export_section_md(out, child);
    }
}

fn export_element_md(out: &mut String, element: &Element) {
    match element {
        Element::Paragraph(p) => {
            out.push_str(&inline_to_md(&p.content));
            out.push_str("\n\n");
        }
        Element::Block(b) => {
            let lang = if b.block_type.eq_ignore_ascii_case("SRC") {
                &b.parameters
            } else {
                ""
            };
            if b.block_type.eq_ignore_ascii_case("QUOTE") {
                for line in b.contents.lines() {
                    out.push_str(&format!("> {line}\n"));
                }
            } else {
                out.push_str(&format!("```{lang}\n{}\n```\n", b.contents.trim_end()));
            }
            out.push('\n');
        }
        Element::List(l) => {
            for item in &l.items {
                export_list_item_md(out, item);
            }
            out.push('\n');
        }
        Element::Table(t) => {
            for (i, row) in t.rows.iter().enumerate() {
                match row {
                    TableRow::Rule(_) => {
                        // Skip — we generate our own separator after header
                    }
                    TableRow::Data { cells, .. } => {
                        out.push_str("| ");
                        out.push_str(&cells.join(" | "));
                        out.push_str(" |\n");

                        // Add separator after first data row (assumes header)
                        if i == 0 {
                            out.push_str("|");
                            for _ in cells {
                                out.push_str(" --- |");
                            }
                            out.push('\n');
                        }
                    }
                }
            }
            out.push('\n');
        }
        Element::Drawer(_) => {
            // Skip drawers in export
        }
        Element::BlankLine(_) => {}
        Element::Verbatim(s) => {
            out.push_str(s.trim());
            out.push_str("\n\n");
        }
    }
}

fn export_list_item_md(out: &mut String, item: &ListItem) {
    let indent: String = " ".repeat(item.indent);
    let bullet = if item.bullet.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("{}.", item.bullet.trim_end_matches('.').trim_end_matches(')'))
    } else {
        "-".to_string()
    };

    out.push_str(&indent);
    out.push_str(&bullet);
    out.push(' ');

    if let Some(ref cb) = item.checkbox {
        match cb {
            Checkbox::Checked => out.push_str("[x] "),
            Checkbox::Unchecked => out.push_str("[ ] "),
            Checkbox::Partial => out.push_str("[-] "),
        }
    }

    if let Some(ref tag) = item.tag {
        out.push_str(&format!("**{tag}**: "));
    }

    out.push_str(&inline_to_md(&item.content));
    out.push('\n');
}

fn inline_to_md(content: &[InlineContent]) -> String {
    let mut out = String::new();
    for item in content {
        match item {
            InlineContent::Text(t) => out.push_str(t),
            InlineContent::Bold(inner) => {
                out.push_str("**");
                out.push_str(&inline_to_md(inner));
                out.push_str("**");
            }
            InlineContent::Italic(inner) => {
                out.push('_');
                out.push_str(&inline_to_md(inner));
                out.push('_');
            }
            InlineContent::Underline(inner) => {
                out.push_str("<u>");
                out.push_str(&inline_to_md(inner));
                out.push_str("</u>");
            }
            InlineContent::StrikeThrough(inner) => {
                out.push_str("~~");
                out.push_str(&inline_to_md(inner));
                out.push_str("~~");
            }
            InlineContent::Code(s) => {
                out.push('`');
                out.push_str(s);
                out.push('`');
            }
            InlineContent::Verbatim(s) => {
                out.push('`');
                out.push_str(s);
                out.push('`');
            }
            InlineContent::Link(link) => {
                let desc = link.description.as_deref().unwrap_or(&link.path);
                match link.link_type {
                    LinkType::Http | LinkType::Https => {
                        out.push_str(&format!("[{desc}]({})", link.path));
                    }
                    LinkType::Id => {
                        // Org-roam id links become wiki-style links
                        out.push_str(&format!("[[{desc}]]"));
                    }
                    _ => {
                        out.push_str(&format!("[{desc}]({})", link.path));
                    }
                }
            }
            InlineContent::Timestamp(ts) => {
                out.push_str(&ts.raw);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_basic_export() {
        let input = "#+TITLE: Test\n* Heading\nSome *bold* text.\n";
        let doc = parse(input);
        let md = to_markdown(&doc);
        assert!(md.contains("# Test"));
        assert!(md.contains("## Heading"));
        assert!(md.contains("**bold**"));
    }

    #[test]
    fn test_code_block_export() {
        let input = "* Code\n#+BEGIN_SRC rust\nfn main() {}\n#+END_SRC\n";
        let doc = parse(input);
        let md = to_markdown(&doc);
        assert!(md.contains("```rust"));
        assert!(md.contains("fn main() {}"));
    }

    #[test]
    fn test_list_export() {
        let input = "* Tasks\n- [ ] Todo\n- [X] Done\n";
        let doc = parse(input);
        let md = to_markdown(&doc);
        assert!(md.contains("- [ ] Todo"));
        assert!(md.contains("- [x] Done"));
    }
}
