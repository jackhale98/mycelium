use crate::cst::*;

/// Serialize an OrgDocument back to org-mode text.
/// Uses raw fields for round-trip fidelity.
pub fn serialize(doc: &OrgDocument) -> String {
    let mut out = String::new();

    // File-level property drawer (before metadata, per org-roam convention)
    if let Some(ref props) = doc.file_properties {
        out.push_str(&props.raw);
    }

    if doc.preamble_items.is_empty() {
        // Metadata lines
        for entry in &doc.metadata {
            out.push_str(&entry.raw);
            out.push('\n');
        }

        // Preamble
        if !doc.preamble.is_empty() {
            out.push_str(&doc.preamble);
            if !doc.preamble.ends_with('\n') {
                out.push('\n');
            }
        }
    } else {
        // Metadata and text interleaved in their original order
        for item in &doc.preamble_items {
            match item {
                PreambleItem::Metadata(entry) => out.push_str(&entry.raw),
                PreambleItem::Text(text) => out.push_str(text),
            }
            out.push('\n');
        }
    }

    // Sections
    for section in &doc.sections {
        serialize_section(&mut out, section);
    }

    // Restore the original absence of a final newline
    if !doc.final_newline && out.ends_with('\n') {
        out.pop();
    }

    // Restore the original line ending style
    if doc.line_ending == LineEnding::Crlf {
        out = out.replace('\n', "\r\n");
    }

    out
}

fn serialize_section(out: &mut String, section: &Section) {
    // Headline
    out.push_str(&section.headline.raw);
    out.push('\n');

    // Planning
    if let Some(ref planning) = section.headline.planning {
        out.push_str(&planning.raw);
        out.push('\n');
    }

    // Property drawer
    if let Some(ref props) = section.headline.properties {
        out.push_str(&props.raw);
    }

    // Body elements
    for element in &section.body {
        serialize_element(out, element);
    }

    // Child sections
    for child in &section.children {
        serialize_section(out, child);
    }
}

fn serialize_element(out: &mut String, element: &Element) {
    match element {
        Element::Paragraph(p) => {
            out.push_str(&p.raw);
            out.push('\n');
        }
        Element::Block(b) => {
            out.push_str(&b.raw);
        }
        Element::List(l) => {
            out.push_str(&l.raw);
        }
        Element::Table(t) => {
            out.push_str(&t.raw);
        }
        Element::Drawer(d) => {
            out.push_str(&d.raw);
        }
        Element::BlankLine(s) => {
            out.push_str(s);
            out.push('\n');
        }
        Element::Verbatim(s) => {
            out.push_str(s);
            out.push('\n');
        }
    }
}
