import { describe, expect, it } from 'vitest';
import {
	cycleTodoKeyword,
	escapeRegExp,
	findHeadlineIndex,
	formatHeadline,
	getPriority,
	getTodoKeyword,
	headlineLevel,
	isDoneKeyword,
	isHeadlineLine,
	parseHeadline,
	setPriority,
	setTodoKeyword,
} from './headline';
import type { KeywordConfig } from './types';

const config: KeywordConfig = { todoKeywords: ['TODO', 'NEXT'], doneKeywords: ['DONE', 'CANCELLED'] };

describe('isHeadlineLine', () => {
	it('accepts column-0 stars followed by whitespace', () => {
		expect(isHeadlineLine('* Heading')).toBe(true);
		expect(isHeadlineLine('*** Deep')).toBe(true);
	});

	it('rejects a bare star line, which org treats as text', () => {
		expect(isHeadlineLine('**')).toBe(false);
		expect(isHeadlineLine('*')).toBe(false);
	});

	it('rejects an indented bullet, which is a list item', () => {
		expect(isHeadlineLine('  * TODO call mum')).toBe(false);
		expect(isHeadlineLine('\t* item')).toBe(false);
	});

	it('handles CRLF lines', () => {
		expect(isHeadlineLine('* Heading\r')).toBe(true);
	});
});

describe('headlineLevel', () => {
	it('counts stars', () => {
		expect(headlineLevel('** Two')).toBe(2);
		expect(headlineLevel('not a headline')).toBe(0);
	});
});

describe('parseHeadline', () => {
	it('splits keyword, priority, title and tags', () => {
		const h = parseHeadline('** TODO [#A] Write tests :work:urgent:', config)!;
		expect(h.level).toBe(2);
		expect(h.keyword).toBe('TODO');
		expect(h.priority).toBe('A');
		expect(h.title).toBe('Write tests');
		expect(h.tags).toEqual(['work', 'urgent']);
	});

	it('reads a tag-only headline as tags, not as a title', () => {
		const h = parseHeadline('* :tagonly:', config)!;
		expect(h.tags).toEqual(['tagonly']);
		expect(h.title).toBe('');
	});

	it('accepts a numeric priority cookie', () => {
		expect(parseHeadline('* TODO [#1] Numbered', config)!.priority).toBe('1');
	});

	it('collapses stacked priority cookies instead of accumulating them', () => {
		const h = parseHeadline('* TODO [#2] [#1] Title', config)!;
		expect(h.priority).toBe('2');
		expect(h.title).toBe('Title');
	});

	it('treats a bare keyword as a keyword with an empty title', () => {
		const h = parseHeadline('* TODO', config)!;
		expect(h.keyword).toBe('TODO');
		expect(h.title).toBe('');
	});

	it('does not match an unconfigured keyword', () => {
		expect(parseHeadline('* WAITING something', config)!.keyword).toBeNull();
	});

	it('returns null for non-headlines', () => {
		expect(parseHeadline('  * bullet', config)).toBeNull();
		expect(parseHeadline('text', config)).toBeNull();
	});
});

describe('formatHeadline round trip', () => {
	it.each([
		'* Simple',
		'** TODO Task',
		'*** TODO [#A] With priority',
		'* DONE Done :tag:',
		'* TODO [#B] Everything :a:b:',
		'* :tagonly:',
	])('re-renders %s unchanged', (line) => {
		expect(formatHeadline(parseHeadline(line, config)!)).toBe(line);
	});

	it('keeps a CRLF ending', () => {
		expect(formatHeadline(parseHeadline('* TODO Task\r', config)!)).toBe('* TODO Task\r');
	});
});

describe('setTodoKeyword', () => {
	it('adds a keyword to a plain headline', () => {
		expect(setTodoKeyword('* Task', 'TODO', config)).toBe('* TODO Task');
	});

	it('replaces an existing keyword', () => {
		expect(setTodoKeyword('* TODO Task', 'DONE', config)).toBe('* DONE Task');
	});

	it('clears a keyword', () => {
		expect(setTodoKeyword('* TODO Task', null, config)).toBe('* Task');
	});

	it('does not duplicate the keyword on a bare `* TODO`', () => {
		expect(setTodoKeyword('* TODO', 'TODO', config)).toBe('* TODO');
	});

	it('keeps priority and tags in place', () => {
		expect(setTodoKeyword('* TODO [#A] Task :work:', 'DONE', config)).toBe('* DONE [#A] Task :work:');
	});
});

describe('cycleTodoKeyword', () => {
	it('walks the configured sequence and back to none', () => {
		let line = '* Task';
		line = cycleTodoKeyword(line, config);
		expect(getTodoKeyword(line, config)).toBe('TODO');
		line = cycleTodoKeyword(line, config);
		expect(getTodoKeyword(line, config)).toBe('NEXT');
		line = cycleTodoKeyword(line, config);
		expect(getTodoKeyword(line, config)).toBe('DONE');
	});

	it('handles keywords containing regex metacharacters', () => {
		const odd: KeywordConfig = { todoKeywords: ['TO.DO'], doneKeywords: ['DONE'] };
		expect(getTodoKeyword('* TO.DO Task', odd)).toBe('TO.DO');
		expect(getTodoKeyword('* TOXDO Task', odd)).toBeNull();
	});
});

describe('setPriority', () => {
	it('adds, replaces and clears without stacking cookies', () => {
		expect(setPriority('* TODO Task', 'A', config)).toBe('* TODO [#A] Task');
		expect(setPriority('* TODO [#A] Task', 'B', config)).toBe('* TODO [#B] Task');
		expect(setPriority('* TODO [#A] Task', null, config)).toBe('* TODO Task');
	});

	it('replaces a numeric priority cleanly', () => {
		expect(setPriority('* TODO [#1] Task', '2', config)).toBe('* TODO [#2] Task');
	});

	it('reads back what it wrote', () => {
		expect(getPriority(setPriority('* Task', 'C', config), config)).toBe('C');
	});
});

describe('isDoneKeyword', () => {
	it('respects custom done states', () => {
		expect(isDoneKeyword('CANCELLED', config)).toBe(true);
		expect(isDoneKeyword('NEXT', config)).toBe(false);
	});
});

describe('findHeadlineIndex', () => {
	it('walks back to the owning headline', () => {
		const lines = ['* One', 'body', '** Two', 'SCHEDULED: <2026-08-17 Mon>', 'more'];
		expect(findHeadlineIndex(lines, 4)).toBe(2);
		expect(findHeadlineIndex(lines, 1)).toBe(0);
	});

	it('returns -1 above the first headline', () => {
		expect(findHeadlineIndex(['#+TITLE: x', 'text'], 1)).toBe(-1);
	});
});

describe('escapeRegExp', () => {
	it('escapes metacharacters', () => {
		expect(new RegExp(`^${escapeRegExp('a+b')}$`).test('a+b')).toBe(true);
	});
});
