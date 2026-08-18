pub mod block;
pub mod cst;
pub mod drawer;
pub mod export_html;
pub mod export_md;
pub mod headline;
pub mod link;
pub mod list;
pub mod markup;
pub mod metadata;
pub mod property;
pub mod serialize;
pub mod table;
pub mod timestamp;

use cst::*;

/// Options controlling how a document is parsed.
#[derive(Debug, Clone, Default)]
pub struct ParseOptions {
    /// Explicit TODO/DONE keyword set. When `None`, per-file `#+TODO:` declarations are
    /// honored if present, otherwise the process-global set from `set_todo_keywords`.
    pub todo_keywords: Option<Vec<String>>,
}

/// Parse an org-mode document from text.
/// Returns a CST that preserves all whitespace for round-trip serialization.
pub fn parse(input: &str) -> OrgDocument {
    parse_document_with_options(input, &ParseOptions::default())
}

/// Parse an org-mode document using an explicit TODO keyword set,
/// bypassing both per-file declarations and the process-global set.
pub fn parse_with_keywords(input: &str, keywords: &[String]) -> OrgDocument {
    parse_document_with_options(
        input,
        &ParseOptions {
            todo_keywords: Some(keywords.to_vec()),
        },
    )
}

/// Parse an org-mode document with explicit options.
pub fn parse_document_with_options(input: &str, options: &ParseOptions) -> OrgDocument {
    let lines: Vec<&str> = input.lines().collect();
    let offsets = line_offsets(input, &lines);
    let mut doc = OrgDocument::new();
    doc.line_ending = LineEnding::detect(input);
    doc.final_newline = input.ends_with('\n');
    let mut i = 0;

    // Parse file-level property drawer, metadata, and preamble (before first headline)
    let mut preamble_lines = Vec::new();

    // Check for file-level property drawer (must come first, per org-roam convention)
    if i < lines.len() && lines[i].trim() == ":PROPERTIES:" {
        if let Some((drawer, count)) = property::parse_property_drawer(&lines[i..]) {
            doc.file_properties = Some(drawer);
            i += count;
        }
    }

    while i < lines.len() {
        let line = lines[i];

        // Check for headline
        if headline::is_headline(line) {
            break;
        }

        // A block in the preamble is preserved verbatim so its contents can't be
        // mistaken for headlines or metadata
        if let Some((_, count)) = block::parse_block(&lines, i) {
            for block_line in &lines[i..i + count] {
                preamble_lines.push(*block_line);
                doc.preamble_items
                    .push(PreambleItem::Text(block_line.to_string()));
            }
            i += count;
            continue;
        }

        // Check for metadata
        if let Some(entry) = metadata::parse_metadata_line(line) {
            doc.preamble_items.push(PreambleItem::Metadata(entry.clone()));
            doc.metadata.push(entry);
            i += 1;
            continue;
        }

        // Preamble text
        preamble_lines.push(line);
        doc.preamble_items.push(PreambleItem::Text(line.to_string()));
        i += 1;
    }

    if !preamble_lines.is_empty() {
        doc.preamble = preamble_lines.join("\n");
        doc.preamble.push('\n');
    }

    let keywords = match &options.todo_keywords {
        Some(kws) => kws.clone(),
        None => {
            let file_keywords = metadata::get_todo_keywords(&doc.metadata);
            if file_keywords.is_empty() {
                headline::todo_keywords()
            } else {
                file_keywords
            }
        }
    };

    // Parse sections (headlines and their content)
    let (sections, _) = parse_sections(&lines, &offsets, i, 0, &keywords);
    doc.sections = sections;

    doc
}

/// Byte offset of each line within the source text.
fn line_offsets(input: &str, lines: &[&str]) -> Vec<usize> {
    let base = input.as_ptr() as usize;
    lines
        .iter()
        .map(|line| line.as_ptr() as usize - base)
        .collect()
}

/// Parse sections at a given level. Returns sections and the line index where parsing stopped.
fn parse_sections(
    lines: &[&str],
    offsets: &[usize],
    start: usize,
    min_level: usize,
    keywords: &[String],
) -> (Vec<Section>, usize) {
    let mut sections = Vec::new();
    let mut i = start;

    while i < lines.len() {
        if let Some(mut hl) = headline::parse_headline_with_keywords(lines[i], keywords) {
            if hl.level <= min_level && min_level > 0 {
                // This headline is at a higher level, stop
                break;
            }

            hl.pos = offsets[i];
            i += 1;

            // Attach planning and properties
            let consumed = headline::attach_headline_metadata(&mut hl, &lines[i..]);
            i += consumed;

            // Parse body elements until next headline
            let (body, next_i) = parse_body(lines, i);
            i = next_i;

            // Parse child sections
            let (children, next_i) = parse_sections(lines, offsets, i, hl.level, keywords);
            i = next_i;

            sections.push(Section {
                headline: hl,
                body,
                children,
            });
        } else {
            break;
        }
    }

    (sections, i)
}

/// Parse body elements (paragraphs, blocks, lists, tables, drawers) until the next headline.
fn parse_body(lines: &[&str], start: usize) -> (Vec<Element>, usize) {
    let mut elements = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = lines[i];

        // Stop at the next headline, whatever its level: deeper ones belong to child sections
        if headline::is_headline(line) {
            break;
        }

        // Blank line
        if line.trim().is_empty() {
            elements.push(Element::BlankLine(line.to_string()));
            i += 1;
            continue;
        }

        // Try block
        if let Some((blk, count)) = block::parse_block(lines, i) {
            elements.push(Element::Block(blk));
            i += count;
            continue;
        }

        // Try drawer
        if let Some((drw, count)) = drawer::parse_drawer(lines, i) {
            elements.push(Element::Drawer(drw));
            i += count;
            continue;
        }

        // Try table
        if let Some((tbl, count)) = table::parse_table(lines, i) {
            elements.push(Element::Table(tbl));
            i += count;
            continue;
        }

        // Try list
        if let Some((lst, count)) = list::parse_list(lines, i) {
            elements.push(Element::List(lst));
            i += count;
            continue;
        }

        // Paragraph / verbatim line
        let content = link::parse_inline_content(line);
        let is_plain_text = content.len() == 1 && matches!(&content[0], InlineContent::Text(_));

        if is_plain_text {
            elements.push(Element::Verbatim(line.to_string()));
        } else {
            elements.push(Element::Paragraph(Paragraph {
                content,
                raw: line.to_string(),
            }));
        }
        i += 1;
    }

    (elements, i)
}

/// Extract all nodes (headlines with :ID: property) from a document.
/// Also extracts file-level nodes (property drawer before first headline).
pub fn extract_nodes(doc: &OrgDocument) -> Vec<NodeInfo> {
    let mut nodes = Vec::new();

    // File-level node: check if there's an :ID: in file-level property drawer
    if let Some(id) = doc.file_id() {
        let title = metadata::get_title(&doc.metadata)
            .unwrap_or("")
            .to_string();
        let filetags = metadata::get_filetags(&doc.metadata);

        let properties_map: serde_json::Map<String, serde_json::Value> = doc
            .file_properties
            .as_ref()
            .map(|pd| {
                pd.properties
                    .iter()
                    .map(|p| {
                        (
                            p.key.clone(),
                            serde_json::Value::String(p.value.clone()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        nodes.push(NodeInfo {
            id: id.to_string(),
            level: 0,
            title,
            pos: 0,
            todo: None,
            priority: None,
            scheduled: None,
            deadline: None,
            tags: filetags,
            aliases: doc.file_roam_aliases().into_iter().map(|s| s.to_string()).collect(),
            refs: doc.file_roam_refs().into_iter().map(|s| s.to_string()).collect(),
            properties_json: serde_json::to_string(&properties_map).unwrap_or_default(),
            olp: Vec::new(),
        });
    }

    for section in &doc.sections {
        extract_nodes_from_section(section, &mut nodes, &mut Vec::new());
    }
    nodes
}

/// Information about a node extracted from the parsed document
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: String,
    pub level: usize,
    pub title: String,
    pub pos: usize,
    pub todo: Option<String>,
    pub priority: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub refs: Vec<String>,
    pub properties_json: String,
    pub olp: Vec<String>,
}

fn extract_nodes_from_section(
    section: &Section,
    nodes: &mut Vec<NodeInfo>,
    olp: &mut Vec<String>,
) {
    let title_text = title_to_text(&section.headline.title);

    if let Some(id) = section.headline.id() {
        let properties_map: serde_json::Map<String, serde_json::Value> = section
            .headline
            .properties
            .as_ref()
            .map(|pd| {
                pd.properties
                    .iter()
                    .map(|p| {
                        (
                            p.key.clone(),
                            serde_json::Value::String(p.value.clone()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();

        nodes.push(NodeInfo {
            id: id.to_string(),
            level: section.headline.level,
            title: title_text.clone(),
            pos: section.headline.pos,
            todo: section.headline.keyword.clone(),
            priority: section.headline.priority.map(|c| c.to_string()),
            scheduled: section
                .headline
                .planning
                .as_ref()
                .and_then(|p| p.scheduled.as_ref())
                .map(|t| t.raw.clone()),
            deadline: section
                .headline
                .planning
                .as_ref()
                .and_then(|p| p.deadline.as_ref())
                .map(|t| t.raw.clone()),
            tags: section.headline.tags.clone(),
            aliases: section.headline.roam_aliases().into_iter().map(|s| s.to_string()).collect(),
            refs: section.headline.roam_refs().into_iter().map(|s| s.to_string()).collect(),
            properties_json: serde_json::to_string(&properties_map).unwrap_or_default(),
            olp: olp.clone(),
        });
    }

    olp.push(title_text);
    for child in &section.children {
        extract_nodes_from_section(child, nodes, olp);
    }
    olp.pop();
}

/// Extract all links from a document, including preamble links
pub fn extract_links(doc: &OrgDocument) -> Vec<LinkInfo> {
    let mut links = Vec::new();
    let file_id = doc.file_id().map(|s| s.to_string());

    // Extract links from preamble text (use file-level ID as source)
    if file_id.is_some() && !doc.preamble.is_empty() {
        let content = link::parse_inline_content(&doc.preamble);
        for l in link::extract_links_from_content(&content) {
            links.push(LinkInfo {
                source_id: file_id.clone(),
                dest: l.path.clone(),
                link_type: link_type_name(&l.link_type).to_string(),
                pos: 0,
            });
        }
    }

    for section in &doc.sections {
        extract_links_from_section(section, &mut links, file_id.as_deref());
    }
    links
}

/// Stable string name for a link type, as stored in the database.
pub fn link_type_name(link_type: &LinkType) -> &str {
    match link_type {
        LinkType::Id => "id",
        LinkType::File => "file",
        LinkType::Http => "http",
        LinkType::Https => "https",
        LinkType::Heading => "heading",
        LinkType::CustomId => "custom-id",
        LinkType::Target => "target",
        LinkType::Custom(s) => s.as_str(),
    }
}

#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub source_id: Option<String>,
    pub dest: String,
    pub link_type: String,
    pub pos: usize,
}

fn extract_links_from_section(
    section: &Section,
    links: &mut Vec<LinkInfo>,
    inherited_id: Option<&str>,
) {
    // org-roam semantics: a headline without its own :ID: belongs to the nearest ancestor node
    let source_id = section
        .headline
        .id()
        .map(|s| s.to_string())
        .or_else(|| inherited_id.map(|s| s.to_string()));

    // Extract links from ALL body elements (paragraphs, list items, verbatim text)
    for element in &section.body {
        let inline_links = match element {
            Element::Paragraph(p) => link::extract_links_from_content(&p.content),
            Element::List(l) => {
                let mut all = Vec::new();
                for item in &l.items {
                    all.extend(link::extract_links_from_content(&item.content));
                }
                all
            }
            Element::Verbatim(_text) => {
                // Verbatim lines rarely contain links, skip for now
                Vec::new()
            }
            _ => Vec::new(),
        };

        for l in inline_links {
            links.push(LinkInfo {
                source_id: source_id.clone(),
                dest: l.path.clone(),
                link_type: link_type_name(&l.link_type).to_string(),
                pos: 0,
            });
        }
    }

    // Extract from headline title
    let title_links = link::extract_links_from_content(&section.headline.title);
    for l in title_links {
        links.push(LinkInfo {
            source_id: source_id.clone(),
            dest: l.path.clone(),
            link_type: link_type_name(&l.link_type).to_string(),
            pos: 0,
        });
    }

    for child in &section.children {
        extract_links_from_section(child, links, source_id.as_deref());
    }
}

/// Convert inline content to plain text
pub fn title_to_text(content: &[InlineContent]) -> String {
    let mut out = String::new();
    for item in content {
        match item {
            InlineContent::Text(t) => out.push_str(t),
            InlineContent::Bold(inner) | InlineContent::Italic(inner) | InlineContent::Underline(inner) | InlineContent::StrikeThrough(inner) => {
                out.push_str(&title_to_text(inner));
            }
            InlineContent::Code(s) | InlineContent::Verbatim(s) => out.push_str(s),
            InlineContent::Link(l) => {
                if let Some(desc) = &l.description {
                    out.push_str(desc);
                } else {
                    out.push_str(&l.path);
                }
            }
            InlineContent::Timestamp(t) => out.push_str(&t.raw),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_doc() {
        let input = "#+TITLE: Test Document

* Heading One
Some text here.

** Sub-heading
More text.
";
        let doc = parse(input);
        assert_eq!(metadata::get_title(&doc.metadata), Some("Test Document"));
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].headline.level, 1);
        assert_eq!(doc.sections[0].children.len(), 1);
    }

    #[test]
    fn test_parse_with_properties() {
        let input = "* My Node
:PROPERTIES:
:ID: test-uuid-123
:ROAM_ALIASES: \"Alias One\"
:END:
Some body text.
";
        let doc = parse(input);
        let section = &doc.sections[0];
        assert_eq!(section.headline.id(), Some("test-uuid-123"));
        assert_eq!(section.headline.roam_aliases(), vec!["Alias One"]);
    }

    #[test]
    fn test_extract_nodes() {
        let input = "* Node Without ID
** Node With ID
:PROPERTIES:
:ID: abc-123
:END:
";
        let doc = parse(input);
        let nodes = extract_nodes(&doc);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "abc-123");
    }

    #[test]
    fn test_extract_links() {
        let input = "* Source Node
:PROPERTIES:
:ID: source-id
:END:
Link to [[id:target-id][Target]] here.
";
        let doc = parse(input);
        let links = extract_links(&doc);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].dest, "target-id");
        assert_eq!(links[0].link_type, "id");
    }

    #[test]
    fn test_round_trip_simple() {
        let input = "#+TITLE: Test
* Heading
Some text.
";
        let doc = parse(input);
        let output = serialize::serialize(&doc);
        assert_eq!(input, output);
    }

    #[test]
    fn test_file_level_node() {
        let input = ":PROPERTIES:
:ID: file-level-id
:ROAM_ALIASES: \"My Alias\"
:END:
#+TITLE: File Level Note

* Sub Heading
:PROPERTIES:
:ID: sub-id
:END:
";
        let doc = parse(input);
        assert_eq!(doc.file_id(), Some("file-level-id"));
        assert_eq!(doc.file_roam_aliases(), vec!["My Alias"]);

        let nodes = extract_nodes(&doc);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].id, "file-level-id");
        assert_eq!(nodes[0].level, 0);
        assert_eq!(nodes[0].title, "File Level Note");
        assert_eq!(nodes[1].id, "sub-id");
        assert_eq!(nodes[1].level, 1);
    }

    #[test]
    fn test_file_level_roundtrip() {
        let input = ":PROPERTIES:
:ID: file-id
:END:
#+TITLE: My Note
* Heading
";
        let doc = parse(input);
        let output = serialize::serialize(&doc);
        assert_eq!(input, output);
    }

    #[test]
    fn test_unquoted_roam_refs() {
        let input = ":PROPERTIES:
:ID: file-id
:ROAM_REFS: https://example.com
:END:
#+TITLE: Ref Note
* Sub
:PROPERTIES:
:ID: sub-id
:ROAM_REFS: https://one.example cite:two
:END:
";
        let doc = parse(input);
        assert_eq!(doc.file_roam_refs(), vec!["https://example.com"]);
        assert_eq!(
            doc.sections[0].headline.roam_refs(),
            vec!["https://one.example", "cite:two"]
        );
        let nodes = extract_nodes(&doc);
        assert_eq!(nodes[0].refs, vec!["https://example.com"]);
        assert_eq!(nodes[1].refs, vec!["https://one.example", "cite:two"]);
    }

    #[test]
    fn test_unterminated_drawer_keeps_headlines() {
        let input = "* First
:PROPERTIES:
:ID: first-id
:END:
:LOGBOOK:
- CLOCK: something
* Second
:PROPERTIES:
:ID: second-id
:END:
Body.
:END:
";
        let doc = parse(input);
        let nodes = extract_nodes(&doc);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[1].id, "second-id");
        assert_eq!(serialize::serialize(&doc), input);
    }

    #[test]
    fn test_node_pos_is_byte_offset() {
        let input = "#+TITLE: Positions\n* One\n:PROPERTIES:\n:ID: one\n:END:\n** Two\n:PROPERTIES:\n:ID: two\n:END:\n";
        let doc = parse(input);
        let nodes = extract_nodes(&doc);
        assert_eq!(nodes[0].pos, input.find("* One").unwrap());
        assert_eq!(nodes[1].pos, input.find("** Two").unwrap());
    }

    #[test]
    fn test_file_level_node_pos_is_zero() {
        let input = ":PROPERTIES:\n:ID: file-id\n:END:\n#+TITLE: T\n* Child\n";
        let doc = parse(input);
        let nodes = extract_nodes(&doc);
        assert_eq!(nodes[0].pos, 0);
    }

    #[test]
    fn test_link_source_inherits_from_ancestor() {
        let input = ":PROPERTIES:
:ID: file-id
:END:
#+TITLE: Inheritance
* Headline without an ID
Link to [[id:target-one][One]].
** Nested with ID
:PROPERTIES:
:ID: nested-id
:END:
Link to [[id:target-two][Two]].
*** Deeper without ID
Link to [[id:target-three][Three]].
";
        let doc = parse(input);
        let links = extract_links(&doc);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].source_id.as_deref(), Some("file-id"));
        assert_eq!(links[1].source_id.as_deref(), Some("nested-id"));
        assert_eq!(links[2].source_id.as_deref(), Some("nested-id"));
    }

    #[test]
    fn test_plain_url_link_extracted() {
        let input = "* Node
:PROPERTIES:
:ID: source-id
:END:
See https://example.com/page and <https://other.example>.
";
        let doc = parse(input);
        let links = extract_links(&doc);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].dest, "https://example.com/page");
        assert_eq!(links[0].link_type, "https");
        assert_eq!(links[1].dest, "https://other.example");
    }

    #[test]
    fn test_internal_links_not_reported_as_file_links() {
        let input = "* Node
:PROPERTIES:
:ID: source-id
:END:
See [[*Other Headline]] and [[#some-custom-id]].
";
        let doc = parse(input);
        let links = extract_links(&doc);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].link_type, "heading");
        assert_eq!(links[1].link_type, "custom-id");
    }

    #[test]
    fn test_per_file_todo_keywords() {
        let input = "#+TITLE: Workflow\n#+TODO: SPEC(s) IMPL | SHIPPED\n* SPEC Design it\n* SHIPPED Done it\n";
        let doc = parse(input);
        assert_eq!(
            doc.file_todo_keywords(),
            vec!["SPEC", "IMPL", "SHIPPED"]
        );
        assert_eq!(doc.sections[0].headline.keyword.as_deref(), Some("SPEC"));
        assert_eq!(title_to_text(&doc.sections[0].headline.title), "Design it");
        assert_eq!(doc.sections[1].headline.keyword.as_deref(), Some("SHIPPED"));
    }

    #[test]
    fn test_parse_with_keywords_overrides() {
        let input = "* STARTED Something\n";
        let keywords = vec!["STARTED".to_string()];
        let doc = parse_with_keywords(input, &keywords);
        assert_eq!(doc.sections[0].headline.keyword.as_deref(), Some("STARTED"));
        assert_eq!(title_to_text(&doc.sections[0].headline.title), "Something");

        let doc = parse_document_with_options(input, &ParseOptions::default());
        assert_eq!(doc.sections[0].headline.keyword, None);
    }

    #[test]
    fn test_unescaped_star_in_preamble_block_splits_like_emacs() {
        // Org requires comma-escaping `*` at column 0 inside literal blocks; an
        // unescaped star is a headline in Emacs, so it must be one here too.
        let input = "#+TITLE: Blocky\n#+BEGIN_SRC org\n* not escaped\n#+END_SRC\n* Real Headline\n";
        let doc = parse(input);
        assert_eq!(doc.sections.len(), 2);
        assert_eq!(title_to_text(&doc.sections[0].headline.title), "not escaped");
        assert_eq!(title_to_text(&doc.sections[1].headline.title), "Real Headline");
        assert_eq!(serialize::serialize(&doc), input);
    }

    #[test]
    fn test_escaped_star_in_preamble_block_does_not_split_file() {
        let input = "#+TITLE: Blocky\n#+BEGIN_SRC org\n,* escaped headline\n#+END_SRC\n* Real Headline\n";
        let doc = parse(input);
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(title_to_text(&doc.sections[0].headline.title), "Real Headline");
        assert_eq!(serialize::serialize(&doc), input);
    }

    #[test]
    fn test_comment_and_archive_exposed() {
        let input = "* COMMENT Scratch\n* Old :ARCHIVE:\n";
        let doc = parse(input);
        assert!(doc.sections[0].headline.is_comment);
        assert!(!doc.sections[0].headline.is_archived());
        assert!(doc.sections[1].headline.is_archived());
        assert!(!doc.sections[1].headline.is_comment);
    }

    #[test]
    fn test_no_file_level_node() {
        let input = "#+TITLE: No Properties
* Heading
";
        let doc = parse(input);
        assert_eq!(doc.file_id(), None);
        assert_eq!(doc.file_properties, None);
        let nodes = extract_nodes(&doc);
        assert_eq!(nodes.len(), 0);
    }
}
