/// Markup handling is integrated into link.rs (parse_inline_content).
/// This module provides utilities for serializing markup back to org format.

use crate::cst::InlineContent;

/// Serialize inline content back to org-mode text
pub fn serialize_inline(content: &[InlineContent]) -> String {
    let mut out = String::new();
    for item in content {
        match item {
            InlineContent::Text(t) => out.push_str(t),
            InlineContent::Bold(inner) => {
                out.push('*');
                out.push_str(&serialize_inline(inner));
                out.push('*');
            }
            InlineContent::Italic(inner) => {
                out.push('/');
                out.push_str(&serialize_inline(inner));
                out.push('/');
            }
            InlineContent::Underline(inner) => {
                out.push('_');
                out.push_str(&serialize_inline(inner));
                out.push('_');
            }
            InlineContent::StrikeThrough(inner) => {
                out.push('+');
                out.push_str(&serialize_inline(inner));
                out.push('+');
            }
            InlineContent::Code(s) => {
                out.push('~');
                out.push_str(s);
                out.push('~');
            }
            InlineContent::Verbatim(s) => {
                out.push('=');
                out.push_str(s);
                out.push('=');
            }
            InlineContent::Link(link) => {
                out.push_str(&link.raw);
            }
            InlineContent::Timestamp(ts) => {
                out.push_str(&ts.raw);
            }
        }
    }
    out
}
