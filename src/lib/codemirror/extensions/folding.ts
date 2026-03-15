import { foldGutter, foldService } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';

/// Org-mode folding: fold property drawers, blocks, and drawers
const orgFoldService = foldService.of((state: EditorState, lineStart: number) => {
	const line = state.doc.lineAt(lineStart);
	const text = line.text.trim();

	// Fold property drawer: :PROPERTIES: ... :END:
	if (text === ':PROPERTIES:') {
		return findEndMarker(state, line.number, ':END:');
	}

	// Fold blocks: #+BEGIN_xxx ... #+END_xxx
	const blockMatch = text.match(/^#\+(BEGIN_(\w+))/i);
	if (blockMatch) {
		const endMarker = `#+END_${blockMatch[2]}`;
		return findEndMarkerCI(state, line.number, endMarker);
	}

	// Fold generic drawers: :NAME: ... :END:
	const drawerMatch = text.match(/^:([A-Z_-]+):$/);
	if (drawerMatch && drawerMatch[1] !== 'END' && drawerMatch[1] !== 'PROPERTIES') {
		return findEndMarker(state, line.number, ':END:');
	}

	// Fold headlines: fold to next headline of same or higher level
	const headlineMatch = text.match(/^(\*+)\s/);
	if (headlineMatch) {
		const level = headlineMatch[1].length;
		for (let i = line.number + 1; i <= state.doc.lines; i++) {
			const nextLine = state.doc.line(i);
			const nextMatch = nextLine.text.match(/^(\*+)\s/);
			if (nextMatch && nextMatch[1].length <= level) {
				// Fold up to the line before the next same-level headline
				const prevLine = state.doc.line(i - 1);
				if (prevLine.number > line.number) {
					return { from: line.to, to: prevLine.to };
				}
				return null;
			}
		}
		// Fold to end of document
		const lastLine = state.doc.line(state.doc.lines);
		if (lastLine.number > line.number) {
			return { from: line.to, to: lastLine.to };
		}
	}

	return null;
});

function findEndMarker(state: EditorState, startLine: number, marker: string) {
	for (let i = startLine + 1; i <= state.doc.lines; i++) {
		const line = state.doc.line(i);
		if (line.text.trim() === marker) {
			return { from: state.doc.line(startLine).to, to: line.to };
		}
	}
	return null;
}

function findEndMarkerCI(state: EditorState, startLine: number, marker: string) {
	const upper = marker.toUpperCase();
	for (let i = startLine + 1; i <= state.doc.lines; i++) {
		const line = state.doc.line(i);
		if (line.text.trim().toUpperCase() === upper) {
			return { from: state.doc.line(startLine).to, to: line.to };
		}
	}
	return null;
}

export function orgFolding() {
	return [
		orgFoldService,
		foldGutter({
			markerDOM(open) {
				const span = document.createElement('span');
				span.textContent = open ? '\u25BE' : '\u25B8';
				span.style.cursor = 'pointer';
				span.style.color = '#9ca3af';
				span.style.fontSize = '0.9em';
				span.title = open ? 'Fold' : 'Unfold';
				return span;
			},
		}),
	];
}
