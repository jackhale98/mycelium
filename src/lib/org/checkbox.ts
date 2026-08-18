/// Checkbox list items and the statistics cookies that summarise them.
///
/// Toggling targets an explicit list item, never the first `[ ]` found on the
/// line: a line may hold several boxes, or literal bracket text before one.

import { isHeadlineLine } from './headline';

const ITEM_RE = /^([ \t]*)(?:[-+*]|\d+[.)])[ \t]+(?:\[([ xX-])\][ \t]*)?/;
const LIST_ITEM_RE = /^[ \t]*(?:[-+*]|\d+[.)])[ \t]+/;
const COOKIE_RE = /\[(?:\d*\/\d*|\d{1,3}%)\]/g;

export type CheckboxState = 'unchecked' | 'checked' | 'partial';

export interface CheckboxItem {
	lineIndex: number;
	indent: string;
	state: CheckboxState;
	/** Character offset of the `[` within the line. */
	offset: number;
}

function stateOf(mark: string): CheckboxState {
	if (mark === 'x' || mark === 'X') return 'checked';
	if (mark === '-') return 'partial';
	return 'unchecked';
}

function markOf(state: CheckboxState): string {
	if (state === 'checked') return 'X';
	if (state === 'partial') return '-';
	return ' ';
}

/** Parse a single line as a checkbox list item. Returns `null` when it is not one. */
export function parseCheckboxLine(line: string, lineIndex = 0): CheckboxItem | null {
	const m = ITEM_RE.exec(line);
	if (!m || m[2] === undefined) return null;
	return {
		lineIndex,
		indent: m[1],
		state: stateOf(m[2]),
		offset: line.indexOf('[', m[1].length),
	};
}

/** Every checkbox item in the document, in source order. */
export function listCheckboxes(lines: readonly string[]): CheckboxItem[] {
	const items: CheckboxItem[] = [];
	lines.forEach((line, index) => {
		const item = parseCheckboxLine(line, index);
		if (item) items.push(item);
	});
	return items;
}

/** Set the state of the checkbox on `lineIndex`. Lines without one are untouched. */
export function setCheckbox(
	lines: readonly string[],
	lineIndex: number,
	state: CheckboxState
): string[] {
	const next = [...lines];
	const item = lineIndex >= 0 && lineIndex < next.length ? parseCheckboxLine(next[lineIndex], lineIndex) : null;
	if (!item || item.offset < 0) return next;
	const line = next[lineIndex];
	next[lineIndex] = `${line.slice(0, item.offset)}[${markOf(state)}]${line.slice(item.offset + 3)}`;
	return next;
}

/** Toggle the checkbox on `lineIndex` between checked and unchecked. */
export function toggleCheckbox(lines: readonly string[], lineIndex: number): string[] {
	const item = lineIndex >= 0 && lineIndex < lines.length ? parseCheckboxLine(lines[lineIndex], lineIndex) : null;
	if (!item) return [...lines];
	return setCheckbox(lines, lineIndex, item.state === 'checked' ? 'unchecked' : 'checked');
}

/** Toggle the checkbox on the line containing `offset` characters into the text. */
export function toggleCheckboxAtOffset(lines: readonly string[], offset: number): string[] {
	let cursor = 0;
	for (let i = 0; i < lines.length; i += 1) {
		const end = cursor + lines[i].length;
		if (offset <= end) return toggleCheckbox(lines, i);
		cursor = end + 1;
	}
	return [...lines];
}

function indentWidth(indent: string): number {
	let width = 0;
	for (const ch of indent) width += ch === '\t' ? 8 : 1;
	return width;
}

/**
 * Direct checkbox children of the line at `parentIndex`: the items one nesting
 * level below it, stopping at the next sibling, a headline, or a blank line.
 */
function directChildren(lines: readonly string[], parentIndex: number): CheckboxItem[] {
	const line = lines[parentIndex];
	// A headline owns every item under it; a list item owns the items indented
	// deeper than itself, whether or not it carries a checkbox of its own.
	const parentIndent = isHeadlineLine(line)
		? -1
		: LIST_ITEM_RE.test(line)
			? indentWidth(/^[ \t]*/.exec(line)![0])
			: null;
	if (parentIndent === null) return [];

	const children: CheckboxItem[] = [];
	let childIndent: number | null = null;

	for (let i = parentIndex + 1; i < lines.length; i += 1) {
		const line = lines[i];
		if (isHeadlineLine(line)) break;
		if (line.trim() === '') {
			if (children.length > 0) break;
			continue;
		}

		const item = parseCheckboxLine(line, i);
		const width = indentWidth(/^[ \t]*/.exec(line)![0]);
		if (width <= parentIndent) break;
		if (!item) continue;

		if (childIndent === null) childIndent = indentWidth(item.indent);
		if (indentWidth(item.indent) === childIndent) children.push(item);
	}

	return children;
}

/**
 * Recompute every `[n/m]` and `[NN%]` cookie from the checkbox items beneath it.
 * Cookies on lines with no checkbox children are left alone.
 */
export function recomputeCookies(lines: readonly string[]): string[] {
	const next = [...lines];

	for (let i = 0; i < next.length; i += 1) {
		COOKIE_RE.lastIndex = 0;
		if (!COOKIE_RE.test(next[i])) continue;

		const children = directChildren(next, i);
		if (children.length === 0) continue;

		const done = children.filter((c) => c.state === 'checked').length;
		const total = children.length;
		const percent = total === 0 ? 0 : Math.floor((done / total) * 100);

		next[i] = next[i].replace(COOKIE_RE, (cookie) =>
			cookie.endsWith('%]') ? `[${percent}%]` : `[${done}/${total}]`
		);
	}

	return next;
}

/** Toggle a checkbox and bring every affected statistics cookie up to date. */
export function toggleCheckboxAndCookies(lines: readonly string[], lineIndex: number): string[] {
	return recomputeCookies(toggleCheckbox(lines, lineIndex));
}
