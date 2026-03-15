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
    /// Top-level sections (headlines and their content)
    pub sections: Vec<Section>,
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timestamp {
    pub active: bool,
    pub date: String,
    pub day_name: Option<String>,
    pub time: Option<String>,
    pub repeater: Option<String>,
    pub raw: String,
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

impl OrgDocument {
    pub fn new() -> Self {
        OrgDocument {
            metadata: Vec::new(),
            file_properties: None,
            preamble: String::new(),
            sections: Vec::new(),
        }
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
        self.file_properties
            .as_ref()
            .and_then(|pd| {
                pd.properties
                    .iter()
                    .find(|p| p.key.eq_ignore_ascii_case("ROAM_ALIASES"))
                    .map(|p| {
                        p.value
                            .split('"')
                            .enumerate()
                            .filter(|(i, _)| i % 2 == 1)
                            .map(|(_, s)| s)
                            .collect()
                    })
            })
            .unwrap_or_default()
    }

    /// Get file-level :ROAM_REFS:
    pub fn file_roam_refs(&self) -> Vec<&str> {
        self.file_properties
            .as_ref()
            .and_then(|pd| {
                pd.properties
                    .iter()
                    .find(|p| p.key.eq_ignore_ascii_case("ROAM_REFS"))
                    .map(|p| {
                        p.value
                            .split('"')
                            .enumerate()
                            .filter(|(i, _)| i % 2 == 1)
                            .map(|(_, s)| s)
                            .collect()
                    })
            })
            .unwrap_or_default()
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
        self.properties
            .as_ref()
            .and_then(|pd| {
                pd.properties
                    .iter()
                    .find(|p| p.key.eq_ignore_ascii_case("ROAM_ALIASES"))
                    .map(|p| {
                        p.value
                            .split('"')
                            .enumerate()
                            .filter(|(i, _)| i % 2 == 1)
                            .map(|(_, s)| s)
                            .collect()
                    })
            })
            .unwrap_or_default()
    }

    pub fn roam_refs(&self) -> Vec<&str> {
        self.properties
            .as_ref()
            .and_then(|pd| {
                pd.properties
                    .iter()
                    .find(|p| p.key.eq_ignore_ascii_case("ROAM_REFS"))
                    .map(|p| {
                        p.value
                            .split('"')
                            .enumerate()
                            .filter(|(i, _)| i % 2 == 1)
                            .map(|(_, s)| s)
                            .collect()
                    })
            })
            .unwrap_or_default()
    }
}
