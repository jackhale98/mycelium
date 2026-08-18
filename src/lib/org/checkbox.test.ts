import { describe, expect, it } from 'vitest';
import {
	listCheckboxes,
	parseCheckboxLine,
	recomputeCookies,
	setCheckbox,
	toggleCheckbox,
	toggleCheckboxAndCookies,
	toggleCheckboxAtOffset,
} from './checkbox';

describe('parseCheckboxLine', () => {
	it('reads state from each bullet style', () => {
		expect(parseCheckboxLine('- [ ] todo')?.state).toBe('unchecked');
		expect(parseCheckboxLine('- [X] done')?.state).toBe('checked');
		expect(parseCheckboxLine('- [x] done')?.state).toBe('checked');
		expect(parseCheckboxLine('- [-] partial')?.state).toBe('partial');
		expect(parseCheckboxLine('1. [ ] numbered')?.state).toBe('unchecked');
		expect(parseCheckboxLine('  + [ ] plus')?.state).toBe('unchecked');
	});

	it('ignores list items without a box and non-items', () => {
		expect(parseCheckboxLine('- plain item')).toBeNull();
		expect(parseCheckboxLine('Some [ ] prose')).toBeNull();
		expect(parseCheckboxLine('* TODO headline')).toBeNull();
	});

	it('records the indent and box offset', () => {
		const item = parseCheckboxLine('   - [ ] nested');
		expect(item?.indent).toBe('   ');
		expect(item?.offset).toBe(5);
	});
});

describe('toggleCheckbox', () => {
	it('toggles the box on the given line', () => {
		expect(toggleCheckbox(['- [ ] a'], 0)[0]).toBe('- [X] a');
		expect(toggleCheckbox(['- [X] a'], 0)[0]).toBe('- [ ] a');
	});

	it('targets the item’s own box, not the first bracket text on the line', () => {
		const lines = ['- [ ] see [ ] in the text'];
		expect(toggleCheckbox(lines, 0)[0]).toBe('- [X] see [ ] in the text');
	});

	it('does not confuse a later item’s box', () => {
		const lines = ['- [ ] first', '- [ ] second'];
		const next = toggleCheckbox(lines, 1);
		expect(next[0]).toBe('- [ ] first');
		expect(next[1]).toBe('- [X] second');
	});

	it('leaves lines without a checkbox untouched', () => {
		const lines = ['plain text'];
		expect(toggleCheckbox(lines, 0)).toEqual(lines);
		expect(toggleCheckbox(lines, 99)).toEqual(lines);
	});

	it('preserves trailing content and CRLF endings', () => {
		expect(toggleCheckbox(['- [ ] a\r'], 0)[0]).toBe('- [X] a\r');
	});
});

describe('toggleCheckboxAtOffset', () => {
	it('resolves the line containing the offset', () => {
		const lines = ['- [ ] one', '- [ ] two', '- [ ] three'];
		const next = toggleCheckboxAtOffset(lines, 12);
		expect(next[1]).toBe('- [X] two');
	});
});

describe('setCheckbox', () => {
	it('writes an explicit state', () => {
		expect(setCheckbox(['- [ ] a'], 0, 'partial')[0]).toBe('- [-] a');
	});
});

describe('listCheckboxes', () => {
	it('finds every item in order', () => {
		const items = listCheckboxes(['- [ ] a', 'text', '  - [X] b']);
		expect(items.map((i) => i.lineIndex)).toEqual([0, 2]);
	});
});

describe('recomputeCookies', () => {
	it('updates a fraction cookie from its children', () => {
		const lines = ['- Parent [0/2]', '  - [X] a', '  - [ ] b'];
		expect(recomputeCookies(lines)[0]).toBe('- Parent [1/2]');
	});

	it('updates a percentage cookie', () => {
		const lines = ['- Parent [0%]', '  - [X] a', '  - [X] b', '  - [ ] c', '  - [ ] d'];
		expect(recomputeCookies(lines)[0]).toBe('- Parent [50%]');
	});

	it('updates a cookie on a headline', () => {
		const lines = ['* Tasks [0/3]', '- [X] a', '- [ ] b', '- [ ] c'];
		expect(recomputeCookies(lines)[0]).toBe('* Tasks [1/3]');
	});

	it('counts only direct children, not deeper nesting', () => {
		const lines = ['- Parent [0/1]', '  - [ ] child', '    - [X] grandchild'];
		expect(recomputeCookies(lines)[0]).toBe('- Parent [0/1]');
	});

	it('stops at the next headline', () => {
		const lines = ['* One [0/1]', '- [X] a', '* Two', '- [ ] b'];
		expect(recomputeCookies(lines)[0]).toBe('* One [1/1]');
	});

	it('leaves a cookie with no checkbox children alone', () => {
		const lines = ['* Heading [2/5]', 'just prose'];
		expect(recomputeCookies(lines)[0]).toBe('* Heading [2/5]');
	});

	it('handles an empty cookie form', () => {
		const lines = ['- Parent [/]', '  - [X] a', '  - [ ] b'];
		expect(recomputeCookies(lines)[0]).toBe('- Parent [1/2]');
	});
});

describe('toggleCheckboxAndCookies', () => {
	it('toggles and refreshes the parent cookie in one step', () => {
		const lines = ['* Tasks [0/2]', '- [ ] a', '- [ ] b'];
		const next = toggleCheckboxAndCookies(lines, 1);
		expect(next[0]).toBe('* Tasks [1/2]');
		expect(next[1]).toBe('- [X] a');
	});
});
