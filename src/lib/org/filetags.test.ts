import { describe, expect, it } from 'vitest';
import {
	findFiletagsIndex,
	getFiletags,
	isValidTag,
	normaliseTag,
	parseFiletagsValue,
	setFiletags,
	toggleFiletag,
} from './filetags';

describe('parseFiletagsValue', () => {
	it('reads the colon-delimited form', () => {
		expect(parseFiletagsValue(':project:work:')).toEqual(['project', 'work']);
	});

	it('reads the space-separated form as separate tags', () => {
		expect(parseFiletagsValue('foo bar')).toEqual(['foo', 'bar']);
	});

	it('reads a mixed form', () => {
		expect(parseFiletagsValue(':a: :b:')).toEqual(['a', 'b']);
	});

	it('returns nothing for an empty value', () => {
		expect(parseFiletagsValue('   ')).toEqual([]);
	});
});

describe('getFiletags', () => {
	it('finds tags regardless of keyword case', () => {
		expect(getFiletags(['#+filetags: :a:b:'])).toEqual(['a', 'b']);
		expect(getFiletags(['#+FILETAGS: :a:'])).toEqual(['a']);
	});

	it('returns nothing when the line is absent', () => {
		expect(getFiletags(['#+TITLE: No tags'])).toEqual([]);
	});
});

describe('findFiletagsIndex', () => {
	it('locates the line', () => {
		expect(findFiletagsIndex(['#+TITLE: x', '#+FILETAGS: :a:'])).toBe(1);
		expect(findFiletagsIndex(['#+TITLE: x'])).toBeNull();
	});
});

describe('normaliseTag and isValidTag', () => {
	it('accepts legal org tag characters', () => {
		expect(isValidTag('work_1')).toBe(true);
		expect(isValidTag('@home')).toBe(true);
		expect(isValidTag('has space')).toBe(false);
	});

	it('rewrites illegal characters instead of writing a broken tag', () => {
		expect(normaliseTag('foo bar')).toBe('foo_bar');
		expect(normaliseTag('  spaced  ')).toBe('spaced');
		expect(normaliseTag('a:b')).toBe('a_b');
	});

	it('returns null when nothing usable remains', () => {
		expect(normaliseTag('   ')).toBeNull();
		expect(normaliseTag(':::')).toBeNull();
	});

	it('keeps unicode tags', () => {
		expect(normaliseTag('café')).toBe('café');
	});
});

describe('setFiletags', () => {
	it('rewrites an existing line in colon form', () => {
		const lines = ['#+TITLE: x', '#+FILETAGS: :old:'];
		expect(setFiletags(lines, ['new', 'other'])[1]).toBe('#+FILETAGS: :new:other:');
	});

	it('converts a space-separated line to colon form without losing tags', () => {
		const lines = ['#+FILETAGS: foo bar'];
		expect(setFiletags(lines, getFiletags(lines))[0]).toBe('#+FILETAGS: :foo:bar:');
	});

	it('inserts a new line after the metadata block', () => {
		const lines = ['#+TITLE: x', '', 'Body text'];
		const next = setFiletags(lines, ['a']);
		expect(next[1]).toBe('#+FILETAGS: :a:');
		expect(next[3]).toBe('Body text');
	});

	it('inserts after a file-level property drawer', () => {
		const lines = [':PROPERTIES:', ':ID: abc', ':END:', '#+TITLE: x', 'Body'];
		const next = setFiletags(lines, ['a']);
		expect(next[4]).toBe('#+FILETAGS: :a:');
	});

	it('removes the line when the last tag goes', () => {
		const lines = ['#+TITLE: x', '#+FILETAGS: :a:'];
		expect(setFiletags(lines, [])).toEqual(['#+TITLE: x']);
	});

	it('normalises and de-duplicates', () => {
		expect(setFiletags(['#+FILETAGS: :x:'], ['a b', 'a_b', 'c'])[0]).toBe('#+FILETAGS: :a_b:c:');
	});

	it('preserves the keyword’s original case and CRLF', () => {
		expect(setFiletags(['#+filetags: :a:\r'], ['b'])[0]).toBe('#+filetags: :b:\r');
	});
});

describe('toggleFiletag', () => {
	it('adds a missing tag', () => {
		expect(toggleFiletag(['#+FILETAGS: :a:'], 'b')[0]).toBe('#+FILETAGS: :a:b:');
	});

	it('removes a present tag', () => {
		expect(toggleFiletag(['#+FILETAGS: :a:b:'], 'a')[0]).toBe('#+FILETAGS: :b:');
	});

	it('normalises before comparing so a spaced tag round-trips', () => {
		const once = toggleFiletag(['#+TITLE: x'], 'my tag');
		expect(getFiletags(once)).toEqual(['my_tag']);
		expect(getFiletags(toggleFiletag(once, 'my tag'))).toEqual([]);
	});

	it('ignores an unusable tag', () => {
		const lines = ['#+FILETAGS: :a:'];
		expect(toggleFiletag(lines, '   ')).toEqual(lines);
	});
});
