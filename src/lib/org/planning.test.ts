import { describe, expect, it } from 'vitest';
import {
	findPlanningLineIndex,
	getPlanning,
	isPlanningLine,
	parsePlanningLine,
	removePlanning,
	setClosed,
	setPlanning,
	setPlanningDate,
} from './planning';
import { makeTimestamp } from './timestamp';

describe('parsePlanningLine', () => {
	it('parses a single entry', () => {
		const parsed = parsePlanningLine('SCHEDULED: <2026-03-20 Fri>');
		expect(parsed?.entries).toHaveLength(1);
		expect(parsed?.entries[0].kind).toBe('SCHEDULED');
		expect(parsed?.entries[0].timestamp.date).toBe('2026-03-20');
	});

	it('parses a combined line and keeps each cookie with its own entry', () => {
		const parsed = parsePlanningLine('SCHEDULED: <2026-03-20 Fri +1w -2d> DEADLINE: <2026-03-25 Wed>');
		expect(parsed?.entries).toHaveLength(2);
		expect(parsed?.entries[0].timestamp.repeater).toBe('+1w');
		expect(parsed?.entries[0].timestamp.warning).toBe('-2d');
		expect(parsed?.entries[1].timestamp.repeater).toBeNull();
		expect(parsed?.entries[1].timestamp.warning).toBeNull();
	});

	it('parses CLOSED written as an inactive timestamp', () => {
		const parsed = parsePlanningLine('CLOSED: [2026-03-20 Fri 14:30]');
		expect(parsed?.entries[0].timestamp.active).toBe(false);
		expect(parsed?.entries[0].timestamp.time).toBe('14:30');
	});

	it('preserves indentation and CRLF endings', () => {
		const parsed = parsePlanningLine('   DEADLINE: <2026-03-20 Fri>\r');
		expect(parsed?.indent).toBe('   ');
		expect(parsed?.eol).toBe('\r');
	});

	it('rejects prose that merely mentions a planning keyword', () => {
		expect(isPlanningLine('We SCHEDULED: the review for later.')).toBe(false);
		expect(isPlanningLine('The old DEADLINE: <2020-01-01 Wed> was missed.')).toBe(false);
		expect(isPlanningLine('Some body text')).toBe(false);
		expect(isPlanningLine('')).toBe(false);
	});

	it('rejects a keyword with no timestamp', () => {
		expect(isPlanningLine('SCHEDULED:')).toBe(false);
		expect(isPlanningLine('SCHEDULED: soon')).toBe(false);
	});
});

describe('findPlanningLineIndex', () => {
	it('only accepts the line directly after the headline', () => {
		const lines = ['* TODO Task', 'SCHEDULED: <2026-03-20 Fri>', ':PROPERTIES:', ':ID: x', ':END:'];
		expect(findPlanningLineIndex(lines, 0)).toBe(1);
	});

	it('ignores planning-looking text further down the subtree', () => {
		const lines = ['* TODO Task', ':PROPERTIES:', ':ID: x', ':END:', 'SCHEDULED: <2026-03-20 Fri>'];
		expect(findPlanningLineIndex(lines, 0)).toBeNull();
	});

	it('returns null at the end of the file', () => {
		expect(findPlanningLineIndex(['* TODO Task'], 0)).toBeNull();
	});
});

describe('setPlanning', () => {
	const ts = (date: string) => makeTimestamp({ date });

	it('inserts a new planning line before the property drawer', () => {
		const lines = ['* TODO Task', ':PROPERTIES:', ':ID: abc', ':END:', 'Body text'];
		const next = setPlanning(lines, 0, 'DEADLINE', ts('2026-03-20'));
		expect(next[1]).toBe('DEADLINE: <2026-03-20 Fri>');
		expect(next[2]).toBe(':PROPERTIES:');
		expect(next).toHaveLength(6);
	});

	it('never writes planning after the drawer, where org would not see it', () => {
		const lines = ['* TODO Task', ':PROPERTIES:', ':ID: abc', ':END:'];
		const next = setPlanning(lines, 0, 'SCHEDULED', ts('2026-03-20'));
		expect(next.indexOf('SCHEDULED: <2026-03-20 Fri>')).toBeLessThan(next.indexOf(':PROPERTIES:'));
	});

	it('adds a second keyword to the existing line rather than creating another', () => {
		const lines = ['* TODO Task', 'SCHEDULED: <2026-03-20 Fri>'];
		const next = setPlanning(lines, 0, 'DEADLINE', ts('2026-03-25'));
		expect(next).toHaveLength(2);
		expect(next[1]).toBe('DEADLINE: <2026-03-25 Wed> SCHEDULED: <2026-03-20 Fri>');
	});

	it('removes only the targeted keyword from a combined line', () => {
		const lines = ['* TODO Task', 'SCHEDULED: <2026-03-20 Fri> DEADLINE: <2026-03-25 Wed>'];
		const next = removePlanning(lines, 0, 'DEADLINE');
		expect(next[1]).toBe('SCHEDULED: <2026-03-20 Fri>');
	});

	it('drops the line entirely when the last entry is removed', () => {
		const lines = ['* TODO Task', 'DEADLINE: <2026-03-25 Wed>', 'Body'];
		const next = removePlanning(lines, 0, 'DEADLINE');
		expect(next).toEqual(['* TODO Task', 'Body']);
	});

	it('leaves the file untouched when the target is not a headline', () => {
		const lines = ['Just text', 'More text'];
		expect(setPlanning(lines, 0, 'DEADLINE', ts('2026-03-20'))).toEqual(lines);
	});

	it('does not touch body text that looks like planning', () => {
		const lines = ['* TODO Task', 'The old DEADLINE: <2020-01-01 Wed> was missed.'];
		const next = setPlanning(lines, 0, 'DEADLINE', ts('2026-03-20'));
		expect(next[1]).toBe('DEADLINE: <2026-03-20 Fri>');
		expect(next[2]).toBe('The old DEADLINE: <2020-01-01 Wed> was missed.');
	});
});

describe('setPlanningDate', () => {
	it('keeps the edited entry’s own repeater and warning', () => {
		const lines = ['* TODO Water', 'SCHEDULED: <2026-03-20 Fri +1w -2d>'];
		const next = setPlanningDate(lines, 0, 'SCHEDULED', '2026-04-01');
		expect(next[1]).toBe('SCHEDULED: <2026-04-01 Wed +1w -2d>');
	});

	it('never copies a sibling entry’s cookies onto the one being edited', () => {
		const lines = ['* TODO Task', 'SCHEDULED: <2026-03-20 Fri +1w -2d> DEADLINE: <2026-03-25 Wed>'];
		const next = setPlanningDate(lines, 0, 'DEADLINE', '2026-09-05');
		expect(next[1]).toBe('DEADLINE: <2026-09-05 Sat> SCHEDULED: <2026-03-20 Fri +1w -2d>');
	});

	it('accepts a datetime-local value', () => {
		const lines = ['* TODO Task'];
		const next = setPlanningDate(lines, 0, 'SCHEDULED', '2026-03-20T14:00');
		expect(next[1]).toBe('SCHEDULED: <2026-03-20 Fri 14:00>');
	});

	it('ignores unparseable input', () => {
		const lines = ['* TODO Task', 'SCHEDULED: <2026-03-20 Fri>'];
		expect(setPlanningDate(lines, 0, 'SCHEDULED', 'not a date')).toEqual(lines);
	});
});

describe('getPlanning', () => {
	it('reads every kind off a combined line', () => {
		const lines = ['* DONE Task', 'CLOSED: [2026-03-21 Sat 09:00] SCHEDULED: <2026-03-20 Fri>'];
		const planning = getPlanning(lines, 0);
		expect(planning.CLOSED?.date).toBe('2026-03-21');
		expect(planning.SCHEDULED?.date).toBe('2026-03-20');
		expect(planning.DEADLINE).toBeNull();
	});
});

describe('setClosed', () => {
	it('writes an inactive stamp with the local time it is given', () => {
		const lines = ['* DONE Task'];
		const next = setClosed(lines, 0, { date: '2026-03-21', time: '09:05' });
		expect(next[1]).toBe('CLOSED: [2026-03-21 Sat 09:05]');
	});

	it('removes the stamp when reopened', () => {
		const lines = ['* TODO Task', 'CLOSED: [2026-03-21 Sat 09:05]'];
		expect(setClosed(lines, 0, null)).toEqual(['* TODO Task']);
	});

	it('keeps other planning entries when clearing CLOSED', () => {
		const lines = ['* TODO Task', 'CLOSED: [2026-03-21 Sat] SCHEDULED: <2026-03-20 Fri>'];
		expect(setClosed(lines, 0, null)[1]).toBe('SCHEDULED: <2026-03-20 Fri>');
	});
});
