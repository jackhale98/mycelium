import { ViewPlugin, type EditorView } from '@codemirror/view';

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
				const lineText = line.text;

				// Check if the click is near a checkbox
				const unchecked = lineText.indexOf('[ ]');
				const checked = lineText.indexOf('[X]');
				const checkedLower = lineText.indexOf('[x]');

				const posInLine = pos - line.from;

				if (unchecked !== -1 && posInLine >= unchecked && posInLine <= unchecked + 3) {
					event.preventDefault();
					this.view.dispatch({
						changes: { from: line.from + unchecked, to: line.from + unchecked + 3, insert: '[X]' },
					});
				} else if (checked !== -1 && posInLine >= checked && posInLine <= checked + 3) {
					event.preventDefault();
					this.view.dispatch({
						changes: { from: line.from + checked, to: line.from + checked + 3, insert: '[ ]' },
					});
				} else if (checkedLower !== -1 && posInLine >= checkedLower && posInLine <= checkedLower + 3) {
					event.preventDefault();
					this.view.dispatch({
						changes: { from: line.from + checkedLower, to: line.from + checkedLower + 3, insert: '[ ]' },
					});
				}
			}

			destroy() {
				this.view.dom.removeEventListener('click', this.handleClick);
			}
		}
	);
}
