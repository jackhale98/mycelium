import { ViewPlugin, type EditorView } from '@codemirror/view';

/// Extension that navigates to org-roam nodes when Cmd/Ctrl+clicking [[id:...]] links.
/// Regular clicks just place the cursor for editing. Only Cmd/Ctrl+Click navigates.
export function orgLinkClickExtension(onNavigate: (id: string) => void) {
	return ViewPlugin.fromClass(
		class {
			constructor(readonly view: EditorView) {
				this.handleClick = this.handleClick.bind(this);
				view.dom.addEventListener('click', this.handleClick);
			}

			handleClick(event: MouseEvent) {
				// Only navigate on Cmd+Click (Mac) or Ctrl+Click (other)
				if (!event.metaKey && !event.ctrlKey) return;

				const target = event.target as HTMLElement;
				if (!target) return;

				// Check if clicked on a live-preview link widget
				const linkWidget = target.closest('.cm-org-link-preview') as HTMLElement | null;
				if (linkWidget) {
					const linkTarget = linkWidget.dataset.target;
					if (linkTarget) {
						event.preventDefault();
						event.stopPropagation();
						const id = linkTarget.replace(/^id:/, '');
						onNavigate(id);
						return;
					}
				}

				// Check raw text for [[id:...]] links at click position
				const pos = this.view.posAtCoords({ x: event.clientX, y: event.clientY });
				if (pos === null) return;

				const line = this.view.state.doc.lineAt(pos);
				const lineText = line.text;
				const posInLine = pos - line.from;

				const linkRegex = /\[\[id:([^\]]+?)(?:\]\[([^\]]*))?\]\]/g;
				let match;
				while ((match = linkRegex.exec(lineText)) !== null) {
					const start = match.index;
					const end = start + match[0].length;
					if (posInLine >= start && posInLine <= end) {
						event.preventDefault();
						event.stopPropagation();
						onNavigate(match[1]);
						return;
					}
				}
			}

			destroy() {
				this.view.dom.removeEventListener('click', this.handleClick);
			}
		}
	);
}
