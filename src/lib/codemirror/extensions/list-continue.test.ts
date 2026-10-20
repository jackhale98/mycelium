import { describe, expect, it } from 'vitest';
import { EditorSelection, EditorState, type Transaction } from '@codemirror/state';
import { continueOrgList } from './list-continue';

/** Run Enter on `doc`, where `|` marks the cursor. Returns the result with `|`, or null if not handled. */
function enter(doc: string): string | null {
	const at = doc.indexOf('|');
	const state = EditorState.create({
		doc: doc.replace('|', ''),
		selection: EditorSelection.cursor(at),
	});
	let result: Transaction | null = null;
	const handled = continueOrgList({ state, dispatch: (tr) => (result = tr) });
	if (!handled || !result) return null;
	const next = (result as Transaction).state;
	const head = next.selection.main.head;
	const text = next.doc.toString();
	return text.slice(0, head) + '|' + text.slice(head);
}

describe('continueOrgList', () => {
	it('starts the next bullet', () => {
		expect(enter('- milk|')).toBe('- milk\n- |');
		expect(enter('  + nested|')).toBe('  + nested\n  + |');
	});

	it('counts numbered items up, keeping the delimiter', () => {
		expect(enter('1. one|')).toBe('1. one\n2. |');
		expect(enter('9) nine|')).toBe('9) nine\n10) |');
	});

	it('adds an unchecked box after a checkbox item', () => {
		expect(enter('- [X] done|')).toBe('- [X] done\n- [ ] |');
		expect(enter('- [ ] todo|')).toBe('- [ ] todo\n- [ ] |');
	});

	it('moves text after the cursor into the new item', () => {
		expect(enter('- buy |milk')).toBe('- buy \n- |milk');
	});

	it('ends the list on an empty item', () => {
		expect(enter('- a\n- |')).toBe('- a\n|');
		expect(enter('- a\n- [ ] |')).toBe('- a\n|');
	});

	it('leaves everything else to the default Enter', () => {
		expect(enter('plain text|')).toBeNull();
		expect(enter('* Headline|')).toBeNull();
		expect(enter('-|- not a bullet')).toBeNull();
		expect(enter('-| item')).toBeNull();
		expect(enter('#+begin_src sh\n- flag|\n#+end_src')).toBeNull();
	});

	it('continues lists after a closed block', () => {
		expect(enter('#+begin_src sh\nx\n#+end_src\n- item|')).toBe('#+begin_src sh\nx\n#+end_src\n- item\n- |');
	});
});
