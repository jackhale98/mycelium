import { describe, expect, it } from 'vitest';
import {
	agendaReason,
	compareTasks,
	deadlineWarningDays,
	isDone,
	itemsForDate,
	overdueItems,
	priorityRank,
	sortTasks,
	timestampDate,
} from './agenda';

const DONE = ['DONE', 'CANCELLED'];
const today = '2026-08-17';

describe('timestampDate', () => {
	it('extracts the date from any timestamp form', () => {
		expect(timestampDate('<2026-08-17 Mon>')).toBe('2026-08-17');
		expect(timestampDate('[2026-08-17 Mon 09:00]')).toBe('2026-08-17');
		expect(timestampDate('<2026-08-17 Mon +1w -2d>')).toBe('2026-08-17');
		expect(timestampDate(null)).toBeNull();
	});
});

describe('deadlineWarningDays', () => {
	it('reads the -Nd cookie', () => {
		expect(deadlineWarningDays('<2026-08-20 Thu -3d>')).toBe(3);
		expect(deadlineWarningDays('<2026-08-20 Thu -2w>')).toBe(14);
	});

	it('falls back to org’s default when absent', () => {
		expect(deadlineWarningDays('<2026-08-20 Thu>')).toBe(14);
	});

	it('does not mistake a repeater for a warning', () => {
		expect(deadlineWarningDays('<2026-08-20 Thu +1w>')).toBe(14);
	});
});

describe('isDone', () => {
	it('respects the configured DONE keywords', () => {
		expect(isDone({ todo: 'CANCELLED' }, DONE)).toBe(true);
		expect(isDone({ todo: 'TODO' }, DONE)).toBe(false);
		expect(isDone({ todo: null }, DONE)).toBe(false);
	});
});

describe('agendaReason', () => {
	it('shows a scheduled item on its day', () => {
		const item = { todo: 'TODO', scheduled: '<2026-08-17 Mon>' };
		expect(agendaReason(item, today, today, DONE)).toBe('scheduled');
	});

	it('carries an overdue scheduled item forward to today', () => {
		const item = { todo: 'TODO', scheduled: '<2026-08-10 Mon>' };
		expect(agendaReason(item, today, today, DONE)).toBe('overdue-scheduled');
	});

	it('does not show an overdue item on days other than today', () => {
		const item = { todo: 'TODO', scheduled: '<2026-08-10 Mon>' };
		expect(agendaReason(item, '2026-08-18', today, DONE)).toBeNull();
	});

	it('shows a deadline on its day', () => {
		const item = { todo: 'TODO', deadline: '<2026-08-20 Thu>' };
		expect(agendaReason(item, '2026-08-20', today, DONE)).toBe('deadline');
	});

	it('warns about an upcoming deadline within its warning window', () => {
		const item = { todo: 'TODO', deadline: '<2026-08-20 Thu>' };
		expect(agendaReason(item, today, today, DONE)).toBe('upcoming-deadline');
	});

	it('stays quiet outside the warning window', () => {
		const item = { todo: 'TODO', deadline: '<2026-12-25 Fri>' };
		expect(agendaReason(item, today, today, DONE)).toBeNull();
	});

	it('honours a shorter -Nd window', () => {
		const item = { todo: 'TODO', deadline: '<2026-08-20 Thu -1d>' };
		expect(agendaReason(item, today, today, DONE)).toBeNull();
		expect(agendaReason(item, '2026-08-19', today, DONE)).toBe('upcoming-deadline');
	});

	it('keeps an overdue deadline on today', () => {
		const item = { todo: 'TODO', deadline: '<2026-08-01 Sat>' };
		expect(agendaReason(item, today, today, DONE)).toBe('deadline');
	});

	it('hides done items, and shows them only on the day they were closed', () => {
		const item = { todo: 'DONE', scheduled: '<2026-08-17 Mon>', closed: '[2026-08-17 Mon 09:00]' };
		expect(agendaReason(item, today, today, DONE)).toBe('scheduled');
		expect(agendaReason({ ...item, closed: '[2026-08-16 Sun]' }, today, today, DONE)).toBeNull();
	});

	it('hides a done item with a custom DONE keyword', () => {
		const item = { todo: 'CANCELLED', scheduled: '<2026-08-17 Mon>' };
		expect(agendaReason(item, today, today, DONE)).toBeNull();
	});
});

describe('itemsForDate and overdueItems', () => {
	const items = [
		{ todo: 'TODO', scheduled: '<2026-08-17 Mon>' },
		{ todo: 'TODO', scheduled: '<2026-08-10 Mon>' },
		{ todo: 'DONE', scheduled: '<2026-08-10 Mon>' },
		{ todo: 'TODO', deadline: '<2026-08-20 Thu>' },
	];

	it('collects everything due on a day', () => {
		expect(itemsForDate(items, today, today, DONE)).toHaveLength(3);
	});

	it('collects overdue open items only', () => {
		const overdue = overdueItems(items, today, DONE);
		expect(overdue).toHaveLength(1);
		expect(overdue[0].scheduled).toBe('<2026-08-10 Mon>');
	});
});

describe('priorityRank', () => {
	it('orders configured priorities highest first', () => {
		expect(priorityRank('A')).toBeLessThan(priorityRank('C'));
	});

	it('treats an unset priority as the middle of the range, not last', () => {
		expect(priorityRank(null)).toBeLessThan(priorityRank('C'));
		expect(priorityRank(null)).toBeGreaterThan(priorityRank('A'));
	});

	it('supports custom priority sets', () => {
		const config = { priorities: ['1', '2', '3', '4'] };
		expect(priorityRank('1', config)).toBeLessThan(priorityRank('4', config));
	});
});

describe('sortTasks', () => {
	it('orders deadline before scheduled before undated', () => {
		const sorted = sortTasks([
			{ todo: 'TODO' },
			{ todo: 'TODO', scheduled: '<2026-08-17 Mon>' },
			{ todo: 'TODO', deadline: '<2026-08-20 Thu>' },
		]);
		expect(sorted[0].deadline).toBeDefined();
		expect(sorted[1].scheduled).toBeDefined();
		expect(sorted[2].deadline).toBeUndefined();
	});

	it('orders by date within a group, then by priority', () => {
		const sorted = sortTasks([
			{ todo: 'TODO', deadline: '<2026-08-25 Tue>' },
			{ todo: 'TODO', deadline: '<2026-08-20 Thu>', priority: 'C' },
			{ todo: 'TODO', deadline: '<2026-08-20 Thu>', priority: 'A' },
		]);
		expect(sorted[0].priority).toBe('A');
		expect(sorted[1].priority).toBe('C');
		expect(sorted[2].deadline).toBe('<2026-08-25 Tue>');
	});

	it('is stable for equal items', () => {
		const a = { todo: 'TODO', title: 'a' } as never;
		const b = { todo: 'TODO', title: 'b' } as never;
		expect(sortTasks([a, b])).toEqual([a, b]);
	});

	it('does not mutate its input', () => {
		const items = [{ todo: 'TODO' }, { todo: 'TODO', deadline: '<2026-08-20 Thu>' }];
		const copy = [...items];
		sortTasks(items);
		expect(items).toEqual(copy);
	});

	it('compareTasks returns 0 for equivalent items', () => {
		expect(compareTasks({ todo: 'TODO' }, { todo: 'TODO' })).toBe(0);
	});
});
