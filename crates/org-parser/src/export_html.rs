use crate::cst::*;

/// Export an OrgDocument to HTML format
pub fn to_html(doc: &OrgDocument) -> String {
    let mut out = String::new();

    out.push_str("<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\n");

    if let Some(title) = doc.metadata.iter().find(|m| m.key.eq_ignore_ascii_case("TITLE")) {
        out.push_str(&format!("<title>{}</title>\n", escape_html(&title.value)));
    }

    out.push_str("<style>\nbody { font-family: system-ui, sans-serif; max-width: 48rem; margin: 2rem auto; padding: 0 1rem; line-height: 1.6; color: #1a1a1a; }\npre { background: #f5f5f5; padding: 1rem; border-radius: 4px; overflow-x: auto; }\ncode { background: #f0f0f0; padding: 0.1em 0.3em; border-radius: 3px; font-size: 0.9em; }\npre code { background: none; padding: 0; }\nblockquote { border-left: 3px solid #ccc; margin-left: 0; padding-left: 1rem; color: #555; }\ntable { border-collapse: collapse; margin: 1rem 0; }\nth, td { border: 1px solid #ddd; padding: 0.5rem 0.75rem; text-align: left; }\nth { background: #f5f5f5; }\n.todo { color: #dc2626; font-weight: bold; }\n.done { color: #16a34a; font-weight: bold; }\n.tag { background: #e0f2fe; color: #0369a1; padding: 0.1em 0.4em; border-radius: 3px; font-size: 0.8em; margin-left: 0.3em; }\n</style>\n");
    out.push_str("</head><body>\n");

    if let Some(title) = doc.metadata.iter().find(|m| m.key.eq_ignore_ascii_case("TITLE")) {
        out.push_str(&format!("<h1>{}</h1>\n", escape_html(&title.value)));
    }

    if !doc.preamble.is_empty() {
        let trimmed = doc.preamble.trim();
        if !trimmed.is_empty() {
            out.push_str(&format!("<p>{}</p>\n", escape_html(trimmed)));
        }
    }

    for section in &doc.sections {
        export_section_html(&mut out, section);
    }

    out.push_str("</body></html>\n");
    out
}

fn export_section_html(out: &mut String, section: &Section) {
    let hl = &section.headline;
    let level = hl.level.min(6);
    let tag = format!("h{level}");

    out.push_str(&format!("<{tag}>"));

    if let Some(ref kw) = hl.keyword {
        let class = if kw == "DONE" { "done" } else { "todo" };
        out.push_str(&format!("<span class=\"{class}\">{kw}</span> "));
    }

    out.push_str(&inline_to_html(&hl.title));

    for t in &hl.tags {
        out.push_str(&format!("<span class=\"tag\">{}</span>", escape_html(t)));
    }

    out.push_str(&format!("</{tag}>\n"));

    if let Some(ref plan) = hl.planning {
        out.push_str("<p class=\"planning\">");
        if let Some(ref s) = plan.scheduled {
            out.push_str(&format!("<strong>SCHEDULED:</strong> {} ", escape_html(&s.raw)));
        }
        if let Some(ref d) = plan.deadline {
            out.push_str(&format!("<strong>DEADLINE:</strong> {} ", escape_html(&d.raw)));
        }
        out.push_str("</p>\n");
    }

    for element in &section.body {
        export_element_html(out, element);
    }

    for child in &section.children {
        export_section_html(out, child);
    }
}

fn export_element_html(out: &mut String, element: &Element) {
    match element {
        Element::Paragraph(p) => {
            out.push_str("<p>");
            out.push_str(&inline_to_html(&p.content));
            out.push_str("</p>\n");
        }
        Element::Block(b) => {
            if b.block_type.eq_ignore_ascii_case("QUOTE") {
                out.push_str("<blockquote>");
                for line in b.contents.lines() {
                    out.push_str(&format!("<p>{}</p>", escape_html(line)));
                }
                out.push_str("</blockquote>\n");
            } else if b.block_type.eq_ignore_ascii_case("SRC") {
                let lang = if b.parameters.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"language-{}\"", escape_html(&b.parameters))
                };
                out.push_str(&format!(
                    "<pre><code{}>{}</code></pre>\n",
                    lang,
                    escape_html(b.contents.trim_end())
                ));
            } else {
                out.push_str(&format!(
                    "<pre>{}</pre>\n",
                    escape_html(b.contents.trim_end())
                ));
            }
        }
        Element::List(l) => {
            let is_ordered = l
                .items
                .first()
                .map(|i| i.bullet.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
                .unwrap_or(false);
            let tag = if is_ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>\n"));
            for item in &l.items {
                out.push_str("<li>");
                if let Some(ref cb) = item.checkbox {
                    let checked = matches!(cb, Checkbox::Checked);
                    out.push_str(&format!(
                        "<input type=\"checkbox\" disabled {}> ",
                        if checked { "checked" } else { "" }
                    ));
                }
                if let Some(ref dt) = item.tag {
                    out.push_str(&format!("<strong>{}:</strong> ", escape_html(dt)));
                }
                out.push_str(&inline_to_html(&item.content));
                out.push_str("</li>\n");
            }
            out.push_str(&format!("</{tag}>\n"));
        }
        Element::Table(t) => {
            out.push_str("<table>\n");
            let mut in_header = true;
            for row in &t.rows {
                match row {
                    TableRow::Rule(_) => {
                        in_header = false;
                    }
                    TableRow::Data { cells, .. } => {
                        out.push_str("<tr>");
                        let cell_tag = if in_header { "th" } else { "td" };
                        for cell in cells {
                            out.push_str(&format!("<{cell_tag}>{}</{cell_tag}>", escape_html(cell)));
                        }
                        out.push_str("</tr>\n");
                    }
                }
            }
            out.push_str("</table>\n");
        }
        Element::Drawer(_) => {}
        Element::BlankLine(_) => {}
        Element::Verbatim(s) => {
            out.push_str(&format!("<p>{}</p>\n", escape_html(s.trim())));
        }
    }
}

fn inline_to_html(content: &[InlineContent]) -> String {
    let mut out = String::new();
    for item in content {
        match item {
            InlineContent::Text(t) => out.push_str(&escape_html(t)),
            InlineContent::Bold(inner) => {
                out.push_str("<strong>");
                out.push_str(&inline_to_html(inner));
                out.push_str("</strong>");
            }
            InlineContent::Italic(inner) => {
                out.push_str("<em>");
                out.push_str(&inline_to_html(inner));
                out.push_str("</em>");
            }
            InlineContent::Underline(inner) => {
                out.push_str("<u>");
                out.push_str(&inline_to_html(inner));
                out.push_str("</u>");
            }
            InlineContent::StrikeThrough(inner) => {
                out.push_str("<del>");
                out.push_str(&inline_to_html(inner));
                out.push_str("</del>");
            }
            InlineContent::Code(s) => {
                out.push_str(&format!("<code>{}</code>", escape_html(s)));
            }
            InlineContent::Verbatim(s) => {
                out.push_str(&format!("<code>{}</code>", escape_html(s)));
            }
            InlineContent::Link(link) => {
                let desc = link
                    .description
                    .as_deref()
                    .unwrap_or(&link.path);
                match link.link_type {
                    LinkType::Http | LinkType::Https => {
                        out.push_str(&format!(
                            "<a href=\"{}\">{}</a>",
                            escape_html(&link.path),
                            escape_html(desc)
                        ));
                    }
                    _ => {
                        out.push_str(&format!("<a href=\"#\">{}</a>", escape_html(desc)));
                    }
                }
            }
            InlineContent::Timestamp(ts) => {
                out.push_str(&format!("<time>{}</time>", escape_html(&ts.raw)));
            }
        }
    }
    out
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn test_html_export() {
        let input = "#+TITLE: Test\n* Heading\nSome *bold* text.\n";
        let doc = parse(input);
        let html = to_html(&doc);
        assert!(html.contains("<h1>Test</h1>"));
        assert!(html.contains("<h1>Heading</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_html_table() {
        let input = "* Data\n| A | B |\n|---+---|\n| 1 | 2 |\n";
        let doc = parse(input);
        let html = to_html(&doc);
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>A</th>"));
        assert!(html.contains("<td>1</td>"));
    }
}
