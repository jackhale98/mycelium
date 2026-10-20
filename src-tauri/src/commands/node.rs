use crate::commands::editor::write_and_index;
use crate::fsutil;
use crate::state::AppState;
use db::query;
use tauri::State;

/// Get a single node by its ID
#[tauri::command]
pub async fn get_node(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<query::NodeRecord>, String> {
    state.with_db(|conn| query::get_node(conn, &id).map_err(|e| e.to_string()))
}

/// List all nodes in the vault
#[tauri::command]
pub async fn list_nodes(state: State<'_, AppState>) -> Result<Vec<query::NodeRecord>, String> {
    state.with_db(|conn| query::list_nodes(conn).map_err(|e| e.to_string()))
}

/// Get backlinks for a node (with context)
#[tauri::command]
pub async fn get_backlinks(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::BacklinkRecord>, String> {
    // Read previews through the vault's own file access: an Android vault has no
    // path std::fs could open, and the previews would come back empty.
    let vault_fs = state.vault_fs();
    state.with_db(|conn| {
        query::get_backlinks_with(conn, &node_id, vault_fs.as_ref()).map_err(|e| e.to_string())
    })
}

/// Get forward links from a node
#[tauri::command]
pub async fn get_forward_links(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::ForwardLink>, String> {
    state.with_db(|conn| query::get_forward_links(conn, &node_id).map_err(|e| e.to_string()))
}

/// Search nodes by title
#[tauri::command]
pub async fn search_nodes(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::NodeRecord>, String> {
    state.with_db(|conn| query::search_nodes(conn, &query).map_err(|e| e.to_string()))
}

/// Full-text search across titles and body content, with snippets
#[tauri::command]
pub async fn search_full(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::SearchResult>, String> {
    state.with_db(|conn| query::search_full(conn, &query).map_err(|e| e.to_string()))
}

/// Get agenda items (ALL headlines with TODO, SCHEDULED, or DEADLINE from ALL org files)
#[tauri::command]
pub async fn get_agenda(
    state: State<'_, AppState>,
) -> Result<Vec<query::HeadlineRecord>, String> {
    state.with_db(|conn| query::get_agenda_items(conn).map_err(|e| e.to_string()))
}

/// Get unlinked mentions for a node (title appears in other files without explicit link)
#[tauri::command]
pub async fn get_unlinked_mentions(
    node_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<query::SearchResult>, String> {
    state.with_db(|conn| query::get_unlinked_mentions(conn, &node_id).map_err(|e| e.to_string()))
}

/// Export a file as Markdown
#[tauri::command]
pub async fn export_markdown(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let key = fsutil::vault_key(&vault_path, &file_path)?;

    let content = state
        .vault_fs()
        .read_to_string(&key)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let doc = org_parser::parse(&content);
    Ok(org_parser::export_md::to_markdown(&doc))
}

/// Export a file as HTML
#[tauri::command]
pub async fn export_html(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let vault_path = state.vault_path()?;
    let key = fsutil::vault_key(&vault_path, &file_path)?;

    let content = state
        .vault_fs()
        .read_to_string(&key)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let doc = org_parser::parse(&content);
    Ok(org_parser::export_html::to_html(&doc))
}

/// Rename a node: update its title in its file and update all backlink descriptions
/// across all files that link to this node.
#[tauri::command]
pub async fn rename_node(
    app: tauri::AppHandle,
    node_id: String,
    new_title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use tauri::Emitter;

    let new_title = crate::commands::editor::sanitize_title(&new_title)?;

    // A rename rewrites the node's file and then every file linking to it; no
    // other write may interleave with that.
    let _writes = state.lock_writes();

    // Get the node's file
    let node = state.with_db(|conn| {
        query::get_node(conn, &node_id).map_err(|e| e.to_string())
    })?.ok_or("Node not found")?;

    let vault_path = state.vault_path()?;

    // 1. Update the title in the node's own file
    let node_key = fsutil::vault_key(&vault_path, &node.file)?;

    let content = state
        .vault_fs()
        .read_to_string(&node_key)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    let new_content = if node.level == 0 {
        set_file_title(&content, &new_title)
    } else {
        update_headline_title(&content, &node_id, &new_title).ok_or_else(|| {
            "The note's headline could not be found; the file may have changed. Reload and try again."
                .to_string()
        })?
    };

    write_and_index(&state, &node_key, &new_content)?;

    // 2. Update link descriptions in all files that reference this node
    let backlinks = state.with_db(|conn| {
        query::get_backlinks(conn, &node_id).map_err(|e| e.to_string())
    })?;

    let old_link_pattern = format!("[[id:{node_id}][");
    let re_str = format!(r"\[\[id:{}\]\[[^\]]*\]\]", regex_lite::escape(&node_id));
    let re = regex_lite::Regex::new(&re_str).unwrap();
    // A literal replacement: `$1` in a title must never expand as a capture group.
    let replacement = format!("[[id:{node_id}][{}]]", link_description(&new_title));
    let mut failed: Vec<String> = Vec::new();

    let vault_fs = state.vault_fs();
    for bl in &backlinks {
        let bl_key = match fsutil::vault_key(&vault_path, &bl.source_file) {
            Ok(key) => key,
            Err(e) => { failed.push(format!("{}: {e}", bl.source_file)); continue; }
        };

        if !vault_fs.exists(&bl_key) { continue; }

        // One unreadable or unwritable file must not abort the rename and leave the
        // vault half-updated; collect the failures and report them all at the end.
        let bl_content = match vault_fs.read_to_string(&bl_key) {
            Ok(c) => c,
            Err(e) => { failed.push(format!("{}: {e}", bl.source_file)); continue; }
        };

        if !bl_content.contains(&old_link_pattern) { continue; }

        let new_bl_content = re
            .replace_all(&bl_content, |_: &regex_lite::Captures| replacement.clone())
            .to_string();
        if new_bl_content == bl_content { continue; }

        // Same write-then-index the node's own file gets, rather than a second
        // hand-rolled copy of it.
        if let Err(e) = write_and_index(&state, &bl_key, &new_bl_content) {
            failed.push(format!("{}: {e}", bl.source_file));
            continue;
        }
    }

    let _ = app.emit("db-updated", ());

    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Renamed the node, but {} file(s) still show the old title: {}",
            failed.len(),
            failed.join("; ")
        ))
    }
}

/// Set the file-level `#+TITLE:`, matching org's case-insensitive keyword and
/// inserting the line when the file has none. The file's line endings and any
/// byte-order mark are kept.
fn set_file_title(content: &str, new_title: &str) -> String {
    // `[^\r\n]`, not `.`: a CRLF file's `\r` belongs to the line ending.
    let re = regex_lite::Regex::new(r"(?im)^([ \t]*#\+TITLE:)[ \t]*[^\r\n]*").unwrap();
    if let Some(caps) = re.captures(content) {
        let keyword = caps[1].to_string();
        return re
            .replace(content, |_: &regex_lite::Captures| format!("{keyword} {new_title}"))
            .to_string();
    }

    // No title line yet: place one after a leading file-level property drawer.
    let (bom, body) = match content.strip_prefix('\u{feff}') {
        Some(rest) => ("\u{feff}", rest),
        None => ("", content),
    };
    let eol = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let text = |l: &str| l.trim_end_matches(['\n', '\r']).trim().to_string();

    let mut insert_at = 0;
    if lines.first().map(|l| text(l).eq_ignore_ascii_case(":PROPERTIES:")).unwrap_or(false) {
        while insert_at < lines.len() && !text(lines[insert_at]).eq_ignore_ascii_case(":END:") {
            insert_at += 1;
        }
        insert_at = (insert_at + 1).min(lines.len());
    }

    let mut out = String::from(bom);
    for (i, line) in lines.iter().enumerate() {
        if i == insert_at {
            out.push_str(&format!("#+TITLE: {new_title}{eol}"));
        }
        out.push_str(line);
        // A last line without a terminator needs one before anything follows it.
        if i + 1 == lines.len() && insert_at == lines.len() && !line.ends_with('\n') {
            out.push_str(eol);
        }
    }
    if insert_at >= lines.len() {
        out.push_str(&format!("#+TITLE: {new_title}"));
        if body.ends_with('\n') || body.is_empty() {
            out.push_str(eol);
        }
    }
    out
}

/// Rewrite the title of the headline whose property drawer holds `:ID: node_id`.
///
/// Only the title text changes. The stars, TODO keyword (including one the
/// file declares itself with `#+TODO:`), priority, `COMMENT` marker, tags and
/// their spacing, and every other byte of the file are left as they were, line
/// endings included.
///
/// `None` when no headline owns that ID — say the file changed since it was
/// indexed — so the caller can refuse instead of reporting a rename that never
/// happened.
fn update_headline_title(content: &str, node_id: &str, new_title: &str) -> Option<String> {
    let file_keywords = org_parser::parse(content).file_todo_keywords();
    let keywords = if file_keywords.is_empty() {
        org_parser::headline::todo_keywords()
    } else {
        file_keywords
    };

    // Lines with their terminators, so the file can be put back exactly.
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let text = |l: &'_ str| -> String { l.trim_end_matches(['\n', '\r']).to_string() };

    for (id_idx, line) in lines.iter().enumerate() {
        if !is_id_property(&text(line), node_id) {
            continue;
        }
        let Some(hl_idx) = owning_headline(&lines, id_idx) else { continue };

        let headline = text(lines[hl_idx]);
        let Some((start, end)) = headline_title_span(&headline, &keywords) else { continue };

        let mut replaced = String::with_capacity(headline.len() + new_title.len());
        replaced.push_str(&headline[..start]);
        if !replaced.ends_with([' ', '\t']) {
            replaced.push(' ');
        }
        replaced.push_str(new_title);
        // An empty title sat against the tags; keep them apart.
        if end <= start && start < headline.len() {
            replaced.push(' ');
        }
        replaced.push_str(&headline[end.max(start)..]);

        let mut out = String::with_capacity(content.len() + new_title.len());
        for (i, l) in lines.iter().enumerate() {
            if i == hl_idx {
                out.push_str(&replaced);
                out.push_str(&l[headline.len()..]);
            } else {
                out.push_str(l);
            }
        }
        return Some(out);
    }
    None
}

/// `true` for a property line `:ID: <node_id>`, with the value matched exactly
/// — `abc` must not match `:ID: abc-2`.
fn is_id_property(line: &str, node_id: &str) -> bool {
    let t = line.trim();
    let Some(rest) = t.get(..4).filter(|k| k.eq_ignore_ascii_case(":ID:")).map(|_| &t[4..]) else {
        return false;
    };
    rest.trim() == node_id
}

/// The headline a property line at `idx` belongs to: walk back through its
/// drawer to `:PROPERTIES:`, then past planning lines. `None` when the line is
/// not inside a headline's drawer at all — an `:ID:` typed into body text.
fn owning_headline(lines: &[&str], idx: usize) -> Option<usize> {
    let text = |i: usize| lines[i].trim_end_matches(['\n', '\r']).trim();
    let mut j = idx;
    loop {
        j = j.checked_sub(1)?;
        let t = text(j);
        if t.eq_ignore_ascii_case(":PROPERTIES:") {
            break;
        }
        if !(t.starts_with(':') && t[1..].contains(':')) {
            return None;
        }
    }
    let mut h = j.checked_sub(1)?;
    while is_planning_line(text(h)) {
        h = h.checked_sub(1)?;
    }
    org_parser::headline::is_headline(lines[h].trim_end_matches(['\n', '\r'])).then_some(h)
}

/// Byte range of a headline's title: after the stars, keyword, priority and
/// `COMMENT`, before the whitespace that precedes trailing tags (or the end).
fn headline_title_span(line: &str, keywords: &[String]) -> Option<(usize, usize)> {
    if !org_parser::headline::is_headline(line) {
        return None;
    }
    let skip_blank = |s: &str, at: usize| at + (s[at..].len() - s[at..].trim_start_matches([' ', '\t']).len());

    let mut start = skip_blank(line, line.bytes().take_while(|&b| b == b'*').count());
    for kw in keywords {
        let rest = &line[start..];
        if rest.starts_with(kw.as_str()) && rest[kw.len()..].chars().next().map_or(true, |c| c == ' ' || c == '\t') {
            start = skip_blank(line, start + kw.len());
            break;
        }
    }
    let rest = &line[start..];
    if rest.len() >= 4 && rest.starts_with("[#") && rest.as_bytes()[3] == b']' {
        start = skip_blank(line, start + 4);
    }
    let rest = &line[start..];
    if rest.starts_with("COMMENT") && rest[7..].chars().next().map_or(true, |c| c == ' ' || c == '\t') {
        start = skip_blank(line, start + 7);
    }

    // Trailing tags: a run of tag characters and colons, starting with ':' and
    // set off by whitespace.
    let trimmed_len = line.trim_end().len();
    let mut end = trimmed_len;
    if line[..trimmed_len].ends_with(':') {
        let mut tag_start = trimmed_len;
        for (i, c) in line[..trimmed_len].char_indices().rev() {
            if c == ':' || c.is_alphanumeric() || "_@#%".contains(c) {
                tag_start = i;
            } else {
                break;
            }
        }
        let before = &line[..tag_start];
        if tag_start >= start
            && line[tag_start..trimmed_len].starts_with(':')
            && line[tag_start..trimmed_len].len() > 1
            && (before.ends_with([' ', '\t']) || tag_start == start)
        {
            end = before.trim_end_matches([' ', '\t']).len();
        }
    }
    Some((start, end))
}

fn is_planning_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("SCHEDULED:") || t.starts_with("DEADLINE:") || t.starts_with("CLOSED:")
}

/// A link description that cannot close the link early. Org ends a bracket
/// link at the first `]]`, and the backlink rewrite looks for descriptions
/// without brackets, so brackets in a title become parentheses here; the title
/// itself is written as typed.
fn link_description(title: &str) -> String {
    title.replace('[', "(").replace(']', ")")
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_title_case_insensitively_and_keeps_keyword_case() {
        let content = "#+title: Old\n\nBody\n";
        assert_eq!(set_file_title(content, "New"), "#+title: New\n\nBody\n");
    }

    #[test]
    fn sets_uppercase_title() {
        assert_eq!(set_file_title("#+TITLE: Old\n", "New"), "#+TITLE: New\n");
    }

    #[test]
    fn title_containing_dollar_is_written_literally() {
        assert_eq!(set_file_title("#+TITLE: Old\n", "Cost $1 million"), "#+TITLE: Cost $1 million\n");
    }

    #[test]
    fn inserts_title_when_the_file_has_none() {
        let content = ":PROPERTIES:\n:ID: abc\n:END:\nBody\n";
        assert_eq!(
            set_file_title(content, "New"),
            ":PROPERTIES:\n:ID: abc\n:END:\n#+TITLE: New\nBody\n"
        );
    }

    #[test]
    fn renames_a_headline_that_has_a_planning_line() {
        let content = "* TODO Old headline\nSCHEDULED: <2026-08-17 Mon>\n:PROPERTIES:\n:ID: abc\n:END:\nBody\n";
        let out = update_headline_title(content, "abc", "New headline").unwrap();
        assert!(out.starts_with("* TODO New headline\n"), "got: {out}");
        assert!(out.contains("SCHEDULED: <2026-08-17 Mon>"));
    }

    #[test]
    fn renames_a_headline_without_planning() {
        let content = "** Old :work:\n:PROPERTIES:\n:ID: abc\n:END:\n";
        let out = update_headline_title(content, "abc", "New").unwrap();
        assert!(out.starts_with("** New :work:\n"), "got: {out}");
    }

    #[test]
    fn headline_rename_preserves_trailing_newline_state() {
        let with_nl = "* Old\n:PROPERTIES:\n:ID: abc\n:END:\n";
        assert!(update_headline_title(with_nl, "abc", "New").unwrap().ends_with('\n'));
        let without_nl = "* Old\n:PROPERTIES:\n:ID: abc\n:END:";
        assert!(!update_headline_title(without_nl, "abc", "New").unwrap().ends_with('\n'));
    }

    #[test]
    fn headline_rename_leaves_other_nodes_alone() {
        let content = "* One\n:PROPERTIES:\n:ID: a\n:END:\n* Two\n:PROPERTIES:\n:ID: b\n:END:\n";
        let out = update_headline_title(content, "b", "Renamed").unwrap();
        assert!(out.contains("* One\n"));
        assert!(out.contains("* Renamed\n"));
    }

    fn rename(content: &str, id: &str, title: &str) -> String {
        update_headline_title(content, id, title).unwrap()
    }

    #[test]
    fn headline_rename_keeps_priority_comment_and_tags_exactly() {
        assert_eq!(
            rename("* TODO [#A] Old :work:\n:PROPERTIES:\n:ID: x\n:END:\n", "x", "New"),
            "* TODO [#A] New :work:\n:PROPERTIES:\n:ID: x\n:END:\n"
        );
        assert_eq!(
            rename("* COMMENT Old\n:PROPERTIES:\n:ID: x\n:END:\n", "x", "New"),
            "* COMMENT New\n:PROPERTIES:\n:ID: x\n:END:\n"
        );
        // Tab-separated, column-aligned tags keep their spacing.
        assert_eq!(
            rename("** Old\t\t:a:b:\n:PROPERTIES:\n:ID: x\n:END:\n", "x", "New"),
            "** New\t\t:a:b:\n:PROPERTIES:\n:ID: x\n:END:\n"
        );
    }

    #[test]
    fn headline_rename_honours_the_files_own_todo_keywords() {
        let content = "#+TODO: WAIT | GONE\n* WAIT Old\n:PROPERTIES:\n:ID: x\n:END:\n";
        assert_eq!(
            rename(content, "x", "New"),
            "#+TODO: WAIT | GONE\n* WAIT New\n:PROPERTIES:\n:ID: x\n:END:\n"
        );
    }

    #[test]
    fn headline_rename_keeps_crlf_line_endings() {
        let content = "* Old\r\n:PROPERTIES:\r\n:ID: x\r\n:END:\r\nBody\r\n";
        assert_eq!(
            rename(content, "x", "New"),
            "* New\r\n:PROPERTIES:\r\n:ID: x\r\n:END:\r\nBody\r\n"
        );
    }

    #[test]
    fn headline_rename_matches_the_exact_id_in_a_drawer_only() {
        let content = "* Prefix\n:PROPERTIES:\n:ID: abc-2\n:END:\n* Prose\nsee\n:ID: abc\n* Real\n:PROPERTIES:\n:ID: abc\n:END:\n";
        let out = rename(content, "abc", "Renamed");
        assert!(out.contains("* Prefix\n"), "got: {out}");
        assert!(out.contains("* Prose\n"), "got: {out}");
        assert!(out.contains("* Renamed\n"), "got: {out}");
    }

    #[test]
    fn headline_rename_reports_a_missing_id() {
        assert!(update_headline_title("* Old\n:PROPERTIES:\n:ID: other\n:END:\n", "x", "New").is_none());
        assert!(update_headline_title(":ID: x\n* Old\n", "x", "New").is_none());
    }

    #[test]
    fn headline_rename_of_an_untitled_headline_keeps_the_tags_apart() {
        assert_eq!(
            rename("* TODO :work:\n:PROPERTIES:\n:ID: x\n:END:\n", "x", "New"),
            "* TODO New :work:\n:PROPERTIES:\n:ID: x\n:END:\n"
        );
    }

    #[test]
    fn file_title_keeps_crlf_and_bom() {
        assert_eq!(set_file_title("#+TITLE: Old\r\nBody\r\n", "New"), "#+TITLE: New\r\nBody\r\n");
        assert_eq!(
            set_file_title("\u{feff}:PROPERTIES:\r\n:ID: x\r\n:END:\r\nBody\r\n", "New"),
            "\u{feff}:PROPERTIES:\r\n:ID: x\r\n:END:\r\n#+TITLE: New\r\nBody\r\n"
        );
        assert_eq!(set_file_title(":PROPERTIES:\n:ID: x\n:END:", "New"), ":PROPERTIES:\n:ID: x\n:END:\n#+TITLE: New");
    }

    #[test]
    fn link_descriptions_cannot_close_the_link() {
        assert_eq!(link_description("a]] b [c]"), "a)) b (c)");
    }
}
