import {
	ViewPlugin,
	Decoration,
	type DecorationSet,
	WidgetType,
	type EditorView,
	type ViewUpdate,
} from '@codemirror/view';
import { RangeSetBuilder, type EditorState } from '@codemirror/state';

class LinkWidget extends WidgetType {
	constructor(
		readonly description: string,
		readonly target: string
	) {
		super();
	}

	eq(other: LinkWidget) {
		return this.description === other.description && this.target === other.target;
	}

	toDOM() {
		const span = document.createElement('span');
		span.className = 'cm-org-link-preview';
		span.textContent = this.description || this.target;
		span.dataset.target = this.target;
		return span;
	}
}

class CheckboxWidget extends WidgetType {
	constructor(readonly checked: boolean) {
		super();
	}

	eq(other: CheckboxWidget) {
		return this.checked === other.checked;
	}

	toDOM() {
		const span = document.createElement('span');
		span.className = 'cm-org-checkbox';
		span.textContent = this.checked ? '\u2611' : '\u2610';
		return span;
	}
}

class BoldWidget extends WidgetType {
	constructor(readonly text: string) { super(); }
	eq(other: BoldWidget) { return this.text === other.text; }
	toDOM() {
		const b = document.createElement('strong');
		b.textContent = this.text;
		return b;
	}
}

class ItalicWidget extends WidgetType {
	constructor(readonly text: string) { super(); }
	eq(other: ItalicWidget) { return this.text === other.text; }
	toDOM() {
		const em = document.createElement('em');
		em.textContent = this.text;
		return em;
	}
}

class CodeWidget extends WidgetType {
	constructor(readonly text: string) { super(); }
	eq(other: CodeWidget) { return this.text === other.text; }
	toDOM() {
		const code = document.createElement('code');
		code.className = 'cm-org-inline-code';
		code.textContent = this.text;
		return code;
	}
}

class StrikeWidget extends WidgetType {
	constructor(readonly text: string) { super(); }
	eq(other: StrikeWidget) { return this.text === other.text; }
	toDOM() {
		const s = document.createElement('s');
		s.textContent = this.text;
		s.style.color = '#9ca3af';
		return s;
	}
}

function buildLivePreview(state: EditorState): DecorationSet {
	const builder = new RangeSetBuilder<Decoration>();
	const cursorHead = state.selection.main.head;
	const cursorLine = state.doc.lineAt(cursorHead).number;
	const doc = state.doc;

	for (let lineNum = 1; lineNum <= doc.lines; lineNum++) {
		const line = doc.line(lineNum);
		const lineText = line.text;

		// Don't decorate the active line — let the user see raw syntax
		if (lineNum === cursorLine) continue;

		const decos: { from: number; to: number; deco: Decoration }[] = [];

		// Links: [[target][description]] or [[target]]
		const linkRegex = /\[\[([^\]]+?)(?:\]\[([^\]]*))?\]\]/g;
		let lm;
		while ((lm = linkRegex.exec(lineText)) !== null) {
			const from = line.from + lm.index;
			const to = from + lm[0].length;
			const target = lm[1];
			const desc = lm[2] ?? target.replace(/^id:/, '');
			decos.push({
				from,
				to,
				deco: Decoration.replace({ widget: new LinkWidget(desc, target) }),
			});
		}

		// Checkboxes: [ ] or [X] or [x]
		const cbRegex = /\[([ Xx-])\]/g;
		let cbm;
		while ((cbm = cbRegex.exec(lineText)) !== null) {
			// Only replace if preceded by "- " list marker context
			const before = lineText.substring(0, cbm.index);
			if (/[-+]\s$/.test(before) || /\d+[.)]\s$/.test(before)) {
				const from = line.from + cbm.index;
				const to = from + 3;
				const checked = cbm[1] === 'X' || cbm[1] === 'x';
				decos.push({
					from,
					to,
					deco: Decoration.replace({ widget: new CheckboxWidget(checked) }),
				});
			}
		}

		// Bold: *text*
		const boldRegex = /(?:^|[\s(])\*([^\s*](?:[^*]*[^\s*])?)\*(?:[\s.,;:!?)]|$)/g;
		let bm;
		while ((bm = boldRegex.exec(lineText)) !== null) {
			const starIdx = lineText.indexOf('*' + bm[1] + '*', bm.index);
			if (starIdx >= 0) {
				const from = line.from + starIdx;
				const to = from + bm[1].length + 2;
				decos.push({
					from,
					to,
					deco: Decoration.replace({ widget: new BoldWidget(bm[1]) }),
				});
			}
		}

		// Italic: /text/
		const italicRegex = /(?:^|[\s(])\/([^\s/](?:[^/]*[^\s/])?)\/(?:[\s.,;:!?)]|$)/g;
		let im;
		while ((im = italicRegex.exec(lineText)) !== null) {
			const slashIdx = lineText.indexOf('/' + im[1] + '/', im.index);
			if (slashIdx >= 0) {
				const from = line.from + slashIdx;
				const to = from + im[1].length + 2;
				decos.push({
					from,
					to,
					deco: Decoration.replace({ widget: new ItalicWidget(im[1]) }),
				});
			}
		}

		// Code: ~text~
		const codeRegex = /(?:^|[\s(])~([^\s~](?:[^~]*[^\s~])?)~(?:[\s.,;:!?)]|$)/g;
		let cm;
		while ((cm = codeRegex.exec(lineText)) !== null) {
			const tildeIdx = lineText.indexOf('~' + cm[1] + '~', cm.index);
			if (tildeIdx >= 0) {
				const from = line.from + tildeIdx;
				const to = from + cm[1].length + 2;
				decos.push({
					from,
					to,
					deco: Decoration.replace({ widget: new CodeWidget(cm[1]) }),
				});
			}
		}

		// Strikethrough: +text+
		const strikeRegex = /(?:^|[\s(])\+([^\s+](?:[^+]*[^\s+])?)\+(?:[\s.,;:!?)]|$)/g;
		let sm;
		while ((sm = strikeRegex.exec(lineText)) !== null) {
			const plusIdx = lineText.indexOf('+' + sm[1] + '+', sm.index);
			if (plusIdx >= 0) {
				const from = line.from + plusIdx;
				const to = from + sm[1].length + 2;
				decos.push({
					from,
					to,
					deco: Decoration.replace({ widget: new StrikeWidget(sm[1]) }),
				});
			}
		}

		// Sort by position and add to builder (must be in order)
		decos.sort((a, b) => a.from - b.from);

		// Filter out overlapping decorations
		let lastEnd = 0;
		for (const d of decos) {
			if (d.from >= lastEnd) {
				builder.add(d.from, d.to, d.deco);
				lastEnd = d.to;
			}
		}
	}

	return builder.finish();
}

const livePreviewPlugin = ViewPlugin.fromClass(
	class {
		decorations: DecorationSet;

		constructor(view: EditorView) {
			this.decorations = buildLivePreview(view.state);
		}

		update(update: ViewUpdate) {
			if (update.docChanged || update.selectionSet || update.viewportChanged) {
				this.decorations = buildLivePreview(update.state);
			}
		}
	},
	{
		decorations: (v) => v.decorations,
	}
);

const livePreviewTheme = EditorView.baseTheme({
	'.cm-org-link-preview': {
		color: '#16a34a',
		textDecoration: 'underline',
		cursor: 'pointer',
	},
	'.cm-org-checkbox': {
		fontSize: '1.1em',
		cursor: 'pointer',
		marginRight: '2px',
	},
	'.cm-org-inline-code': {
		fontFamily: 'monospace',
		fontSize: '0.9em',
		backgroundColor: '#f1f5f9',
		padding: '1px 4px',
		borderRadius: '3px',
		color: '#dc2626',
	},
});

export function orgLivePreview() {
	return [livePreviewPlugin, livePreviewTheme];
}
