import { ViewPlugin, type EditorView } from '@codemirror/view';
import { parseCheckboxLine, recomputeCookies, setCheckbox } from '$lib/org';

/// Extension that toggles org-mode checkboxes when clicked: [ ] <-> [X]
export function orgCheckboxToggle() {
	return ViewPlugin.fromClass(
		class {
			constructor(readonly view: EditorView) {
				this.handleClick = this.handleClick.bind(this);
				view.dom.addEventListener('click', this.handleClick);
			}

			handleClick(event: MouseEvent) {
				const pos = this.view.posAtCoords({
					x: event.clientX,
					y: event.clientY,
				});
				if (pos === null) return;

				const line = this.view.state.doc.lineAt(pos);
				// The list item's own box, not the first bracket text on the line.
				const item = parseCheckboxLine(line.text);
				if (!item || item.offset < 0) return;

				const posInLine = pos - line.from;
				if (posInLine < item.offset || posInLine > item.offset + 3) return;

				event.preventDefault();

				const lines = this.view.state.doc.toJSON();
				const lineIndex = this.view.state.doc.lineAt(pos).number - 1;
				const next = recomputeCookies(
					setCheckbox(lines, lineIndex, item.state === 'checked' ? 'unchecked' : 'checked')
				);

				// Dispatch only the lines that actually changed, so the cursor and
				// undo history survive a toggle that also updates parent cookies.
				const changes = [];
				for (let i = 0; i < lines.length; i += 1) {
					if (lines[i] === next[i]) continue;
					const target = this.view.state.doc.line(i + 1);
					changes.push({ from: target.from, to: target.to, insert: next[i] });
				}
				if (changes.length > 0) this.view.dispatch({ changes });
			}

			destroy() {
				this.view.dom.removeEventListener('click', this.handleClick);
			}
		}
	);
}
