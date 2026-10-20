import { EditorSelection, type EditorState, type StateCommand } from '@codemirror/state';

/**
 * A list item line: indentation, bullet (`-`, `+`, or `1.` / `1)`), optional
 * checkbox, then the item's text. `*` bullets are left out on purpose: at the
 * start of a line a star is a headline.
 */
const ITEM = /^(\s*)([-+]|\d+[.)])([ \t]+)(\[[ Xx-]\][ \t]+)?/;

/** How far back to look for an enclosing block before giving up. */
const BLOCK_SCAN_LIMIT = 300;

/** `true` when line `lineNo` (1-based) sits inside a `#+begin_` … `#+end_` block. */
function insideBlock(state: EditorState, lineNo: number): boolean {
	const stop = Math.max(1, lineNo - BLOCK_SCAN_LIMIT);
	for (let n = lineNo - 1; n >= stop; n -= 1) {
		const text = state.doc.line(n).text.trimStart().toLowerCase();
		if (text.startsWith('#+end_')) return false;
		if (text.startsWith('#+begin_')) return true;
	}
	return false;
}

/** The bullet for the item after one with `bullet`: numbers count up. */
function nextBullet(bullet: string): string {
	const numbered = /^(\d+)([.)])$/.exec(bullet);
	return numbered ? `${Number(numbered[1]) + 1}${numbered[2]}` : bullet;
}

/**
 * Enter inside an org list item starts the next item, as org-mode's
 * `M-RET` does and as every notes app does on Enter.
 *
 * - `- text|` → a new `- ` item; text after the cursor moves into it.
 * - `- [X] done|` → a new `- [ ] ` item, unchecked.
 * - `3. text|` → `4. `.
 * - Enter on an empty item ends the list, removing the bullet, so a second
 *   Enter is how you get out — the same as Notes or Bear.
 *
 * Falls through to the default Enter for anything else: a selection, the
 * cursor inside the bullet itself, headlines, ordinary text, and lists inside
 * source or example blocks, where a dash is code.
 */
export const continueOrgList: StateCommand = ({ state, dispatch }) => {
	if (state.selection.ranges.length !== 1) return false;
	const range = state.selection.main;
	if (!range.empty) return false;

	const line = state.doc.lineAt(range.head);
	const match = ITEM.exec(line.text);
	if (!match) return false;

	const markerEnd = line.from + match[0].length;
	if (range.head < markerEnd) return false;
	if (insideBlock(state, line.number)) return false;

	const [, indent, bullet, gap, checkbox] = match;
	const itemText = line.text.slice(match[0].length);

	if (itemText.trim() === '') {
		// An empty item: end the list instead of adding another.
		dispatch(
			state.update({
				changes: { from: line.from, to: line.to, insert: '' },
				selection: EditorSelection.cursor(line.from),
				scrollIntoView: true,
				userEvent: 'input',
			})
		);
		return true;
	}

	const prefix = `${indent}${nextBullet(bullet)}${gap}${checkbox ? '[ ] ' : ''}`;
	const insert = `\n${prefix}`;
	// Text after the cursor carries into the new item, minus the spaces that
	// separated it from the text before.
	const after = state.doc.sliceString(range.head, line.to);
	const trimmed = after.replace(/^[ \t]+/, '');
	dispatch(
		state.update({
			changes: { from: range.head, to: line.to, insert: insert + trimmed },
			selection: EditorSelection.cursor(range.head + insert.length),
			scrollIntoView: true,
			userEvent: 'input',
		})
	);
	return true;
};
