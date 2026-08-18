use serde::{Deserialize, Serialize};

/// Concrete Syntax Tree for org-mode documents.
/// Preserves all whitespace and formatting for round-trip fidelity.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrgDocument {
    /// File-level metadata (#+TITLE:, #+FILETAGS:, etc.)
    pub metadata: Vec<MetadataEntry>,
    /// File-level property drawer (before first headline, used by org-roam for file-level :ID:)
    pub file_properties: Option<PropertyDrawer>,
    /// Blank lines or text before the first headline
    pub preamble: String,
    /// Everything before the first headline in original order (metadata and text interleaved)
    #[serde(default)]
    pub preamble_items: Vec<PreambleItem>,
    /// Line ending style of the source file
    #[serde(default)]
    pub line_ending: LineEnding,
    /// Whether the source file ended with a newline
    #[serde(default = "default_true")]
    pub final_newline: bool,
    /// Top-level sections (headlines and their content)
    pub sections: Vec<Section>,
}

fn default_true() -> bool {
    true
}

/// One entry of the region before the first headline, in original order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PreambleItem {
    Metadata(MetadataEntry),
    /// A single line of text (without its line terminator)
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        }
    }

    /// Detect the dominant line ending of a source text.
    pub fn detect(input: &str) -> LineEnding {
        let crlf = input.matches("\r\n").count();
        let lf = input.matches('\n').count() - crlf;
        if crlf > lf {
            LineEnding::Crlf
        } else {
            LineEnding::Lf
        }
    }
}

impl Default for LineEnding {
    fn default() -> Self {
        LineEnding::Lf
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
    /// Original line text for round-trip
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub headline: Headline,
    pub body: Vec<Element>,
    pub children: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Headline {
    pub level: usize,
    pub keyword: Option<String>,
    pub priority: Option<char>,
    pub title: Vec<InlineContent>,
    pub tags: Vec<String>,
    /// Raw text of the headline line for round-trip
    pub raw: String,
    pub planning: Option<Planning>,
    pub properties: Option<PropertyDrawer>,
    /// Byte offset of the headline line within the source document
    #[serde(default)]
    pub pos: usize,
    /// Whether the headline is a COMMENT headline
    #[serde(default)]
    pub is_comment: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Planning {
    pub scheduled: Option<Timestamp>,
    pub deadline: Option<Timestamp>,
    pub closed: Option<Timestamp>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyDrawer {
    pub properties: Vec<Property>,
    /// Full raw text including :PROPERTIES: and :END: lines
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: String,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Element {
    Paragraph(Paragraph),
    Block(Block),
    List(List),
    Table(Table),
    Drawer(Drawer),
    BlankLine(String),
    /// Lines we don't specifically parse — preserved verbatim
    Verbatim(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    pub content: Vec<InlineContent>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InlineContent {
    Text(String),
    Bold(Vec<InlineContent>),
    Italic(Vec<InlineContent>),
    Underline(Vec<InlineContent>),
    StrikeThrough(Vec<InlineContent>),
    Code(String),
    Verbatim(String),
    Link(Link),
    Timestamp(Timestamp),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub link_type: LinkType,
    pub path: String,
    pub description: Option<String>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LinkType {
    Id,
    File,
    Http,
    Https,
    Custom(String),
    /// Internal link to a headline by title: [[*Headline]]
    Heading,
    /// Internal link to a :CUSTOM_ID: : [[#custom-id]]
    CustomId,
    /// A target definition (<<target>>) or a link to one
    Target,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timestamp {
    pub active: bool,
    pub date: String,
    pub day_name: Option<String>,
    pub time: Option<String>,
    /// Repeater: +1w, ++1m, .+2d, +3h, +1y
    pub repeater: Option<String>,
    /// Warning period: -3d, -1w
    pub warning: Option<String>,
    pub raw: String,
    /// End of a same-day time range: `10:00-11:30` stores `11:30` here
    #[serde(default)]
    pub time_end: Option<String>,
    /// End of a date range written as `<a>--<b>`
    #[serde(default)]
    pub range_end: Option<Box<Timestamp>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub block_type: String,
    pub parameters: String,
    pub contents: String,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    pub items: Vec<ListItem>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    pub indent: usize,
    pub bullet: String,
    pub checkbox: Option<Checkbox>,
    pub tag: Option<String>,
    pub content: Vec<InlineContent>,
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Checkbox {
    Unchecked,
    Checked,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub rows: Vec<TableRow>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TableRow {
    Rule(String),
    Data { cells: Vec<String>, raw: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Drawer {
    pub name: String,
    pub contents: String,
    pub raw: String,
}

/// Split an org-roam multi-value property (`:ROAM_REFS:`, `:ROAM_ALIASES:`).
/// Handles both quoted segments and bare whitespace-separated tokens.
pub fn split_roam_value(value: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            if start < j {
                out.push(&value[start..j]);
            }
            i = if j < bytes.len() { j + 1 } else { j };
        } else {
            let start = i;
            let mut j = i;
            while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'"' {
                j += 1;
            }
            out.push(&value[start..j]);
            i = j;
        }
    }

    out
}

impl OrgDocument {
    pub fn new() -> Self {
        OrgDocument {
            metadata: Vec::new(),
            file_properties: None,
            preamble: String::new(),
            preamble_items: Vec::new(),
            line_ending: LineEnding::Lf,
            final_newline: true,
            sections: Vec::new(),
        }
    }

    /// TODO keywords declared in this file via `#+TODO:` / `#+SEQ_TODO:` / `#+TYP_TODO:`.
    /// Empty when the file does not declare its own workflow.
    pub fn file_todo_keywords(&self) -> Vec<String> {
        crate::metadata::get_todo_keywords(&self.metadata)
    }

    /// Get the file-level :ID: (org-roam file-level node)
    pub fn file_id(&self) -> Option<&str> {
        self.file_properties.as_ref().and_then(|pd| {
            pd.properties
                .iter()
                .find(|p| p.key.eq_ignore_ascii_case("ID"))
                .map(|p| p.value.as_str())
        })
    }

    /// Get file-level :ROAM_ALIASES:
    pub fn file_roam_aliases(&self) -> Vec<&str> {
        self.file_property("ROAM_ALIASES")
            .map(split_roam_value)
            .unwrap_or_default()
    }

    /// Get file-level :ROAM_REFS:
    pub fn file_roam_refs(&self) -> Vec<&str> {
        self.file_property("ROAM_REFS")
            .map(split_roam_value)
            .unwrap_or_default()
    }

    fn file_property(&self, key: &str) -> Option<&str> {
        self.file_properties.as_ref().and_then(|pd| {
            pd.properties
                .iter()
                .find(|p| p.key.eq_ignore_ascii_case(key))
                .map(|p| p.value.as_str())
        })
    }
}

impl Default for OrgDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl Headline {
    pub fn id(&self) -> Option<&str> {
        self.properties.as_ref().and_then(|pd| {
            pd.properties
                .iter()
                .find(|p| p.key.eq_ignore_ascii_case("ID"))
                .map(|p| p.value.as_str())
        })
    }

    pub fn roam_aliases(&self) -> Vec<&str> {
        self.property("ROAM_ALIASES")
            .map(split_roam_value)
            .unwrap_or_default()
    }

    pub fn roam_refs(&self) -> Vec<&str> {
        self.property("ROAM_REFS")
            .map(split_roam_value)
            .unwrap_or_default()
    }

    /// Whether this headline carries the `ARCHIVE` tag.
    pub fn is_archived(&self) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case("ARCHIVE"))
    }

    fn property(&self, key: &str) -> Option<&str> {
        self.properties.as_ref().and_then(|pd| {
            pd.properties
                .iter()
                .find(|p| p.key.eq_ignore_ascii_case(key))
                .map(|p| p.value.as_str())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_roam_value_quoted() {
        assert_eq!(
            split_roam_value("\"Alias One\" \"Alias Two\""),
            vec!["Alias One", "Alias Two"]
        );
    }

    #[test]
    fn test_split_roam_value_bare() {
        assert_eq!(
            split_roam_value("https://example.com  cite:foo"),
            vec!["https://example.com", "cite:foo"]
        );
    }

    #[test]
    fn test_split_roam_value_mixed() {
        assert_eq!(
            split_roam_value("https://example.com \"Some Ref\" other"),
            vec!["https://example.com", "Some Ref", "other"]
        );
    }

    #[test]
    fn test_detect_line_ending() {
        assert_eq!(LineEnding::detect("a\r\nb\r\n"), LineEnding::Crlf);
        assert_eq!(LineEnding::detect("a\nb\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("a"), LineEnding::Lf);
    }
}
