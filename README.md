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

- **Rendered view** with clickable links, collapsible headings, tables, code blocks, inline images, bold/italic/code formatting
- **Source editor** powered by CodeMirror 6 with org-mode syntax highlighting, folding, and live preview
- **Backlinks & forward links** in a slide-out drawer with source context
- **Full-text search** across titles and file content with highlighted snippets
- **Graph view** with d3-force visualization, tag-based coloring, orphan node detection, and random node discovery
- **Daily notes** with quick-access calendar
- **Tag browser** with filterable dropdown and inline filtering on the files tab
- **Node creation** with auto-generated `:ID:` properties
- **Node refactoring** — rename a node and update all backlink descriptions across the vault
- **Image import** — pick an image, copy it to the vault, and insert an org link
- **Export** to Markdown and HTML
- **Theme system** — light, dark, and system-auto modes
- **File watcher** (desktop) and re-scan on focus (mobile)
- **Round-trip fidelity** — the org parser preserves all whitespace and formatting

## Architecture

```
Frontend (Svelte 5 + SvelteKit)     Rust Backend (Tauri v2)
+-----------+  +--------+           +------------+  +-----------+
| Rendered  |  | CM6    |    IPC    | org-parser |  | db        |
| View      |  | Editor |  <-----> | (custom)   |  | (SQLite)  |
+-----------+  +--------+           +------------+  +-----------+
| Graph | Search | Daily |           | File Watch | Export      |
+---------------------+             +----------------------------+
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

# Run tests
cargo test -p org-parser -p db
```

## Project Structure

```
mycelium/
├── crates/
│   ├── org-parser/          # Rust org-mode parser (round-trip fidelity)
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
│           ├── schema.rs    # Org-roam v2 compatible schema + FTS5
│           ├── index.rs     # File indexer
│           ├── query.rs     # Backlinks, search, graph, tags, daily
│           └── sync.rs      # Hash-based incremental vault sync
├── src-tauri/               # Tauri v2 app shell
│   └── src/
│       ├── commands/        # IPC: vault, node, editor, graph, daily, tags
│       ├── watcher.rs       # File system watcher (notify crate)
│       └── state.rs         # Shared app state
├── src/                     # Frontend (Svelte 5 + SvelteKit)
│   ├── lib/
│   │   ├── components/
│   │   │   ├── editor/      # RenderedView, OrgEditor, EditorToolbar
│   │   │   ├── sidebar/     # BacklinkPanel, FileTree, OutlinePanel
│   │   │   ├── graph/       # GraphView (d3-force)
│   │   │   └── common/      # MobileNav, QuickSwitcher, CreateNodeModal
│   │   ├── codemirror/      # CM6 org-mode language, extensions
│   │   ├── stores/          # Svelte 5 runes stores
│   │   └── tauri/           # Typed IPC wrappers + mock data
│   └── routes/              # SvelteKit pages
├── tests/fixtures/          # Sample org-roam vault for testing
├── Cargo.toml               # Rust workspace
├── package.json             # Node dependencies
└── LICENSE                  # Apache 2.0
```

## Database Schema

Mycelium uses an org-roam v2 compatible SQLite schema:

| Table | Purpose |
|-------|---------|
| `files` | Indexed files with content hash for incremental sync |
| `nodes` | Org nodes (file-level and headline-level with `:ID:`) |
| `links` | `[[id:]]` links between nodes |
| `tags` | Tags from headlines (`:tag1:tag2:`) and `#+FILETAGS:` |
| `aliases` | `:ROAM_ALIASES:` for alternative node names |
| `refs` | `:ROAM_REFS:` for external references |
| `nodes_fts` | FTS5 index on node titles |
| `files_fts` | FTS5 index on file body content (Porter stemming) |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+E` | Toggle read/edit mode |
| `Cmd+K` | Insert link (quick switcher) |
| `Cmd+S` | Save file |
| `Cmd+Click` | Follow link in source mode |

## Mobile

The app is built with Tauri v2 which supports iOS and Android. To build for mobile:

```bash
# iOS (requires macOS + Xcode)
cargo tauri ios init
cargo tauri ios dev

# Android (requires Android Studio + SDK)
cargo tauri android init
cargo tauri android dev
```

## License

Apache 2.0 — see [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome! Please open an issue before submitting large changes.
