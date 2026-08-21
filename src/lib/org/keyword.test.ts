import { describe, expect, it } from 'vitest';
import {
	DEFAULT_CATEGORY_CONFIG, keywordCategory, keywordCategoryClass,
	notDoneKeywords, toKeywordConfig,
} from './keyword';
import { getTodoKeyword, isDoneKeyword, nextTodoKeyword, setTodoKeyword } from './headline';
import { agendaReason } from './agenda';

const config = {
	todoKeywords: ['TODO', 'NEXT'],
	waitingKeywords: ['WAITING', 'HOLD'],
	doneKeywords: ['DONE', 'CANCELLED'],
};

describe('keywordCategory', () => {
	it('splits the three configured categories', () => {
		expect(keywordCategory('TODO', config)).toBe('todo');
		expect(keywordCategory('NEXT', config)).toBe('todo');
		expect(keywordCategory('WAITING', config)).toBe('waiting');
		expect(keywordCategory('HOLD', config)).toBe('waiting');
		expect(keywordCategory('DONE', config)).toBe('done');
		expect(keywordCategory('CANCELLED', config)).toBe('done');
	});

	it('treats a missing keyword as none', () => {
		expect(keywordCategory(null, config)).toBe('none');
		expect(keywordCategory(undefined, config)).toBe('none');
		expect(keywordCategory('', config)).toBe('none');
	});

	it('treats an unconfigured keyword as unfinished work, not as done', () => {
		// A `#+TODO:` line the user has not mirrored into settings still means
		// the headline is open; colouring it as done would be a lie.
		expect(keywordCategory('DELEGATED', config)).toBe('todo');
	});

	it('never shows a keyword as open when it is also configured as done', () => {
		const overlap = { todoKeywords: ['X'], waitingKeywords: ['X'], doneKeywords: ['X'] };
		expect(keywordCategory('X', overlap)).toBe('done');
	});

	it('defaults to plain TODO/DONE with no waiting states', () => {
		expect(keywordCategory('TODO', DEFAULT_CATEGORY_CONFIG)).toBe('todo');
		expect(keywordCategory('DONE', DEFAULT_CATEGORY_CONFIG)).toBe('done');
		expect(keywordCategory('WAITING', DEFAULT_CATEGORY_CONFIG)).toBe('todo');
	});

	it('maps to a paint class', () => {
		expect(keywordCategoryClass('WAITING', config)).toBe('state-waiting');
		expect(keywordCategoryClass(null, config)).toBe('state-none');
	});
});

describe('waiting keywords stay not-done to org', () => {
	it('lists active states before blocked ones', () => {
		expect(notDoneKeywords(config)).toEqual(['TODO', 'NEXT', 'WAITING', 'HOLD']);
	});

	it('hands waiting states to the parser as TODO keywords', () => {
		expect(toKeywordConfig(config)).toEqual({
			todoKeywords: ['TODO', 'NEXT', 'WAITING', 'HOLD'],
			doneKeywords: ['DONE', 'CANCELLED'],
		});
	});

	it('parses a waiting headline as a keyword, not as part of the title', () => {
		const parser = toKeywordConfig(config);
		expect(getTodoKeyword('* WAITING Ship the release', parser)).toBe('WAITING');
	});

	it('regression: omitting waiting states swallows the keyword into the title', () => {
		const broken = { todoKeywords: config.todoKeywords, doneKeywords: config.doneKeywords };
		expect(getTodoKeyword('* WAITING Ship the release', broken)).toBe(null);
	});

	it('does not count a waiting task as done', () => {
		expect(isDoneKeyword('WAITING', toKeywordConfig(config))).toBe(false);
	});

	it('keeps a waiting task on the agenda', () => {
		const item = { todo: 'WAITING', scheduled: '<2026-08-20 Thu>', deadline: null };
		expect(agendaReason(item, '2026-08-20', '2026-08-20', config.doneKeywords)).toBe('scheduled');
	});

	it('cycles active, then waiting, then done, then none', () => {
		const parser = toKeywordConfig(config);
		const seen: (string | null)[] = [];
		let current: string | null = null;
		for (let i = 0; i < 7; i += 1) {
			current = nextTodoKeyword(current, parser);
			seen.push(current);
		}
		expect(seen).toEqual(['TODO', 'NEXT', 'WAITING', 'HOLD', 'DONE', 'CANCELLED', null]);
	});

	it('writes a waiting keyword onto a headline', () => {
		const parser = toKeywordConfig(config);
		expect(setTodoKeyword('* TODO Ship it', 'WAITING', parser)).toBe('* WAITING Ship it');
		expect(setTodoKeyword('* WAITING Ship it', 'DONE', parser)).toBe('* DONE Ship it');
	});
});
