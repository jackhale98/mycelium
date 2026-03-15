use crate::cst::*;

/// Serialize an OrgDocument back to org-mode text.
/// Uses raw fields for round-trip fidelity.
pub fn serialize(doc: &OrgDocument) -> String {
    let mut out = String::new();

    // File-level property drawer (before metadata, per org-roam convention)
    if let Some(ref props) = doc.file_properties {
        out.push_str(&props.raw);
    }

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

    // Sections
    for section in &doc.sections {
        serialize_section(&mut out, section);
    }

    // Remove trailing extra newline if the input didn't have one
    // (the caller should compare with original)
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
