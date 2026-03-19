# Mycelium

<p align="center">
  <img src="app-image.svg" width="128" height="128" alt="Mycelium logo" />
</p>

<p align="center">
  An open-source, cross-platform knowledge base for <a href="https://www.orgroam.com/">org-roam</a> vaults.<br/>
  Built with Rust, Tauri v2, Svelte 5, and SQLite.
</p>

---

Mycelium lets you view, edit, search, and navigate your org-roam notes on desktop and mobile — no Emacs required. Named after the underground fungal networks that connect trees in a forest, Mycelium connects your notes through the same bidirectional linking that makes org-roam powerful.

## Features

### Reading & Navigation
- **Rendered view** — clickable links, collapsible headings, tables, code blocks, inline images, bold/italic/code/verbatim formatting. Property drawers and metadata hidden automatically.
- **Backlinks & forward links** — slide-out drawer showing which nodes link here (with source context) and where this node links to
- **Unlinked mentions** — discovers files that mention a node's title without an explicit link
- **Graph view** — d3-force visualization with tag-based coloring, zoom-to-fit, orphan node detection, and clickable node navigation

### Editing
- **Source editor** — CodeMirror 6 with org-mode syntax highlighting, folding, and Cmd+Click link navigation
- **Native keyboard toolbar** (iOS + Android) — formatting bar above the keyboard with buttons for headings, TODO, priority, tags, bold/italic/underline/strikethrough, code, lists, tables, timestamps, and more
- **TODO management** — native picker with user-configured keywords, cycles through states
- **Priority management** — native picker with configurable priority levels
- **Tags** — toggle file tags from a picker showing all vault tags, or add new ones. Uses `#+FILETAGS:` format.
- **Deadline & Schedule** — native date picker with calendar view, pre-selects existing dates, supports updating and removing
- **Table insertion** — size picker (2x2 through 5x3) generates org tables
- **Create node at point** — type a name in Cmd+K, select "Create and insert link" to make a new node and insert `[[id:uuid][title]]` in one step
- **Make heading into node** — "ID" button adds `:PROPERTIES:/:ID:/:END:` to the heading at point, turning it into an org-roam node
- **Node refactoring** — rename a node and automatically update all backlink descriptions across the vault
- **Image import** — pick an image file, copy it to the vault's `images/` directory, insert `[[file:images/name.png]]`
- **Org-roam filename convention** — new files named `YYYYMMDDHHmmss-slug.org`

### Organization
- **Agenda view** — weekly view with overdue items + tasks tab with filtering. All TODO/SCHEDULED/DEADLINE items from SQLite headlines table. Tasks sorted by deadline > scheduled > no date, then by priority. Tap items to navigate to their node.
- **Quick capture** — floating button to jot a thought in 2 seconds. Appends timestamped entry to today's daily note.
- **Daily notes** — finds notes with date-formatted titles and files in `daily/` subdirectories
- **Tag browser** — filterable dropdown that filters the file list inline
- **Full-text search** — FTS5 with prefix matching across titles and body content with highlighted snippets

### Mobile (iOS + Android)
- **Native folder picker** (iOS) — `UIDocumentPickerViewController` with security-scoped bookmarks for persistent access across app launches, including subdirectory traversal
- **Native keyboard toolbar** — `inputAccessoryView` on iOS (same approach as iA Writer/Bear/Blink), `WindowInsetsCompat`-positioned toolbar on Android. All pickers (TODO, heading, priority, tags, dates, tables) use native platform UI.
- **Dark mode** — no flash on tab switching or app launch (inline dark-mode script + CSS background on html/body)
- **Client-side navigation** — SvelteKit `goto()` for instant tab switching, full reload for node-to-node navigation

### System
- **`#+ROAM_EXCLUDE: t`** — respected during indexing, excluded files don't appear in search/graph/agenda
- **Regex-based link extraction** — scans raw file content with regex for `[[id:...]]` links, catching links in all contexts (paragraphs, lists, verbatim, preamble)
- **Headlines table** — indexes ALL headlines from ALL org files for agenda support, not just org-roam nodes
- **Export** to Markdown and HTML
- **Theme system** — light, dark, and system-auto modes
- **Configurable org settings** — TODO keywords, DONE keywords, and priority levels (persisted to localStorage)
- **File watcher** (desktop) and re-scan on focus (mobile)
- **Database rebuild** — settings option to drop and re-index from scratch
- **Round-trip fidelity** — the org parser preserves all whitespace and formatting (87+ tests)
- **Database stored in app data** — not in the vault directory, so it won't pollute your git repo

## Architecture

```
Frontend (Svelte 5 + SvelteKit)     Rust Backend (Tauri v2)
+-----------+  +--------+           +------------+  +-----------+
| Rendered  |  | CM6    |    IPC    | org-parser |  | db        |
| View      |  | Editor |  <-----> | (custom)   |  | (SQLite)  |
+-----------+  +--------+           +------------+  +-----------+
| Graph | Search | Daily |           | File Watch | Export      |
+---------------------+             +----------------------------+
                                     Native Plugins (Swift/Kotlin)
                                     +----------------------------+
                                     | Folder Picker | Keyboard   |
                                     | Toolbar | Date/TODO/Tag    |
                                     | Pickers                    |
                                     +----------------------------+
```

## Prerequisites

- **Rust** 1.70+ with `cargo`
- **Node.js** 18+ with `npm`
- **System dependencies** (Linux): `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`

## Quick Start

```bash
# Install dependencies
npm install

# Run in development mode (opens desktop window)
cargo tauri dev

# Or just the frontend (browser demo mode available)
npm run dev
```

Open `http://localhost:5173` in a browser and click "Try Demo Mode" to explore the UI without a vault.

## Building

```bash
# Build the frontend
npm run build

# Build the desktop app
cargo tauri build

# Build for iOS (requires macOS + Xcode)
cargo tauri ios init
cargo tauri ios build

# Build for Android (requires Android Studio + SDK)
cargo tauri android init
cargo tauri android build

# Run tests
cargo test -p org-parser -p db
```

## Project Structure

```
mycelium/
├── crates/
│   ├── org-parser/          # Rust org-mode parser (round-trip fidelity, 87+ tests)
│   │   ├── src/
│   │   │   ├── cst.rs       # Concrete syntax tree types
│   │   │   ├── headline.rs  # Headline parsing (TODO, priority, tags)
│   │   │   ├── property.rs  # :PROPERTIES: drawers
│   │   │   ├── link.rs      # [[id:]] links, inline markup
│   │   │   ├── block.rs     # #+BEGIN_SRC blocks
│   │   │   ├── table.rs     # | col | col | tables
│   │   │   ├── serialize.rs # CST back to org text
│   │   │   ├── export_md.rs # Org to Markdown
│   │   │   └── export_html.rs # Org to HTML
│   │   └── tests/
│   │       └── roundtrip.rs # serialize(parse(x)) == x
│   └── db/                  # SQLite database layer
│       └── src/
│           ├── schema.rs    # Org-roam v2 compatible schema + FTS5 + headlines
│           ├── index.rs     # File indexer + regex link extraction + headline indexing
│           ├── query.rs     # Backlinks, search (prefix matching), graph, tags, daily, agenda
│           └── sync.rs      # Mtime-based incremental vault sync with error surfacing
├── src-tauri/               # Tauri v2 app shell
│   └── src/
│       ├── commands/        # IPC: vault, node, editor, graph, daily, tags, picker
│       ├── watcher.rs       # File system watcher (notify crate)
│       └── state.rs         # Shared app state
├── tauri-plugin-folder-picker/  # Cross-platform Tauri v2 plugin
│   ├── ios/Sources/
│   │   ├── ExamplePlugin.swift     # Folder picker + security-scoped bookmarks
│   │   └── KeyboardToolbar.swift   # Native inputAccessoryView toolbar + pickers
│   ├── android/src/main/java/
│   │   └── FolderPickerPlugin.kt   # Android toolbar + native dialogs
│   └── src/                        # Rust plugin bridge (mobile.rs, desktop.rs)
├── src/                     # Frontend (Svelte 5 + SvelteKit)
│   ├── lib/
│   │   ├── components/
│   │   │   ├── editor/      # RenderedView, OrgEditor, EditorToolbar
│   │   │   ├── sidebar/     # BacklinkPanel, FileTree, OutlinePanel
│   │   │   ├── graph/       # GraphView (d3-force, zoom-to-fit)
│   │   │   └── common/      # MobileNav, QuickSwitcher, CreateNodeModal
│   │   ├── codemirror/      # CM6 org-mode language, extensions
│   │   ├── stores/          # Svelte 5 runes stores (vault, editor, navigation, theme, orgconfig)
│   │   └── tauri/           # Typed IPC wrappers + mock data
│   └── routes/              # SvelteKit pages (vault, node, graph, search, daily, agenda, settings)
├── .github/workflows/       # CI + iOS/Android/Desktop release builds
├── Cargo.toml               # Rust workspace
├── package.json             # Node dependencies
└── LICENSE                  # Apache 2.0
```

## Database Schema

Mycelium uses an org-roam v2 compatible SQLite schema:

| Table | Purpose |
|-------|---------|
| `files` | Indexed files with content hash and mtime for incremental sync |
| `nodes` | Org nodes (file-level and headline-level with `:ID:`) |
| `links` | `[[id:]]` links between nodes (regex-extracted from raw content) |
| `tags` | Tags from headlines (`:tag1:tag2:`) and `#+FILETAGS:` |
| `aliases` | `:ROAM_ALIASES:` for alternative node names |
| `refs` | `:ROAM_REFS:` for external references |
| `headlines` | ALL headlines from ALL files (for agenda: TODO, SCHEDULED, DEADLINE) |
| `nodes_fts` | FTS5 index on node titles (prefix matching) |
| `files_fts` | FTS5 index on file body content (Porter stemming, prefix matching) |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+E` | Toggle read/edit mode |
| `Cmd+K` | Insert link (quick switcher) |
| `Cmd+S` | Save file |
| `Cmd+Click` | Follow link in source mode |

## Mobile Toolbar

The native toolbar appears above the keyboard on iOS and Android with these buttons:

| Button | Action |
|--------|--------|
| **Link** | Insert org-roam link via quick switcher |
| **H** | Insert heading (auto-detect level, or pick H1-H4) |
| **ID** | Add `:ID:` property to heading at point (make it a node) |
| **TODO** | Set TODO state from configured keywords |
| **[#]** | Set priority from configured levels |
| **Tag** | Toggle file tags from vault-wide tag list |
| **DL** | Set deadline with native date picker |
| **SC** | Set scheduled date with native date picker |
| **B I U S** | Bold, italic, underline, strikethrough |
| **<> =** | Code, verbatim |
| **Table** | Insert table with size picker |
| **{ }** | Source block |
| **Date** | Insert timestamp |

## License

Apache 2.0 — see [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome! Please open an issue before submitting large changes.
