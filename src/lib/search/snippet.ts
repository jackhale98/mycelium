/// Safe rendering of FTS snippets returned by the backend.
/// SQLite's `snippet(files_fts, ...)` wraps matches in `<<`/`>>` over raw org
/// body text, so the text must be HTML-escaped before any markup is added.

const MARK_CLASS = 'bg-mycelium-200 dark:bg-mycelium-800 rounded px-0.5';

/** Escape a string for safe interpolation into HTML text or attribute context. */
export function escapeHtml(value: string): string {
	return value
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;')
		.replace(/'/g, '&#39;');
}

/** Escape a snippet, then turn its `<<match>>` markers into `<mark>` elements. */
export function highlightSnippet(snippet: string): string {
	return escapeHtml(snippet).replace(
		/&lt;&lt;([\s\S]*?)&gt;&gt;/g,
		`<mark class="${MARK_CLASS}">$1</mark>`
	);
}
