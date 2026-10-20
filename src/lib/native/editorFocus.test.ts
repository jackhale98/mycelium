import { describe, expect, it } from 'vitest';
import { focusReport, isEditorTarget } from './editorFocus';

/** Just enough of an Element for `closest`. */
function el(insideEditor: boolean): EventTarget {
	return { closest: (sel: string) => (sel === '.cm-content' && insideEditor ? {} : null) } as any;
}

describe('isEditorTarget', () => {
	it('recognises elements inside the editor', () => {
		expect(isEditorTarget(el(true))).toBe(true);
	});

	it('rejects other fields, nothing, and non-elements', () => {
		expect(isEditorTarget(el(false))).toBe(false);
		expect(isEditorTarget(null)).toBe(false);
		expect(isEditorTarget({} as EventTarget)).toBe(false);
	});
});

describe('focusReport', () => {
	it('shows the toolbar when the editor gains focus', () => {
		expect(focusReport('focusin', el(true), null)).toBe(true);
	});

	it('hides it when the search field gains focus', () => {
		// The regression: the toolbar used to appear over every field.
		expect(focusReport('focusin', el(false), el(true))).toBe(false);
	});

	it('hides it when focus leaves for nowhere', () => {
		expect(focusReport('focusout', el(true), null)).toBe(false);
	});

	it('stays quiet when focus moves to another field, leaving that to focusin', () => {
		expect(focusReport('focusout', el(true), el(false))).toBeNull();
	});
});
