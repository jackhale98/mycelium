# Contributing to Mycelium

Thanks for your interest in contributing!

## Development Setup

1. Install Rust (1.70+), Node.js (18+), and system dependencies:
   ```bash
   # Ubuntu/Debian
   sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
   ```

2. Clone and install:
   ```bash
   git clone <repo-url>
   cd mycelium
   npm install
   ```

3. Run in development:
   ```bash
   cargo tauri dev
   ```

## Running Tests

```bash
# Rust tests (parser + database)
cargo test -p org-parser -p db

# Frontend type check
npm run check

# Build frontend
npm run build
```

## Code Structure

- **Rust changes**: Edit files in `crates/org-parser/`, `crates/db/`, or `src-tauri/`
- **Frontend changes**: Edit files in `src/`
- **Adding a Tauri command**: Add function in `src-tauri/src/commands/`, register in `src-tauri/src/lib.rs`, add TypeScript wrapper in `src/lib/tauri/commands.ts`, add mock handler in `src/lib/tauri/mock.ts`

## Guidelines

- Run `cargo test` before submitting
- Run `npm run build` to verify the frontend compiles
- Keep the org parser round-trip safe: `serialize(parse(text)) == text`
- Use inline styles in `RenderedView.svelte` (Safari compatibility)
- Use `document.createElement` not `innerHTML` in rendered view components

## Architecture Decisions

- **No innerHTML in rendered view**: Safari strips tags with certain attributes. All DOM is built programmatically.
- **`window.location.href` for navigation**: SvelteKit's `goto()` doesn't work reliably across page loads. Vault state is persisted in `sessionStorage`.
- **Inline styles over CSS classes**: Svelte's scoped styles don't apply to dynamically created DOM elements. All styling in RenderedView uses `element.style.cssText`.
- **Cmd+Click for source mode links**: Regular clicks in the editor should place the cursor, not navigate away.
